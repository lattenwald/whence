use std::io::BufReader;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use whence::protocol::*;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_whence"))
}

fn fixture() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/erlang/local_chain"
    )
}

struct Session {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Session {
    fn spawn(args: &[&str]) -> Session {
        let mut child = bin()
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Session {
            child,
            stdin,
            stdout,
        }
    }

    fn replay_serve() -> Session {
        Session::spawn(&["replay", "--serve", fixture()])
    }

    fn recv(&mut self) -> Message {
        read_message(&mut self.stdout).unwrap().unwrap()
    }

    fn send(&mut self, m: &Message) {
        write_message(&mut self.stdin, m).unwrap();
    }

    fn request(&mut self, id: i64, method: &str, params: serde_json::Value) -> Response {
        self.send(&Message::Request(Request {
            id: Id::Num(id),
            method: method.into(),
            params,
        }));
        self.recv_response()
    }

    fn recv_response(&mut self) -> Response {
        match self.recv() {
            Message::Response(r) => r,
            other => panic!("expected a response, got {other:?}"),
        }
    }

    fn initialize(&mut self) -> Response {
        self.request(
            1,
            "initialize",
            serde_json::json!({"root": fixture(), "capabilities": {"documentHighlight": false}}),
        )
    }
}

#[test]
fn initialize_trace_and_shutdown_over_stdio_with_replay_host() {
    let mut s = Session::replay_serve();
    let r = s.initialize();
    let result = r.result.unwrap();
    assert_eq!(result["version"], env!("CARGO_PKG_VERSION"));
    assert!(
        result["languages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l == "erlang")
    );

    let r = s.request(
        2,
        "whence/trace",
        serde_json::json!({"file": format!("{}/a.erl", fixture()), "line": 6, "col": 4}),
    );
    assert_eq!(r.id, Id::Num(2));
    let tree = r.result.unwrap();
    assert_eq!(tree["root"]["label"], "Z");
    assert_eq!(tree["root"]["children"][0]["label"], "Y");

    let r = s.request(3, "shutdown", serde_json::json!({}));
    assert_eq!(r.result.unwrap(), serde_json::json!({}));
    assert!(s.child.wait().unwrap().success());
}

#[test]
fn trace_before_initialize_is_rejected() {
    let mut s = Session::replay_serve();
    let r = s.request(
        1,
        "whence/trace",
        serde_json::json!({"file": format!("{}/a.erl", fixture()), "line": 6, "col": 4}),
    );
    let e = r.error.unwrap();
    assert_eq!(e.code, E_INVALID_REQUEST);
    assert_eq!(e.message, "not initialized");
    s.send(&Message::Notification(Notification {
        method: "exit".into(),
        params: serde_json::Value::Null,
    }));
    assert!(s.child.wait().unwrap().success());
}

#[test]
fn trace_errors_are_mapped_to_codes() {
    let mut s = Session::replay_serve();
    s.initialize();

    let r = s.request(
        2,
        "whence/trace",
        serde_json::json!({"file": format!("{}/a.erl", fixture()), "line": 0, "col": 0}),
    );
    assert_eq!(r.error.unwrap().code, E_NOT_IDENTIFIER);

    let r = s.request(
        3,
        "whence/trace",
        serde_json::json!({"file": format!("{}/host.json", fixture()), "line": 0, "col": 0}),
    );
    assert_eq!(r.error.unwrap().code, E_NO_LANGUAGE);

    let r = s.request(4, "whence/trace", serde_json::json!({"file": 7}));
    assert_eq!(r.error.unwrap().code, E_INVALID_PARAMS);

    let r = s.request(5, "whence/nope", serde_json::json!({}));
    assert_eq!(r.error.unwrap().code, E_METHOD_NOT_FOUND);

    s.send(&Message::Notification(Notification {
        method: "$/whatever".into(),
        params: serde_json::Value::Null,
    }));
    s.send(&Message::Response(Response {
        id: Id::Num(99),
        result: Some(serde_json::json!({})),
        error: None,
    }));
    let r = s.request(6, "shutdown", serde_json::json!({}));
    assert!(r.error.is_none());
    assert!(s.child.wait().unwrap().success());
}

#[test]
fn limits_from_params_are_applied() {
    let mut s = Session::replay_serve();
    s.initialize();
    let r = s.request(
        2,
        "whence/trace",
        serde_json::json!({"file": format!("{}/a.erl", fixture()), "line": 6, "col": 4,
                           "limits": {"depth": 1}}),
    );
    let tree = r.result.unwrap();
    assert_eq!(tree["root"]["children"][0]["stop"]["detail"], "depth");
}

#[test]
fn eof_on_stdin_ends_the_server() {
    let mut s = Session::replay_serve();
    s.initialize();
    drop(s.stdin);
    assert!(s.child.wait().unwrap().success());
}

#[test]
fn replay_cli_json_output() {
    let out = bin()
        .args(["replay", fixture(), "a.erl:7:5", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["root"]["label"], "Z");
}

#[test]
fn replay_cli_limit_flags_are_applied() {
    let out = bin()
        .args(["replay", fixture(), "a.erl:7:5", "--json", "--depth", "1"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["root"]["children"][0]["stop"]["detail"], "depth");
}

#[test]
fn replay_cli_reports_trace_errors_on_stderr() {
    let out = bin()
        .args(["replay", fixture(), "a.erl:1:1"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("identifier"),
        "expected the NotIdentifier message on stderr"
    );
}

#[test]
fn stdio_host_requests_go_to_the_editor_and_failures_become_host_errors() {
    let mut s = Session::spawn(&["serve"]);
    s.initialize();
    s.send(&Message::Request(Request {
        id: Id::Num(2),
        method: "whence/trace".into(),
        params: serde_json::json!({"file": format!("{}/a.erl", fixture()), "line": 6, "col": 4}),
    }));
    let Message::Request(host_req) = s.recv() else {
        panic!("expected a host request")
    };
    assert_eq!(host_req.method, "host/text");

    s.send(&Message::Request(Request {
        id: Id::Num(9),
        method: "shutdown".into(),
        params: serde_json::json!({}),
    }));
    let busy = s.recv_response();
    assert_eq!(busy.id, Id::Num(9));
    assert_eq!(busy.error.unwrap().message, "busy");

    s.send(&Message::Response(Response {
        id: host_req.id,
        result: None,
        error: Some(RpcError::new(E_HOST, "no client")),
    }));
    let r = s.recv_response();
    assert_eq!(r.id, Id::Num(2));
    assert_eq!(r.error.unwrap().code, E_HOST);

    let r = s.request(3, "shutdown", serde_json::json!({}));
    assert!(r.error.is_none());
    assert!(s.child.wait().unwrap().success());
}
