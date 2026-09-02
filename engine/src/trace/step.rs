//! The step function of spec §5.2: what feeds this expression?

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::host::Location;
use crate::lang::vocab;
use crate::pos::Pos;
use crate::syntax::{Doc, FnDecl, N, Role, Span};
use crate::trace::TraceError;
use crate::trace::frame::{Ctx, ExprRef, Frame};
use crate::tree::{Loc, Node, NodeKind, StopReason, Via, node_id, path_id};

pub enum Expr {
    Ident(PathBuf, Pos),
    Value(PathBuf, Span),
}

impl Expr {
    fn file(&self) -> &Path {
        match self {
            Expr::Ident(f, _) | Expr::Value(f, _) => f,
        }
    }
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

    fn relabel(&self, label: String) -> Site {
        Site {
            loc: self.loc.clone(),
            rel: self.rel.clone(),
            label,
            snippet: self.snippet.clone(),
        }
    }
}

fn label_of(doc: &Doc, n: N) -> String {
    let s = doc
        .text_of(n)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    clip(&s)
}

fn first_line_of(doc: &Doc, n: N) -> String {
    clip(doc.text_of(n).lines().next().unwrap_or("").trim())
}

fn clip(s: &str) -> String {
    if s.chars().count() > 40 {
        s.chars().take(39).collect::<String>() + "…"
    } else {
        s.to_string()
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
        ctx.parent(),
        &site.rel,
        site.loc.line,
        site.loc.col,
        &kind,
        0,
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

fn stop(ctx: &Ctx, site: &Site, reason: StopReason, detail: impl Into<String>) -> Node {
    stop_nth(ctx, site, reason, detail, 0)
}

fn stop_nth(
    ctx: &Ctx,
    site: &Site,
    reason: StopReason,
    detail: impl Into<String>,
    nth: u32,
) -> Node {
    Node::stop(
        ctx.parent(),
        &site.rel,
        site.loc.clone(),
        &site.label,
        &site.snippet,
        reason,
        detail,
        nth,
    )
}

fn unresolved(ctx: &Ctx, site: &Site, detail: impl Into<String>) -> Node {
    stop(ctx, site, StopReason::Unresolved, detail)
}

pub fn expand(ctx: &mut Ctx, e: &Expr, depth: u32) -> Result<Node, TraceError> {
    let file = e.file().to_path_buf();
    let doc = ctx.doc(&file)?;
    ctx.node_count += 1;

    // A value that is just a variable is a variable use; deciding this before the
    // cycle check keeps the two forms from colliding in the visited set.
    let node = match e {
        Expr::Ident(_, p) => doc.ident_at(*p),
        Expr::Value(_, s) => doc.node(*s),
    };
    let Some(node) = node else {
        let p = match e {
            Expr::Ident(_, p) => *p,
            Expr::Value(_, s) => doc.pos_at(s.start),
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
        return Ok(unresolved(ctx, &site, "expression not found in the source"));
    };
    let is_ident = doc.has_cap(node, vocab::IDENT);
    let site = Site::new(ctx, &doc, &file, node);

    if depth >= ctx.limits.depth {
        return Ok(stop(ctx, &site, StopReason::Limit, "depth"));
    }
    if ctx.node_count > ctx.limits.nodes {
        return Ok(stop(ctx, &site, StopReason::Limit, "nodes"));
    }
    if Instant::now() >= ctx.deadline {
        return Ok(stop(ctx, &site, StopReason::Limit, "time"));
    }
    let path_key = (file.clone(), Span::of(node), ctx.frame_hash());
    if !ctx.visited.insert(path_key.clone()) {
        return Ok(unresolved(ctx, &site, "recursion"));
    }

    ctx.path.push(path_id(
        ctx.parent(),
        &site.rel,
        site.loc.line,
        site.loc.col,
    ));
    let out = if is_ident {
        ident(ctx, &file, &site, depth)
    } else {
        value(ctx, &doc, &file, node, &site, depth)
    };
    ctx.path.pop();
    // Leaving the path: a later branch may reach this expression again legitimately.
    ctx.visited.remove(&path_key);
    out
}

fn ident(ctx: &mut Ctx, file: &Path, site: &Site, depth: u32) -> Result<Node, TraceError> {
    let defs = distinct(&ctx.definition(file, site.pos())?);
    let def = match defs.len() {
        0 => return Ok(unresolved(ctx, site, "no definition from language server")),
        1 => &defs[0],
        n => return Ok(unresolved(ctx, site, format!("{n} definitions"))),
    };
    if !ctx.in_root(&def.file) {
        return Ok(stop(ctx, site, StopReason::External, site.label.clone()));
    }

    let doc = ctx.doc(&def.file)?;
    let Some(ident) = doc.ident_at(def.range.start) else {
        return Ok(unresolved(ctx, site, "definition is not an identifier"));
    };
    let dsite = Site::new(ctx, &doc, &def.file, ident);

    match doc.role_of(ident) {
        Role::BoundBy { pattern, value }
        | Role::BranchPattern {
            pattern,
            subject: value,
        } => {
            let source = doc.destructure(pattern, ident, value).unwrap_or(value);
            let child = Expr::Value(def.file.clone(), Span::of(source));
            let child = matched(expand(ctx, &child, depth + 1)?);
            Ok(make(
                ctx,
                NodeKind::Binding,
                &dsite,
                Via::Match,
                vec![child],
                0,
            ))
        }
        Role::Param { func, index } => {
            param(ctx, &doc, &def.file, &func, index, ident, &dsite, depth)
        }
        Role::Opaque(n) => Ok(unresolved(
            ctx,
            &dsite,
            format!("bound inside {}", n.0.kind()),
        )),
        Role::Use => Ok(unresolved(ctx, &dsite, "definition site not recognised")),
    }
}

/// `via` is the edge from the parent: through a pattern a parameter is matched, not passed.
fn matched(mut n: Node) -> Node {
    if n.kind == NodeKind::Param && n.via == Some(Via::Arg) {
        n.via = Some(Via::Match);
    }
    n
}

/// Servers repeat a definition per client and per index; the same (file, range) is one place.
fn distinct(defs: &[Location]) -> Vec<Location> {
    let mut out: Vec<Location> = Vec::new();
    for d in defs {
        if !out.iter().any(|o| o.file == d.file && o.range == d.range) {
            out.push(d.clone());
        }
    }
    out
}

/// Destructuring needs one document: a caller in another file keeps the whole argument.
fn narrow(doc: &Doc, pattern: Option<N>, ident: N, file: &Path, arg: &ExprRef) -> Span {
    if arg.0 != file {
        return arg.1;
    }
    let Some(pattern) = pattern else { return arg.1 };
    let Some(value) = doc.node(arg.1) else {
        return arg.1;
    };
    doc.destructure(pattern, ident, value)
        .map(Span::of)
        .unwrap_or(arg.1)
}

#[allow(clippy::too_many_arguments)]
fn param(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    func: &FnDecl,
    index: usize,
    ident: N,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let func_id = ctx.func_id(file, &func.name, func.params.len());

    if let Some(frame) = ctx.frames.pop_if(|f| f.func_id == func_id) {
        let arg = frame.args.get(index).cloned();
        // The argument is the caller's expression: expand it in the caller's frame.
        let child = match arg {
            Some(a) => {
                let s = narrow(doc, func.params.get(index).copied(), ident, file, &a);
                expand(ctx, &Expr::Value(a.0, s), depth + 1)
            }
            None => {
                ctx.node_count += 1;
                Ok(unresolved(
                    ctx,
                    site,
                    format!("frame has no argument {index} for {func_id}"),
                ))
            }
        };
        ctx.frames.push(frame);
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child?], 0));
    }

    let Some(name) = doc
        .caps_within(
            vocab::FUNCTION_NAME,
            func.node.0.start_byte(),
            func.node.0.end_byte(),
        )
        .into_iter()
        .next()
    else {
        return Ok(unresolved(ctx, site, "function declaration has no name"));
    };
    let refs = ctx.references(file, doc.pos_of(name), false)?;

    let arity = func.params.len();
    let mut sites: Vec<ExprRef> = Vec::new();
    // An `-export` entry or a `fun f/1` value is a reference, not a caller.
    let mut strays: Vec<(PathBuf, Pos)> = Vec::new();
    let mut refs_seen = 0u32;
    for r in refs {
        if !ctx.in_root(&r.file) || ctx.reg.for_file(&r.file).is_none() {
            refs_seen += 1;
            continue;
        }
        let rdoc = ctx.doc(&r.file)?;
        if rdoc.covers(vocab::FUNCTION_NAME, r.range.start) {
            continue;
        }
        refs_seen += 1;
        let mut matched = false;
        for call in rdoc.calls_containing(r.range.start) {
            if !names(&rdoc.callee_text(&call), &func.name) || call.args.len() != arity {
                continue;
            }
            if let Some(a) = call.args.get(index) {
                sites.push((r.file.clone(), Span::of(*a)));
            }
            matched = true;
            break;
        }
        if !matched {
            strays.push((r.file.clone(), r.range.start));
        }
    }

    if sites.is_empty() {
        ctx.node_count += 1;
        let child = if refs_seen == 0 {
            let detail = format!("no call sites of {}/{arity}", func.name);
            stop(ctx, site, StopReason::EntryPoint, detail)
        } else {
            sort_local(&mut strays, file, |p| (p.line, p.col));
            strays.dedup();
            let dropped = fanout(ctx, &mut strays);
            let mut kids = Vec::new();
            for (f, p) in strays {
                kids.push(stray_stop(ctx, &f, p, &func.name)?);
            }
            let detail = format!(
                "{refs_seen} reference(s) to {}/{arity} are not call sites",
                func.name
            );
            let mut n = unresolved(ctx, site, detail);
            n.children = kids;
            n.truncated = dropped;
            n
        };
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child], 0));
    }

    sort_local(&mut sites, file, |s| s.start);
    sites.dedup();
    let dropped = fanout(ctx, &mut sites);
    let mut children = Vec::new();
    for a in sites {
        let s = narrow(doc, func.params.get(index).copied(), ident, file, &a);
        children.push(expand(ctx, &Expr::Value(a.0, s), depth + 1)?);
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

fn stray_stop(ctx: &mut Ctx, file: &Path, p: Pos, name: &str) -> Result<Node, TraceError> {
    let doc = ctx.doc(file)?;
    let (label, snippet) = match doc.ident_at(p) {
        Some(n) => (label_of(&doc, n), doc.line_of(n).to_string()),
        None => (name.to_string(), doc.line_at(p).to_string()),
    };
    ctx.node_count += 1;
    Ok(Node::stop(
        ctx.parent(),
        ctx.rel(file),
        Loc {
            file: file.to_path_buf(),
            line: p.line,
            col: p.col,
        },
        &label,
        &snippet,
        StopReason::Unresolved,
        "reference is not a call site",
        0,
    ))
}

/// Same file first, then by path and position: the nearest candidates survive the fan-out cut.
fn sort_local<T, K: Ord>(items: &mut [(PathBuf, T)], file: &Path, key: impl Fn(&T) -> K) {
    items.sort_by(|a, b| (a.0 != file, &a.0, key(&a.1)).cmp(&(b.0 != file, &b.0, key(&b.1))));
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
        return Ok(stop(ctx, site, StopReason::Literal, n.0.kind()));
    }
    if doc.is_opaque(n) {
        return Ok(unresolved(ctx, site, n.0.kind()));
    }
    if doc.has_cap(n, vocab::RETURN_CONTAINER) {
        return branch(ctx, doc, file, n, site, depth);
    }
    if let Some((container, field)) = doc.field_access(n) {
        let mut sources: Vec<Span> = field_sources(doc, n, container, &field)
            .iter()
            .map(|s| Span::of(*s))
            .collect();
        if !sources.is_empty() {
            // Every construction that reaches this use, never the last one alone (§5.5).
            let dropped = fanout(ctx, &mut sources);
            let mut children = Vec::new();
            for s in sources {
                children.push(expand(ctx, &Expr::Value(file.to_path_buf(), s), depth + 1)?);
            }
            return Ok(make(
                ctx,
                NodeKind::Field,
                site,
                Via::FieldSet,
                children,
                dropped,
            ));
        }
        let container_expr = if doc.has_cap(container, vocab::IDENT) {
            Expr::Ident(file.to_path_buf(), doc.pos_of(container))
        } else {
            Expr::Value(file.to_path_buf(), Span::of(container))
        };
        let mut child = expand(ctx, &container_expr, depth + 1)?;
        child.via = Some(Via::Field);
        let site = site.relabel(format!("{field} of {}", doc.text_of(container)));
        return Ok(make(
            ctx,
            NodeKind::Field,
            &site,
            Via::Field,
            vec![child],
            0,
        ));
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
            doc.pos_of(call.callee),
            &callee,
            args,
            file,
            site,
            depth,
        );
    }
    if doc.has_cap(n, vocab::CONSTRUCT) {
        return Ok(unresolved(
            ctx,
            site,
            format!("constructed value {}", n.0.kind()),
        ));
    }
    Ok(unresolved(ctx, site, n.0.kind()))
}

#[derive(PartialEq)]
struct Callee {
    file: PathBuf,
    name: String,
    arity: usize,
}

impl Callee {
    fn func_id(&self, ctx: &Ctx) -> String {
        ctx.func_id(&self.file, &self.name, self.arity)
    }
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
    let defs = distinct(&ctx.definition(file, name_pos)?);
    if defs.is_empty() {
        return Ok(unresolved(ctx, site, "callee not found"));
    }
    let outside = defs.iter().filter(|l| !ctx.in_root(&l.file)).count();
    if outside == defs.len() {
        return Ok(stop(ctx, site, StopReason::External, callee));
    }
    if outside > 0 {
        return Ok(unresolved(
            ctx,
            site,
            format!("{} definitions, some outside root", defs.len()),
        ));
    }

    // Distinct callees, not clauses: several definitions of one function collapse here.
    let mut targets: Vec<Callee> = Vec::new();
    for d in &defs {
        let doc = ctx.doc(&d.file)?;
        let Some(decl) = doc.function_at(d.range.start) else {
            return Ok(unresolved(
                ctx,
                site,
                format!("definition of {callee} is not a function"),
            ));
        };
        let t = Callee {
            file: d.file.clone(),
            name: decl.name.clone(),
            arity: decl.params.len(),
        };
        if !targets.contains(&t) {
            targets.push(t);
        }
    }

    let mut plan: Vec<(usize, Span)> = Vec::new();
    let mut recursive: Vec<usize> = Vec::new();
    for (i, t) in targets.iter().enumerate() {
        let func_id = t.func_id(ctx);
        if ctx
            .frames
            .iter()
            .any(|f| f.func_id == func_id && f.args == args)
        {
            recursive.push(i);
            continue;
        }
        let doc = ctx.doc(&t.file)?;
        let mut returns: Vec<Span> = doc
            .functions()
            .iter()
            .filter(|c| c.name == t.name && c.params.len() == t.arity)
            .flat_map(|c| doc.returns_of(c))
            .map(Span::of)
            .collect();
        returns.sort_by_key(|s| s.start);
        plan.extend(returns.into_iter().map(|s| (i, s)));
    }
    if plan.is_empty() && !recursive.is_empty() {
        let t = &targets[recursive[0]];
        return Ok(unresolved(
            ctx,
            site,
            format!("recursive call to {}/{}", t.name, t.arity),
        ));
    }

    let dropped = fanout(ctx, &mut plan);
    let mut children = Vec::new();
    for (i, s) in plan {
        let t = &targets[i];
        let frame = Frame {
            func_id: t.func_id(ctx),
            args: args.clone(),
        };
        let f = t.file.clone();
        ctx.frames.push(frame);
        let child = expand(ctx, &Expr::Value(f, s), depth + 1);
        ctx.frames.pop();
        children.push(child?);
    }
    for (nth, i) in recursive.into_iter().enumerate() {
        let t = &targets[i];
        let detail = format!("recursive call to {}/{}", t.name, t.arity);
        children.push(stop_nth(
            ctx,
            site,
            StopReason::Unresolved,
            detail,
            nth as u32,
        ));
        ctx.node_count += 1;
    }

    let site = site.relabel(callee.to_string());
    Ok(make(
        ctx,
        NodeKind::CallResult,
        &site,
        Via::Return,
        children,
        dropped,
    ))
}

fn field_sources<'d>(doc: &'d Doc<'_>, n: N<'d>, container: N<'d>, field: &str) -> Vec<N<'d>> {
    let Some(func) = doc.enclosing_function(n) else {
        return Vec::new();
    };
    let want = doc.text_of(container);
    let mut out = Vec::new();
    let (s, e) = (func.node.0.start_byte(), func.node.0.end_byte());
    for b in doc.caps_within(vocab::BINDING, s, e) {
        if b.0.start_byte() >= n.0.start_byte() {
            continue;
        }
        let Some((pattern, value)) = doc.binding_parts(b) else {
            continue;
        };
        if doc.text_of(pattern) != want || !doc.has_cap(value, vocab::CONSTRUCT) {
            continue;
        }
        if let Some(set) = doc.construct_field(value, field) {
            out.push(set);
        }
    }
    out
}

fn branch(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    n: N,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let mut tails: Vec<Span> = doc.tails_of(n).iter().map(|t| Span::of(*t)).collect();
    tails.sort_by_key(|s| s.start);
    tails.dedup();
    if tails.is_empty() {
        return Ok(unresolved(ctx, site, n.0.kind()));
    }
    let dropped = fanout(ctx, &mut tails);
    let mut children = Vec::new();
    for s in tails {
        let mut child = expand(ctx, &Expr::Value(file.to_path_buf(), s), depth + 1)?;
        child.via = Some(Via::Match);
        children.push(child);
    }
    let site = site.relabel(first_line_of(doc, n));
    Ok(make(
        ctx,
        NodeKind::Branch,
        &site,
        Via::Match,
        children,
        dropped,
    ))
}
