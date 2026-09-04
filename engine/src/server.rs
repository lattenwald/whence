use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::host_replay::ReplayHost;
use crate::host_rpc::{Caps, RpcHost};
use crate::lang::Registry;
use crate::pos::Pos;
use crate::protocol::{
    E_HOST, E_INVALID_PARAMS, E_INVALID_REQUEST, E_METHOD_NOT_FOUND, E_NO_LANGUAGE,
    E_NOT_IDENTIFIER, Id, Message, Request, Response, RpcError, read_message, write_message,
};
use crate::trace::{TraceError, TraceRequest, trace};
use crate::tree::Limits;

pub enum HostSource {
    Stdio,
    Replay(PathBuf),
}

enum Source {
    Stdio,
    Replay(Box<ReplayHost>),
}

struct SharedWriter<W>(Arc<Mutex<W>>);

impl<W> Clone for SharedWriter<W> {
    fn clone(&self) -> Self {
        SharedWriter(Arc::clone(&self.0))
    }
}

impl<W: Write> Write for SharedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().expect("stdout mutex").write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.lock().expect("stdout mutex").flush()
    }
}

#[derive(Deserialize)]
struct InitParams {
    root: PathBuf,
    #[serde(default)]
    capabilities: Caps,
}

#[derive(Deserialize)]
struct TraceParams {
    file: PathBuf,
    line: u32,
    col: u32,
    #[serde(default)]
    limits: Limits,
}

struct Session {
    root: PathBuf,
    caps: Caps,
}

pub fn serve<R, W>(reader: R, writer: W, source: HostSource) -> anyhow::Result<()>
where
    R: BufRead + Send + 'static,
    W: Write + Send + 'static,
{
    let mut source = match source {
        HostSource::Stdio => Source::Stdio,
        HostSource::Replay(dir) => Source::Replay(Box::new(ReplayHost::load(&dir)?)),
    };
    let reg = Registry::embedded()?;
    let out = SharedWriter(Arc::new(Mutex::new(writer)));
    let (tx, mut rx) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = reader;
        loop {
            match read_message(&mut reader) {
                Ok(Some(m)) => {
                    if tx.send(m).is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(e) => {
                    log::error!("stdin: {e}");
                    return;
                }
            }
        }
    });

    let mut session: Option<Session> = None;
    let mut next_host_id = 1;
    loop {
        let Ok(msg) = rx.recv() else { return Ok(()) };
        let req = match msg {
            Message::Request(r) => r,
            Message::Notification(n) => {
                if n.method == "exit" {
                    return Ok(());
                }
                log::debug!("ignoring notification {}", n.method);
                continue;
            }
            Message::Response(r) => {
                log::warn!("dropping unsolicited response {:?}", r.id);
                continue;
            }
        };
        let id = req.id.clone();
        let outcome = match req.method.as_str() {
            "initialize" => initialize(&reg, &req, &mut session),
            "whence/trace" => match &session {
                None => Err(RpcError::new(E_INVALID_REQUEST, "not initialized")),
                Some(s) => run_trace(&reg, s, &req, &mut source, &out, &mut rx, &mut next_host_id),
            },
            "shutdown" => {
                reply(&out, id, Ok(json!({})));
                return Ok(());
            }
            other => Err(RpcError::new(
                E_METHOD_NOT_FOUND,
                format!("unknown method {other}"),
            )),
        };
        reply(&out, id, outcome);
    }
}

fn initialize(
    reg: &Registry,
    req: &Request,
    session: &mut Option<Session>,
) -> Result<Value, RpcError> {
    let p: InitParams = params(&req.params)?;
    *session = Some(Session {
        root: p.root,
        caps: p.capabilities,
    });
    Ok(json!({ "version": env!("CARGO_PKG_VERSION"), "languages": reg.names() }))
}

fn run_trace<W: Write>(
    reg: &Registry,
    session: &Session,
    req: &Request,
    source: &mut Source,
    out: &SharedWriter<W>,
    rx: &mut Receiver<Message>,
    next_host_id: &mut i64,
) -> Result<Value, RpcError> {
    let p: TraceParams = params(&req.params)?;
    let treq = TraceRequest {
        root: session.root.clone(),
        file: p.file,
        pos: Pos {
            line: p.line,
            col: p.col,
        },
        limits: p.limits,
    };
    let tree = match source {
        Source::Replay(host) => {
            host.reset();
            trace(host.as_mut(), reg, &treq)
        }
        Source::Stdio => {
            let mut host = RpcHost::resume(out.clone(), rx, session.caps, *next_host_id);
            let tree = trace(&mut host, reg, &treq);
            *next_host_id = host.next_id();
            tree
        }
    };
    tree.map_err(trace_error)
        .and_then(|t| serde_json::to_value(t).map_err(|e| RpcError::new(E_HOST, e.to_string())))
}

fn trace_error(e: TraceError) -> RpcError {
    let code = match e {
        TraceError::NoLanguage(_) => E_NO_LANGUAGE,
        TraceError::NotIdentifier => E_NOT_IDENTIFIER,
        TraceError::Host(_) | TraceError::Io(_) => E_HOST,
    };
    RpcError::new(code, e.to_string())
}

fn params<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, RpcError> {
    serde_json::from_value(v.clone()).map_err(|e| RpcError::new(E_INVALID_PARAMS, e.to_string()))
}

fn reply<W: Write>(out: &SharedWriter<W>, id: Id, outcome: Result<Value, RpcError>) {
    let (result, error) = match outcome {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let mut w = out.clone();
    if let Err(e) = write_message(&mut w, &Message::Response(Response { id, result, error })) {
        log::error!("writing response: {e}");
    }
}

pub fn replay_once(
    dir: &std::path::Path,
    file: &std::path::Path,
    pos: Pos,
    limits: Limits,
) -> anyhow::Result<crate::tree::Tree> {
    let mut host = ReplayHost::load(dir)?;
    let reg = Registry::embedded()?;
    let req = TraceRequest {
        root: dir.to_path_buf(),
        file: file.to_path_buf(),
        pos,
        limits,
    };
    Ok(trace(&mut host, &reg, &req)?)
}
