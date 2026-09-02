use super::Message;
use std::io::{self, BufRead, Write};

pub fn read_message(r: &mut impl BufRead) -> io::Result<Option<Message>> {
    let mut len: Option<usize> = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        if let Some(v) = line.strip_prefix("Content-Length:") {
            len =
                Some(v.trim().parse().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "bad Content-Length")
                })?);
        }
    }
    let len =
        len.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length"))?;
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    let v: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Message::from_json(v)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.message))
}

pub fn write_message(w: &mut impl Write, m: &Message) -> io::Result<()> {
    let body = serde_json::to_vec(&m.to_json())?;
    write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
    w.write_all(&body)?;
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Id, Request};
    use std::io::Cursor;

    #[test]
    fn roundtrip_request() {
        let m = Message::Request(Request {
            id: Id::Num(1),
            method: "whence/trace".into(),
            params: serde_json::json!({"file":"/a.erl","line":3,"col":4}),
        });
        let mut buf = Vec::new();
        write_message(&mut buf, &m).unwrap();
        let s = String::from_utf8(buf.clone()).unwrap();
        assert!(s.starts_with("Content-Length: "));
        assert!(s.contains("\r\n\r\n{"));
        let back = read_message(&mut Cursor::new(buf)).unwrap().unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn eof_returns_none() {
        assert!(
            read_message(&mut Cursor::new(Vec::<u8>::new()))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn response_without_result_or_error_is_error() {
        let raw = b"Content-Length: 12\r\n\r\n{\"id\":1,\"x\":1}";
        assert!(read_message(&mut Cursor::new(raw.to_vec())).is_err());
    }
}
