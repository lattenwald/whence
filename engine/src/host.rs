use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::pos::Pos;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Range {
    pub start: Pos,
    pub end: Pos,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub file: PathBuf,
    pub range: Range,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HighlightKind {
    Read,
    Write,
    Text,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub range: Range,
    pub kind: HighlightKind,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("host request {method} failed: {message}")]
    Rpc { method: String, message: String },
    #[error("host does not support {0}")]
    Unsupported(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub trait Host {
    fn text(&mut self, file: &Path) -> Result<String, HostError>;
    fn definition(&mut self, file: &Path, pos: Pos) -> Result<Vec<Location>, HostError>;
    fn references(
        &mut self,
        file: &Path,
        pos: Pos,
        include_decl: bool,
    ) -> Result<Vec<Location>, HostError>;
    fn document_highlight(&mut self, file: &Path, pos: Pos) -> Result<Vec<Highlight>, HostError>;
    fn request_count(&self) -> u32;
}
