//! The step function of spec §5.2: what feeds this expression?

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::host::Location;
use crate::lang::vocab;
use crate::pos::{self, Pos};
use crate::syntax::{Doc, FnDecl, N, Role};
use crate::trace::TraceError;
use crate::trace::frame::{Ctx, ExprRef, Frame, Span, node_at};
use crate::tree::{Loc, Node, NodeKind, StopReason, Via, node_id};

pub enum Expr {
    Ident(PathBuf, Pos),
    Value(PathBuf, Span),
}

struct Site {
    loc: Loc,
    /// Root-relative; only this reaches node ids.
    rel: PathBuf,
    label: String,
    snippet: String,
}

impl Site {
    fn new(ctx: &Ctx, doc: &Doc, file: &Path, n: N) -> Site {
        let p = doc.pos_of(n);
        Site {
            loc: Loc {
                file: file.to_path_buf(),
                line: p.line,
                col: p.col,
            },
            rel: ctx.rel(file).to_path_buf(),
            label: label_of(doc, n),
            snippet: doc.line_of(n).to_string(),
        }
    }

    fn pos(&self) -> Pos {
        Pos {
            line: self.loc.line,
            col: self.loc.col,
        }
    }
}

fn label_of(doc: &Doc, n: N) -> String {
    let s = doc
        .text_of(n)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if s.chars().count() > 40 {
        s.chars().take(39).collect::<String>() + "…"
    } else {
        s
    }
}

fn make(
    ctx: &mut Ctx,
    kind: NodeKind,
    site: &Site,
    via: Via,
    children: Vec<Node>,
    truncated: u32,
) -> Node {
    let id = node_id(
        &site.rel,
        site.loc.line,
        site.loc.col,
        &kind,
        ctx.frame_hash(),
    );
    Node {
        id,
        kind,
        label: site.label.clone(),
        loc: site.loc.clone(),
        via: Some(via),
        snippet: site.snippet.clone(),
        stop: None,
        children,
        truncated,
    }
}

fn stop(site: &Site, reason: StopReason, detail: impl Into<String>) -> Node {
    Node::stop_rel(
        &site.rel,
        site.loc.clone(),
        &site.label,
        &site.snippet,
        reason,
        detail,
    )
}

fn unresolved(site: &Site, detail: impl Into<String>) -> Node {
    stop(site, StopReason::Unresolved, detail)
}

pub fn expand(ctx: &mut Ctx, e: &Expr, depth: u32) -> Result<Node, TraceError> {
    let file = match e {
        Expr::Ident(f, _) | Expr::Value(f, _) => f.clone(),
    };
    let doc = ctx.doc(&file)?;
    ctx.node_count += 1;

    // A value that is just a variable is a variable use; deciding this before the
    // cycle check keeps the two forms from colliding in the visited set.
    let node = match e {
        Expr::Ident(_, p) => doc.ident_at(*p),
        Expr::Value(_, s) => node_at(&doc, *s),
    };
    let Some(node) = node else {
        let p = match e {
            Expr::Ident(_, p) => *p,
            Expr::Value(_, s) => pos::pos_of(&doc.text, s.start),
        };
        let site = Site {
            loc: Loc {
                file: file.clone(),
                line: p.line,
                col: p.col,
            },
            rel: ctx.rel(&file).to_path_buf(),
            label: String::new(),
            snippet: String::new(),
        };
        return Ok(unresolved(&site, "expression not found in the source"));
    };
    let is_ident = doc.has_cap(node, vocab::IDENT);
    let site = Site::new(ctx, &doc, &file, node);

    if depth >= ctx.limits.depth {
        return Ok(stop(&site, StopReason::Limit, "depth"));
    }
    if ctx.node_count > ctx.limits.nodes {
        return Ok(stop(&site, StopReason::Limit, "nodes"));
    }
    if Instant::now() > ctx.deadline {
        return Ok(stop(&site, StopReason::Limit, "time"));
    }
    let path_key = (file.clone(), Span::of(node), ctx.frame_hash());
    if !ctx.visited.insert(path_key.clone()) {
        return Ok(unresolved(&site, "recursion"));
    }

    let out = if is_ident {
        ident(ctx, &file, &site, depth)
    } else {
        value(ctx, &doc, &file, node, &site, depth)
    };
    // Leaving the path: a later branch may reach this expression again legitimately.
    ctx.visited.remove(&path_key);
    out
}

fn ident(ctx: &mut Ctx, file: &Path, site: &Site, depth: u32) -> Result<Node, TraceError> {
    let defs = ctx.host.definition(file, site.pos())?;
    let Some(def) = pick_definition(&defs, file) else {
        return Ok(unresolved(site, "no definition from language server"));
    };
    if !ctx.in_root(&def.file) {
        return Ok(stop(site, StopReason::External, site.label.clone()));
    }

    let doc = ctx.doc(&def.file)?;
    let Some(ident) = doc.ident_at(def.range.start) else {
        return Ok(unresolved(site, "definition is not an identifier"));
    };
    let dsite = Site::new(ctx, &doc, &def.file, ident);

    match doc.role_of(ident) {
        Role::BoundBy { pattern, value } => {
            let source = doc.destructure(pattern, ident, value).unwrap_or(value);
            let child = Expr::Value(def.file.clone(), Span::of(source));
            let child = expand(ctx, &child, depth + 1)?;
            Ok(make(
                ctx,
                NodeKind::Binding,
                &dsite,
                Via::Match,
                vec![child],
                0,
            ))
        }
        Role::BranchPattern { pattern, subject } => {
            let source = doc.destructure(pattern, ident, subject).unwrap_or(subject);
            let child = Expr::Value(def.file.clone(), Span::of(source));
            let child = expand(ctx, &child, depth + 1)?;
            Ok(make(
                ctx,
                NodeKind::Binding,
                &dsite,
                Via::Match,
                vec![child],
                0,
            ))
        }
        Role::Param { func, index } => param(ctx, &doc, &def.file, &func, index, &dsite, depth),
        Role::Opaque(n) => Ok(unresolved(&dsite, format!("bound inside {}", n.0.kind()))),
        Role::Use => Ok(unresolved(&dsite, "definition site not recognised")),
    }
}

fn pick_definition<'l>(defs: &'l [Location], file: &Path) -> Option<&'l Location> {
    defs.iter().find(|l| l.file == file).or(defs.first())
}

fn param(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    func: &FnDecl,
    index: usize,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let func_id = format!(
        "{}:{}/{}",
        ctx.rel(file).display(),
        func.name,
        func.params.len()
    );

    if let Some(frame) = ctx.frames.pop_if(|f| f.func_id == func_id) {
        let arg = frame.args.get(index).cloned();
        // The argument is the caller's expression: expand it in the caller's frame.
        let child = match arg {
            Some((f, s)) => expand(ctx, &Expr::Value(f, s), depth + 1),
            None => {
                ctx.node_count += 1;
                Ok(unresolved(
                    site,
                    format!("frame has no argument {index} for {func_id}"),
                ))
            }
        };
        ctx.frames.push(frame);
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child?], 0));
    }

    let Some(name) = capture_in(doc, func.node, vocab::FUNCTION_NAME)
        .into_iter()
        .next()
    else {
        return Ok(unresolved(site, "function declaration has no name"));
    };
    let refs = ctx.host.references(file, doc.pos_of(name), false)?;

    let mut sites: Vec<ExprRef> = Vec::new();
    for r in refs {
        if !ctx.in_root(&r.file) || ctx.reg.for_file(&r.file).is_none() {
            continue;
        }
        let rdoc = ctx.doc(&r.file)?;
        for call in rdoc.calls_containing(r.range.start) {
            if !names(&rdoc.callee_text(&call), &func.name) || call.args.len() != func.params.len()
            {
                continue;
            }
            if let Some(a) = call.args.get(index) {
                sites.push((r.file.clone(), Span::of(*a)));
            }
            break;
        }
    }

    if sites.is_empty() {
        let detail = format!("no call sites of {}/{}", func.name, func.params.len());
        let child = stop(site, StopReason::EntryPoint, detail);
        ctx.node_count += 1;
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child], 0));
    }

    sites.sort_by(|a, b| (a.0 != file, &a.0, a.1.start).cmp(&(b.0 != file, &b.0, b.1.start)));
    let dropped = fanout(ctx, &mut sites);
    let mut children = Vec::new();
    for (f, s) in sites {
        children.push(expand(ctx, &Expr::Value(f, s), depth + 1)?);
    }
    Ok(make(
        ctx,
        NodeKind::Param,
        site,
        Via::Arg,
        children,
        dropped,
    ))
}

/// `mod:fun` and `fun` both name `fun`; `flag` does not.
fn names(callee_text: &str, name: &str) -> bool {
    callee_text == name || callee_text.ends_with(&format!(":{name}"))
}

fn fanout<T>(ctx: &mut Ctx, items: &mut Vec<T>) -> u32 {
    let keep = ctx.limits.fanout as usize;
    if items.len() <= keep {
        return 0;
    }
    let dropped = (items.len() - keep) as u32;
    items.truncate(keep);
    ctx.truncated += dropped;
    dropped
}

fn value(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    n: N,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    if doc.is_literal(n) {
        return Ok(stop(site, StopReason::Literal, n.0.kind()));
    }
    if doc.is_opaque(n) {
        return Ok(unresolved(site, n.0.kind()));
    }
    if let Some((container, field)) = doc.field_access(n) {
        return match field_source(doc, n, container, &field) {
            Some(source) => {
                let child = Expr::Value(file.to_path_buf(), Span::of(source));
                let child = expand(ctx, &child, depth + 1)?;
                Ok(make(
                    ctx,
                    NodeKind::Field,
                    site,
                    Via::FieldSet,
                    vec![child],
                    0,
                ))
            }
            None => Ok(unresolved(
                site,
                format!("field {field} of {}", doc.text_of(container)),
            )),
        };
    }
    if let Some(call) = doc.call_at(n) {
        let callee = doc.callee_text(&call);
        let args: Vec<ExprRef> = call
            .args
            .iter()
            .map(|a| (file.to_path_buf(), Span::of(*a)))
            .collect();
        return call_result(
            ctx,
            doc.callee_name_pos(&call),
            &callee,
            args,
            file,
            site,
            depth,
        );
    }
    if doc.has_cap(n, vocab::CONSTRUCT) {
        return Ok(unresolved(
            site,
            format!("constructed value {}", n.0.kind()),
        ));
    }
    Ok(unresolved(site, n.0.kind()))
}

fn call_result(
    ctx: &mut Ctx,
    name_pos: Pos,
    callee: &str,
    args: Vec<ExprRef>,
    file: &Path,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let defs = ctx.host.definition(file, name_pos)?;
    if defs.is_empty() {
        return Ok(unresolved(site, "callee not found"));
    }
    if defs.iter().any(|l| !ctx.in_root(&l.file)) {
        return Ok(stop(site, StopReason::External, callee));
    }
    let def = pick_definition(&defs, file).expect("non-empty").clone();

    let doc = ctx.doc(&def.file)?;
    let Some(decl) = function_at(&doc, def.range.start) else {
        return Ok(unresolved(
            site,
            format!("definition of {callee} is not a function"),
        ));
    };
    let arity = decl.params.len();
    let func_id = format!("{}:{}/{}", ctx.rel(&def.file).display(), decl.name, arity);
    if ctx
        .frames
        .iter()
        .any(|f| f.func_id == func_id && f.args == args)
    {
        return Ok(unresolved(
            site,
            format!("recursive call to {}/{arity}", decl.name),
        ));
    }

    let mut returns: Vec<Span> = functions(&doc)
        .iter()
        .filter(|c| c.name == decl.name && c.params.len() == arity)
        .flat_map(|c| doc.returns_of(c))
        .map(Span::of)
        .collect();
    returns.sort_by_key(|s| s.start);
    let dropped = fanout(ctx, &mut returns);

    ctx.frames.push(Frame { func_id, args });
    let mut children = Vec::new();
    let mut failure = None;
    for s in returns {
        match expand(ctx, &Expr::Value(def.file.clone(), s), depth + 1) {
            Ok(n) => children.push(n),
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    ctx.frames.pop();
    if let Some(e) = failure {
        return Err(e);
    }

    let site = Site {
        loc: site.loc.clone(),
        rel: site.rel.clone(),
        label: callee.to_string(),
        snippet: site.snippet.clone(),
    };
    Ok(make(
        ctx,
        NodeKind::CallResult,
        &site,
        Via::Return,
        children,
        dropped,
    ))
}

fn field_source<'d>(doc: &'d Doc<'_>, n: N<'d>, container: N<'d>, field: &str) -> Option<N<'d>> {
    let func = doc.enclosing_function(n)?;
    let want = doc.text_of(container);
    let mut best: Option<(usize, N)> = None;
    for b in capture_in(doc, func.node, vocab::BINDING) {
        if b.0.start_byte() >= n.0.start_byte() {
            continue;
        }
        let Some((pattern, value)) = binding_parts(doc, b) else {
            continue;
        };
        if doc.text_of(pattern) != want || !doc.has_cap(value, vocab::CONSTRUCT) {
            continue;
        }
        let Some(set) = doc.construct_field(value, field) else {
            continue;
        };
        if best.is_none_or(|(start, _)| start < b.0.start_byte()) {
            best = Some((b.0.start_byte(), set));
        }
    }
    best.map(|(_, n)| n)
}

fn binding_parts<'d>(doc: &'d Doc<'_>, binding: N<'d>) -> Option<(N<'d>, N<'d>)> {
    let mut cursor = binding.0.walk();
    let mut pattern = None;
    let mut value = None;
    for c in binding.0.named_children(&mut cursor) {
        if pattern.is_none() && doc.has_cap(N(c), vocab::BINDING_PATTERN) {
            pattern = Some(N(c));
        } else if value.is_none() && doc.has_cap(N(c), vocab::BINDING_VALUE) {
            value = Some(N(c));
        }
    }
    Some((pattern?, value?))
}

fn function_at<'d>(doc: &'d Doc<'_>, p: Pos) -> Option<FnDecl<'d>> {
    let off = pos::byte_offset(&doc.text, p)?;
    let at = doc.tree.root_node().descendant_for_byte_range(off, off)?;
    doc.enclosing_function(N(at))
}

/// Every clause in the file: multi-clause functions are separate `@function` matches.
fn functions<'d>(doc: &'d Doc<'_>) -> Vec<FnDecl<'d>> {
    capture_in(doc, N(doc.tree.root_node()), vocab::FUNCTION)
        .into_iter()
        .filter_map(|n| doc.enclosing_function(n))
        .collect()
}

fn capture_in<'d>(doc: &'d Doc<'_>, root: N<'d>, cap: &str) -> Vec<N<'d>> {
    let mut out = Vec::new();
    let mut stack = vec![root.0];
    while let Some(n) = stack.pop() {
        if doc.has_cap(N(n), cap) {
            out.push(N(n));
        }
        let mut cursor = n.walk();
        stack.extend(n.named_children(&mut cursor));
    }
    out.sort_by_key(|n| (n.0.start_byte(), n.0.end_byte()));
    out
}
