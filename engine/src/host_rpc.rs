use std::io::{self, Write};
use std::path::Path;
use std::sync::mpsc::Receiver;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::host::{Highlight, Host, HostError, Location};
use crate::pos::Pos;
use crate::protocol::{E_INVALID_REQUEST, Id, Message, Response, RpcError, write_message};

/// `inbox` is fed by the server's stdin reader thread; `writer` is the shared stdout.
pub struct RpcHost<'a, W: Write> {
    writer: W,
    inbox: &'a mut Receiver<Message>,
    supports_highlight: bool,
    next_id: i64,
    count: u32,
}

impl<'a, W: Write> RpcHost<'a, W> {
    #[cfg(test)]
    pub fn new(writer: W, inbox: &'a mut Receiver<Message>, supports_highlight: bool) -> Self {
        Self::resume(writer, inbox, supports_highlight, 1)
    }

    /// Continues an earlier host's id sequence, so a late response to a previous
    /// trace is never mistaken for a reply to this one.
    pub fn resume(
        writer: W,
        inbox: &'a mut Receiver<Message>,
        supports_highlight: bool,
        next_id: i64,
    ) -> Self {
        Self {
            writer,
            inbox,
            supports_highlight,
            next_id,
            count: 0,
        }
    }

    pub fn next_id(&self) -> i64 {
        self.next_id
    }

    fn call<T: DeserializeOwned>(&mut self, method: &str, params: Value) -> Result<T, HostError> {
        let result = self.call_raw(method, params)?;
        serde_json::from_value(result).map_err(|e| HostError::Rpc {
            method: method.to_string(),
            message: e.to_string(),
        })
    }

    /// LSP answers a location request with `null` for "nothing"; a VS Code host passes it on.
    fn call_list<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Vec<T>, HostError> {
        let result = self.call_raw(method, params)?;
        if result.is_null() {
            return Ok(Vec::new());
        }
        serde_json::from_value(result).map_err(|e| HostError::Rpc {
            method: method.to_string(),
            message: e.to_string(),
        })
    }

    fn call_raw(&mut self, method: &str, params: Value) -> Result<Value, HostError> {
        let id = Id::Num(self.next_id);
        self.next_id += 1;
        self.count += 1;
        write_message(
            &mut self.writer,
            &Message::Request(crate::protocol::Request {
                id: id.clone(),
                method: method.to_string(),
                params,
            }),
        )?;
        self.await_response(method, &id)
    }

    fn await_response(&mut self, method: &str, id: &Id) -> Result<Value, HostError> {
        loop {
            let msg = self.inbox.recv().map_err(|_| {
                HostError::Io(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "host channel disconnected",
                ))
            })?;
            match msg {
                Message::Response(r) if r.id == *id => {
                    if let Some(e) = r.error {
                        return Err(HostError::Rpc {
                            method: method.to_string(),
                            message: e.message,
                        });
                    }
                    return Ok(r.result.unwrap_or(Value::Null));
                }
                Message::Request(r) => write_message(
                    &mut self.writer,
                    &Message::Response(Response {
                        id: r.id,
                        result: None,
                        error: Some(RpcError::new(E_INVALID_REQUEST, "busy")),
                    }),
                )?,
                Message::Response(r) => {
                    log::warn!("dropping response with unexpected id {:?}", r.id)
                }
                Message::Notification(n) => {
                    log::warn!("dropping notification {} during host request", n.method)
                }
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct TextResult {
    text: String,
}

impl<W: Write> Host for RpcHost<'_, W> {
    fn text(&mut self, file: &Path) -> Result<String, HostError> {
        let r: TextResult = self.call("host/text", json!({ "file": file }))?;
        Ok(r.text)
    }

    fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError> {
        self.call_list(
            "host/definition",
            json!({"file": file, "line": pos.line, "col": pos.col}),
        )
    }

    fn references(
        &mut self,
        file: &Path,
        pos: Pos,
        include_decl: bool,
    ) -> Result<Vec<Location>, HostError> {
        self.call_list(
            "host/references",
            json!({"file": file, "line": pos.line, "col": pos.col,
                   "includeDeclaration": include_decl}),
        )
    }

    fn document_highlight(&mut self, file: &Path, pos: Pos) -> Result<Vec<Highlight>, HostError> {
        if !self.supports_highlight {
            return Err(HostError::Unsupported("documentHighlight"));
        }
        self.call_list(
            "host/documentHighlight",
            json!({"file": file, "line": pos.line, "col": pos.col}),
        )
    }

    fn request_count(&self) -> u32 {
        self.count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;
    use std::sync::mpsc;

    #[test]
    fn definition_sends_request_and_reads_matching_response() {
        let (tx, mut rx) = mpsc::channel();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, &mut rx, false);
        tx.send(Message::Response(Response {
            id: Id::Num(1),
            error: None,
            result: Some(serde_json::json!([
                {"file":"/x.erl","range":{"start":{"line":1,"col":2},"end":{"line":1,"col":3}}}
            ])),
        }))
        .unwrap();
        let locs = h
            .definition(std::path::Path::new("/x.erl"), Pos { line: 5, col: 6 })
            .unwrap();
        assert_eq!(locs[0].range.start, Pos { line: 1, col: 2 });
        let sent = String::from_utf8(out).unwrap();
        assert!(sent.contains(r#""method":"host/definition""#));
        assert!(sent.contains(r#""line":5"#));
    }

    #[test]
    fn null_result_for_a_location_list_is_empty() {
        for answer in [None, Some(Value::Null)] {
            let (tx, mut rx) = mpsc::channel();
            let mut out = Vec::new();
            let mut h = RpcHost::new(&mut out, &mut rx, true);
            tx.send(Message::Response(Response {
                id: Id::Num(1),
                error: None,
                result: answer,
            }))
            .unwrap();
            assert_eq!(
                h.definition(std::path::Path::new("/x.erl"), Pos { line: 0, col: 0 })
                    .unwrap(),
                Vec::new()
            );
        }
    }

    #[test]
    fn highlight_unsupported_when_host_lacks_capability() {
        let (_tx, mut rx) = mpsc::channel::<Message>();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, &mut rx, false);
        assert!(matches!(
            h.document_highlight(std::path::Path::new("/x"), Pos { line: 0, col: 0 }),
            Err(HostError::Unsupported(_))
        ));
    }

    #[test]
    fn error_response_becomes_rpc_error() {
        let (tx, mut rx) = mpsc::channel();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, &mut rx, true);
        tx.send(Message::Response(Response {
            id: Id::Num(1),
            result: None,
            error: Some(RpcError::new(E_HOST, "no client")),
        }))
        .unwrap();
        let err = h
            .document_highlight(std::path::Path::new("/x"), Pos { line: 0, col: 0 })
            .unwrap_err();
        assert!(matches!(err, HostError::Rpc { ref message, .. } if message == "no client"));
    }

    #[test]
    fn stale_messages_are_dropped_and_host_requests_answered_busy() {
        let (tx, mut rx) = mpsc::channel();
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, &mut rx, false);
        tx.send(Message::Response(Response {
            id: Id::Num(99),
            result: Some(serde_json::json!([])),
            error: None,
        }))
        .unwrap();
        tx.send(Message::Notification(Notification {
            method: "$/cancel".into(),
            params: serde_json::Value::Null,
        }))
        .unwrap();
        tx.send(Message::Request(Request {
            id: Id::Str("q".into()),
            method: "whence/trace".into(),
            params: serde_json::Value::Null,
        }))
        .unwrap();
        tx.send(Message::Response(Response {
            id: Id::Num(1),
            result: Some(serde_json::json!({"text": "hi"})),
            error: None,
        }))
        .unwrap();
        assert_eq!(h.text(std::path::Path::new("/x")).unwrap(), "hi");
        assert_eq!(h.request_count(), 1);
        let sent = String::from_utf8(out).unwrap();
        assert!(sent.contains(r#""message":"busy""#));
    }

    #[test]
    fn disconnected_channel_is_io_error() {
        let (tx, mut rx) = mpsc::channel::<Message>();
        drop(tx);
        let mut out = Vec::new();
        let mut h = RpcHost::new(&mut out, &mut rx, false);
        let err = h.text(std::path::Path::new("/x")).unwrap_err();
        assert!(matches!(err, HostError::Io(e) if e.kind() == io::ErrorKind::BrokenPipe));
    }
}
