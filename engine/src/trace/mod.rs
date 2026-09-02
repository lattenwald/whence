mod frame;
mod step;

use std::path::PathBuf;
use std::time::Instant;

use crate::host::{Host, HostError};
use crate::lang::Registry;
use crate::pos::Pos;
use crate::tree::{Limits, Stats, Tree};

pub use crate::syntax::Span;
pub use frame::{Ctx, ExprRef, Frame};
pub use step::{Expr, expand};

pub struct TraceRequest {
    pub root: PathBuf,
    pub file: PathBuf,
    pub pos: Pos,
    pub limits: Limits,
}

#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("no language for {0}")]
    NoLanguage(PathBuf),
    #[error("cursor is not on an identifier")]
    NotIdentifier,
    #[error(transparent)]
    Host(#[from] HostError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub fn trace(host: &mut dyn Host, reg: &Registry, req: &TraceRequest) -> Result<Tree, TraceError> {
    let started = Instant::now();
    let mut ctx = Ctx::new(host, reg, &req.root, req.limits, started);
    let doc = ctx.doc(&req.file)?;
    let cursor = doc
        .ident_at(req.pos)
        .ok_or(TraceError::NotIdentifier)
        .map(|n| doc.pos_of(n))?;
    drop(doc);

    let root = expand(&mut ctx, &Expr::Ident(req.file.clone(), cursor), 0)?;
    let stats = Stats {
        nodes: root.count(),
        truncated: ctx.truncated,
        host_requests: ctx.host.request_count(),
        ms: started.elapsed().as_millis() as u64,
    };
    Ok(Tree { root, stats })
}
