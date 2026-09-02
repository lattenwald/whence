pub mod framing;
pub use framing::{read_message, write_message};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Num(i64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: Id,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub id: Id,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

pub const E_PARSE: i64 = -32700;
pub const E_INVALID_REQUEST: i64 = -32600;
pub const E_METHOD_NOT_FOUND: i64 = -32601;
pub const E_INVALID_PARAMS: i64 = -32602;
pub const E_HOST: i64 = -32000;
pub const E_NO_LANGUAGE: i64 = -32001;
pub const E_NOT_IDENTIFIER: i64 = -32002;

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Wire {
    #[serde(skip_serializing_if = "Option::is_none")]
    jsonrpc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
    // `"result": null` is a valid success reply (LSP "no definition"), distinct from absent.
    #[serde(
        default,
        deserialize_with = "present_value",
        skip_serializing_if = "Option::is_none"
    )]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

fn present_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

fn present(v: &Value) -> Option<Value> {
    (!v.is_null()).then(|| v.clone())
}

impl Message {
    pub fn to_json(&self) -> Value {
        let mut w = Wire {
            jsonrpc: Some("2.0".into()),
            ..Default::default()
        };
        match self {
            Message::Request(r) => {
                w.id = Some(r.id.clone());
                w.method = Some(r.method.clone());
                w.params = present(&r.params);
            }
            Message::Notification(n) => {
                w.method = Some(n.method.clone());
                w.params = present(&n.params);
            }
            Message::Response(r) => {
                w.id = Some(r.id.clone());
                w.result = r.result.clone();
                w.error = r.error.clone();
            }
        }
        serde_json::to_value(w).expect("wire message is serializable")
    }

    pub fn from_json(v: Value) -> Result<Message, RpcError> {
        let w: Wire = serde_json::from_value(v)
            .map_err(|e| RpcError::new(E_INVALID_REQUEST, e.to_string()))?;
        let params = w.params.unwrap_or(Value::Null);
        match (w.id, w.method, w.result, w.error) {
            (Some(id), Some(method), _, _) => Ok(Message::Request(Request { id, method, params })),
            (None, Some(method), _, _) => {
                Ok(Message::Notification(Notification { method, params }))
            }
            (Some(id), None, result, error) if result.is_some() || error.is_some() => {
                Ok(Message::Response(Response { id, result, error }))
            }
            _ => Err(RpcError::new(
                E_INVALID_REQUEST,
                "message is neither request, notification nor response",
            )),
        }
    }
}
