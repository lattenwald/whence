//! The step function of spec §5.2: what feeds this expression?

use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::host::{HostError, Location};
use crate::lang::vocab;
use crate::pos::Pos;
use crate::syntax::{CallSite, Doc, FnDecl, N, Proj, Role, Slot, Span, index_containing};
use crate::trace::TraceError;
use crate::trace::frame::{Ctx, ExprRef, Frame, FuncId};
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
    let outcome = Node::construct(kind);
    let id = node_id(
        ctx.parent(),
        &site.rel,
        site.loc.line,
        site.loc.col,
        &outcome,
        0,
    );
    Node {
        id,
        outcome,
        label: site.label.clone(),
        loc: site.loc.clone(),
        via: Some(via),
        snippet: site.snippet.clone(),
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

fn no_language(ctx: &Ctx, file: &Path) -> String {
    format!("no language for {}", ctx.rel(file).display())
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
    let node = match doc.through(node) {
        Some(inner) if doc.has_cap(inner, vocab::IDENT) => inner,
        _ => node,
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
    let locs = distinct(&ctx.definition(file, site.pos())?);
    let mut defs: Vec<(PathBuf, Pos)> = locs
        .iter()
        .map(|l| (l.file.clone(), l.range.start))
        .collect();
    if defs.is_empty() {
        return Ok(unresolved(ctx, site, "no definition from language server"));
    }
    let doc = ctx.doc(file)?;
    let mut occ = if doc.single_assignment() {
        Vec::new()
    } else {
        occurrences(ctx, &doc, file, site, &locs)?
    };

    if occ.is_empty() && defs.len() == 1 {
        return definition(ctx, &defs[0].0, defs[0].1, site, depth);
    }

    let mut children = Vec::new();
    let (dropped, collapsed) = if occ.is_empty() {
        sort_local(&mut defs, file, |p| (p.line, p.col));
        candidates(ctx, site, &mut defs, "definitions")
    } else {
        occ.sort_by_key(|o| std::cmp::Reverse(o.occurrence().start));
        let cut = candidates(ctx, site, &mut occ, "writes");
        for o in occ {
            children.push(expand_occurrence(ctx, file, o, depth)?);
        }
        children.extend(cut.1);
        (cut.0, None)
    };
    // Past the fan-out cut when occurrences took it, so the binding stays visible.
    for (f, p) in &defs {
        let mut child = definition(ctx, f, *p, site, depth)?;
        child.via = Some(Via::Match);
        children.push(child);
    }
    children.extend(collapsed);
    Ok(make(
        ctx,
        NodeKind::Branch,
        site,
        Via::Match,
        children,
        dropped,
    ))
}

enum Occ {
    Write {
        occurrence: Span,
        assign: Span,
        /// The `@assign.target`, `@through` already applied.
        target: Span,
        value: Option<Span>,
        /// The write lands on the occurrence itself, not on a field or element of it.
        whole: bool,
        via: Via,
    },
    Escape {
        occurrence: Span,
        detail: String,
    },
}

impl Occ {
    fn occurrence(&self) -> Span {
        match self {
            Occ::Write { occurrence, .. } | Occ::Escape { occurrence, .. } => *occurrence,
        }
    }
}

/// Everything that may have written the variable between its binding and this use (spec §3.1).
fn occurrences(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    site: &Site,
    defs: &[Location],
) -> Result<Vec<Occ>, TraceError> {
    if defs.iter().any(|d| d.file != file) {
        return Ok(Vec::new());
    }
    let use_pos = site.pos();
    let (Some(use_off), Some(func)) = (
        doc.byte_offset(use_pos),
        doc.ident_at(use_pos)
            .and_then(|u| doc.enclosing_function_node(u))
            .map(|f| f.0.id()),
    ) else {
        return Ok(Vec::new());
    };
    let Some(def_off) = defs
        .iter()
        .filter_map(|d| doc.byte_offset(d.range.start))
        .min()
    else {
        return Ok(Vec::new());
    };

    let positions = ctx.occurrences(file, use_pos)?;
    ctx.remember_symbol(file, defs, &positions);
    let mut out = Vec::new();
    for p in positions {
        if defs.iter().any(|d| d.range.start == p) {
            continue;
        }
        let Some(off) = doc.byte_offset(p) else {
            continue;
        };
        if off <= def_off || off >= use_off {
            continue;
        }
        let Some(o) = doc.ident_at(p) else {
            continue;
        };
        if doc.enclosing_function_node(o).map(|f| f.0.id()) != Some(func) {
            continue;
        }
        if let Some(occ) = classify(ctx, doc, file, o)? {
            out.push(occ);
        }
    }
    Ok(out)
}

fn classify(ctx: &mut Ctx, doc: &Doc, file: &Path, o: N) -> Result<Option<Occ>, TraceError> {
    if let Some(a) = doc.assign_at(o) {
        let target = doc.through(a.target).unwrap_or(a.target);
        let place = place_written(doc, target, o);
        // Reads that live inside the target: the `i` of `x[i]`, the key of `m[k]`.
        if !doc
            .place_chain(place)
            .iter()
            .any(|p| Span::of(*p) == Span::of(o))
        {
            return Ok(None);
        }
        let whole = Span::of(place) == Span::of(o);
        if let Some(proj) = ctx.proj.last()
            && !whole
            && !sets_projection(doc, place, o, proj)
        {
            return Ok(None);
        }
        return Ok(Some(Occ::Write {
            occurrence: Span::of(o),
            assign: Span::of(a.node),
            target: Span::of(target),
            value: a.value.map(Span::of),
            whole,
            via: if whole && !a.compound {
                Via::Rebind
            } else {
                Via::Mutation
            },
        }));
    }
    let using = doc.call_using(o);
    if let Some(escape) = doc.escaped(o) {
        let what = match &using {
            Some((call, _)) => first_line_of(doc, call.node),
            None => clip(doc.line_of(escape).trim()),
        };
        return Ok(Some(Occ::Escape {
            occurrence: Span::of(o),
            detail: format!("may be written by {what}"),
        }));
    }
    let Some((call, slot)) = using else {
        return Ok(None);
    };
    let callee = doc.callee_text(&call);
    let defs = distinct(&ctx.definition(file, doc.pos_of(call.callee))?);
    if defs.is_empty() {
        return Ok(None);
    }
    if defs.iter().all(|d| !ctx.in_root(&d.file)) {
        // A value passed to unknown code is no evidence of a write; a receiver is.
        if !matches!(slot, Slot::Receiver) {
            return Ok(None);
        }
        return Ok(Some(Occ::Escape {
            occurrence: Span::of(o),
            detail: format!("may be written by external method {callee}"),
        }));
    }
    let mut mutable = false;
    for d in &defs {
        if !ctx.in_root(&d.file) {
            continue;
        }
        let Some(cdoc) = ctx.doc_if_known(&d.file)? else {
            continue;
        };
        let Some(decl) = cdoc.declares_function(d.range.start) else {
            continue;
        };
        mutable |= match declared_slot(&decl, &call, slot) {
            Slot::Receiver => cdoc.has_mutable_receiver(&decl),
            Slot::Arg(i) => cdoc.param_is_mutable(&decl, i),
        };
    }
    Ok(mutable.then(|| Occ::Escape {
        occurrence: Span::of(o),
        detail: format!("may be written by {callee}(…)"),
    }))
}

/// The one target of a multi-target write that `o` belongs to: `a, b = b, a` writes `a` alone.
fn place_written<'t>(doc: &'t Doc<'_>, target: N<'t>, o: N<'t>) -> N<'t> {
    doc.positional(target)
        .and_then(|es| index_containing(&es, o).map(|i| es[i]))
        .unwrap_or(target)
}

fn sets_projection(doc: &Doc, place: N, o: N, proj: &Proj) -> bool {
    doc.field_access(place)
        .is_some_and(|(container, field)| Span::of(container) == Span::of(o) && &field == proj)
}

/// The part of `value` the pattern binds to `ident`, else `value` under its element index.
fn expand_pattern<'t>(
    ctx: &mut Ctx,
    doc: &'t Doc<'_>,
    file: &Path,
    pattern: N<'t>,
    ident: N<'t>,
    value: N<'t>,
    depth: u32,
) -> Result<Node, TraceError> {
    let (source, index) = match doc.destructure(pattern, ident, value) {
        Some(s) => (s, None),
        None => (value, doc.pattern_index(pattern, ident)),
    };
    if let Some(i) = index {
        ctx.proj.push(Proj::Index(i));
    }
    let child = expand(
        ctx,
        &Expr::Value(file.to_path_buf(), Span::of(source)),
        depth + 1,
    );
    if index.is_some() {
        ctx.proj.pop();
    }
    child
}

fn expand_occurrence(ctx: &mut Ctx, file: &Path, occ: Occ, depth: u32) -> Result<Node, TraceError> {
    let doc = ctx.doc(file)?;
    let o = doc
        .node(occ.occurrence())
        .expect("the occurrence was found in this document");
    let osite = Site::new(ctx, &doc, file, o);
    let (assign, target, value, whole, via) = match occ {
        Occ::Escape { detail, .. } => {
            ctx.node_count += 1;
            return Ok(unresolved(ctx, &osite, detail));
        }
        Occ::Write {
            assign,
            target,
            value,
            whole,
            via,
            ..
        } => (assign, target, value, whole, via),
    };
    let found = "classify resolved these spans in this document";
    let child = match value {
        None => {
            let a = doc.node(assign).expect(found);
            let asite = Site::new(ctx, &doc, file, a);
            ctx.node_count += 1;
            stop(ctx, &asite, StopReason::Literal, a.0.kind())
        }
        Some(v) => {
            // `o.f = v` sets the pending projection: the child is the value, the projection consumed.
            let pending = if whole { None } else { ctx.proj.pop() };
            let field_set = pending.is_some();
            let child = expand_pattern(
                ctx,
                &doc,
                file,
                doc.node(target).expect(found),
                o,
                doc.node(v).expect(found),
                depth,
            );
            ctx.proj.extend(pending);
            let mut child = child?;
            if field_set {
                child.via = Some(Via::FieldSet);
            }
            child
        }
    };
    Ok(make(ctx, NodeKind::Binding, &osite, via, vec![child], 0))
}

fn definition(
    ctx: &mut Ctx,
    def_file: &Path,
    def_pos: Pos,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    if !ctx.in_root(def_file) {
        return Ok(stop(ctx, site, StopReason::External, site.label.clone()));
    }
    let Some(doc) = ctx.doc_if_known(def_file)? else {
        return Ok(unresolved(ctx, site, no_language(ctx, def_file)));
    };
    let Some(ident) = doc.ident_at(def_pos) else {
        return Ok(unresolved(ctx, site, "definition is not an identifier"));
    };
    let dsite = Site::new(ctx, &doc, def_file, ident);

    match doc.role_of(ident) {
        Role::Declared => Ok(unresolved(ctx, &dsite, "declared without a value")),
        Role::BoundBy { pattern, value }
        | Role::BranchPattern {
            pattern,
            subject: value,
        } => {
            let child = expand_pattern(ctx, &doc, def_file, pattern, ident, value, depth)?;
            let child = matched(child);
            Ok(make(
                ctx,
                NodeKind::Binding,
                &dsite,
                Via::Match,
                vec![child],
                0,
            ))
        }
        Role::ElementOf { value } => {
            let mut child = expand(
                ctx,
                &Expr::Value(def_file.to_path_buf(), Span::of(value)),
                depth + 1,
            )?;
            child.via = Some(Via::Element);
            Ok(make(
                ctx,
                NodeKind::Binding,
                &dsite,
                Via::Match,
                vec![child],
                0,
            ))
        }
        Role::Param { func, slot } => {
            param_like(ctx, &doc, def_file, &func, slot, ident, &dsite, depth)
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
    if n.outcome.kind() == Some(&NodeKind::Param) && n.via == Some(Via::Arg) {
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

/// A method can be called as a plain function with the receiver first (Go `T.M(s, x)`).
fn receiver_shift(func: &FnDecl, has_receiver: bool, argc: usize) -> bool {
    func.receiver.is_some() && !has_receiver && argc == func.params.len() + 1
}

/// The receiver and arguments of a call as the declaration numbers them.
fn as_declared<T>(func: &FnDecl, receiver: Option<T>, mut args: Vec<T>) -> (Option<T>, Vec<T>) {
    if receiver_shift(func, receiver.is_some(), args.len()) {
        return (Some(args.remove(0)), args);
    }
    (receiver, args)
}

fn declared_slot(func: &FnDecl, call: &CallSite, slot: Slot) -> Slot {
    match slot {
        Slot::Arg(i) if receiver_shift(func, call.receiver.is_some(), call.args.len()) => match i {
            0 => Slot::Receiver,
            i => Slot::Arg(i - 1),
        },
        s => s,
    }
}

fn pick<T: Clone>(slot: Slot, receiver: &Option<T>, args: &[T]) -> Option<T> {
    match slot {
        Slot::Arg(i) => args.get(i).cloned(),
        Slot::Receiver => receiver.clone(),
    }
}

#[allow(clippy::too_many_arguments)]
fn param_like(
    ctx: &mut Ctx,
    doc: &Doc,
    file: &Path,
    func: &FnDecl,
    slot: Slot,
    ident: N,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let func_id = FuncId {
        file: file.to_path_buf(),
        group: doc.function_group(func),
        name: func.name.clone(),
        arity: func.params.len(),
    };

    if let Some(frame) = ctx.frames.pop_if(|f| f.func_id == func_id) {
        let arg = pick(slot, &frame.receiver, &frame.args);
        // The argument is the caller's expression: expand it in the caller's frame.
        let child = match arg {
            Some(a) => {
                let s = narrow(
                    doc,
                    pick(slot, &func.receiver, &func.params),
                    ident,
                    file,
                    &a,
                );
                expand(ctx, &Expr::Value(a.0, s), depth + 1)
            }
            None => {
                ctx.node_count += 1;
                let what = match slot {
                    Slot::Arg(i) => format!("argument {i}"),
                    Slot::Receiver => "receiver".to_string(),
                };
                Ok(unresolved(
                    ctx,
                    site,
                    format!("frame has no {what} for {}", func_id.describe(ctx)),
                ))
            }
        };
        ctx.frames.push(frame);
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child?], 0));
    }

    let Some(name) = doc.name_node(func) else {
        return Ok(unresolved(ctx, site, "function declaration has no name"));
    };
    let refs = ctx.references(file, doc.pos_of(name), false)?;

    let arity = func.params.len();
    let mut sites: Vec<ExprRef> = Vec::new();
    // An `-export` entry or a `fun f/1` value is a reference, not a caller.
    let mut strays: Vec<(PathBuf, Pos, String)> = Vec::new();
    let mut refs_seen = 0u32;
    for r in refs {
        let rdoc = if ctx.in_root(&r.file) {
            ctx.doc_if_known(&r.file)?
        } else {
            None
        };
        let Some(rdoc) = rdoc else {
            refs_seen += 1;
            continue;
        };
        if rdoc.declares_function(r.range.start).is_some() {
            continue;
        }
        refs_seen += 1;
        // The server already tied the reference to this function; what is left is
        // structural: is it the callee of a call, and does that call pass this argument?
        match rdoc.call_with_callee_at(r.range.start) {
            Some(call) => {
                let (receiver, args) = as_declared(func, call.receiver, call.args);
                let picked = pick(slot, &receiver, &args);
                match picked {
                    Some(a) => sites.push((r.file.clone(), Span::of(a))),
                    None => strays.push((
                        r.file.clone(),
                        r.range.start,
                        match slot {
                            Slot::Arg(i) => format!("call has no argument {}", i + 1),
                            Slot::Receiver => "call has no receiver".to_string(),
                        },
                    )),
                }
            }
            None => strays.push((
                r.file.clone(),
                r.range.start,
                "reference is not a call site".to_string(),
            )),
        }
    }

    if sites.is_empty() {
        ctx.node_count += 1;
        let child = if refs_seen == 0 {
            let detail = format!("no call sites of {}/{arity}", func.name);
            stop(ctx, site, StopReason::EntryPoint, detail)
        } else {
            strays.sort_by(|a, b| {
                (a.0 != file, &a.0, a.1.line, a.1.col).cmp(&(b.0 != file, &b.0, b.1.line, b.1.col))
            });
            strays.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
            let (dropped, collapsed) = candidates(ctx, site, &mut strays, "references");
            let mut kids = Vec::new();
            for (f, p, detail) in strays {
                kids.push(stray_stop(ctx, &f, p, &func.name, detail)?);
            }
            let detail = format!(
                "{refs_seen} reference(s) to {}/{arity} are not call sites",
                func.name
            );
            let mut n = unresolved(ctx, site, detail);
            n.children = if collapsed.is_some() {
                Vec::new()
            } else {
                kids
            };
            n.truncated = dropped;
            n
        };
        return Ok(make(ctx, NodeKind::Param, site, Via::Arg, vec![child], 0));
    }

    sort_local(&mut sites, file, |s| s.start);
    sites.dedup();
    let (dropped, collapsed) = candidates(ctx, site, &mut sites, "call sites");
    let mut children = Vec::new();
    for a in sites {
        let s = narrow(
            doc,
            pick(slot, &func.receiver, &func.params),
            ident,
            file,
            &a,
        );
        children.push(expand(ctx, &Expr::Value(a.0, s), depth + 1)?);
    }
    children.extend(collapsed);
    Ok(make(
        ctx,
        NodeKind::Param,
        site,
        Via::Arg,
        children,
        dropped,
    ))
}

fn stray_stop(
    ctx: &mut Ctx,
    file: &Path,
    p: Pos,
    name: &str,
    detail: String,
) -> Result<Node, TraceError> {
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
        detail,
        0,
    ))
}

/// Same file first, then by path and position: the nearest candidates survive the fan-out cut.
fn sort_local<T, K: Ord>(items: &mut [(PathBuf, T)], file: &Path, key: impl Fn(&T) -> K) {
    items.sort_by(|a, b| (a.0 != file, &a.0, key(&a.1)).cmp(&(b.0 != file, &b.0, key(&b.1))));
}

/// Every fork of the tree passes here, so `split` is honoured everywhere.
fn candidates<T>(
    ctx: &mut Ctx,
    site: &Site,
    items: &mut Vec<T>,
    what: &str,
) -> (u32, Option<Node>) {
    if !ctx.limits.split && items.len() > 1 {
        let n = items.len() as u32;
        items.clear();
        ctx.truncated += n;
        ctx.node_count += 1;
        let stop = unresolved(ctx, site, format!("{n} candidates: {what}"));
        return (n, Some(stop));
    }
    let keep = ctx.limits.fanout as usize;
    if items.len() <= keep {
        return (0, None);
    }
    let dropped = (items.len() - keep) as u32;
    items.truncate(keep);
    ctx.truncated += dropped;
    (dropped, None)
}

fn value<'t>(
    ctx: &mut Ctx,
    doc: &'t Doc<'_>,
    file: &Path,
    n: N<'t>,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let n = doc.through(n).unwrap_or(n);
    if let Some(proj) = ctx.proj.last().cloned()
        && doc.has_cap(n, vocab::CONSTRUCT)
    {
        return project(ctx, doc, file, n, site, &proj, depth);
    }
    if doc.is_literal(n) {
        if let Some(proj) = ctx.proj.last() {
            return Ok(unresolved(
                ctx,
                site,
                format!("no {} in a literal", proj.describe()),
            ));
        }
        return Ok(stop(ctx, site, StopReason::Literal, n.0.kind()));
    }
    if doc.is_opaque(n) {
        return Ok(unresolved(ctx, site, n.0.kind()));
    }
    if doc.has_cap(n, vocab::RETURN_CONTAINER) {
        return branch(ctx, doc, file, n, site, depth);
    }
    if let Some((container, field)) = doc.field_access(n) {
        let container_expr = if doc.has_cap(container, vocab::IDENT) {
            Expr::Ident(file.to_path_buf(), doc.pos_of(container))
        } else {
            Expr::Value(file.to_path_buf(), Span::of(container))
        };
        ctx.proj.push(field.clone());
        let child = expand(ctx, &container_expr, depth + 1);
        ctx.proj.pop();
        let mut child = child?;
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
        let receiver = call.receiver.map(|r| (file.to_path_buf(), Span::of(r)));
        return call_result(
            ctx,
            doc.pos_of(call.callee),
            &callee,
            args,
            receiver,
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

enum Halt {
    /// A `stop: unresolved` with this detail, built by the caller: it holds the site.
    Stop(String),
    Fail(TraceError),
}

impl From<TraceError> for Halt {
    fn from(e: TraceError) -> Halt {
        Halt::Fail(e)
    }
}

/// What each declaration already expanded to; `None` while its own expansion is in flight.
type Expansions = Vec<(Location, Option<Vec<Location>>)>;

/// An abstract declaration is not a callee: its implementations are, and an implementation
/// may be abstract in turn (an interface embedding the method), so this runs to a fixpoint.
fn implementations_of(
    ctx: &mut Ctx,
    defs: &[Location],
    done: &mut Expansions,
) -> Result<Vec<Location>, Halt> {
    let mut out: Vec<Location> = Vec::new();
    for d in defs {
        if let Some((_, prior)) = done.iter().find(|(l, _)| l == d) {
            out.extend(prior.clone().unwrap_or_default());
            continue;
        }
        done.push((d.clone(), None));
        let here = implementations_at(ctx, d, done)?;
        if let Some((_, slot)) = done.iter_mut().find(|(l, _)| l == d) {
            *slot = Some(here.clone());
        }
        out.extend(here);
    }
    Ok(distinct(&out))
}

/// The callees `d` stands for: itself, unless it is an abstract declaration.
fn implementations_at(
    ctx: &mut Ctx,
    d: &Location,
    done: &mut Expansions,
) -> Result<Vec<Location>, Halt> {
    if !ctx.in_root(&d.file) {
        return Ok(vec![d.clone()]);
    }
    let Some(doc) = ctx.doc_if_known(&d.file)? else {
        return Ok(vec![d.clone()]);
    };
    let Some(abs) = doc
        .declares_function(d.range.start)
        .filter(|f| doc.is_abstract(f))
    else {
        return Ok(vec![d.clone()]);
    };
    let Some(name) = doc.name_node(&abs) else {
        return Err(Halt::Stop("function declaration has no name".into()));
    };
    let decl_pos = doc.pos_of(name);
    let impls = match ctx.implementation(&d.file, decl_pos) {
        Ok(impls) => impls,
        Err(TraceError::Host(HostError::Unsupported(_))) => {
            return Err(Halt::Stop(format!("abstract method {}", abs.name)));
        }
        Err(e) => return Err(Halt::Fail(e)),
    };
    let mut here = implementations_of(ctx, &impls, done)?;
    if abs.body.is_some() {
        here.push(d.clone());
    }
    if here.is_empty() {
        return Err(Halt::Stop(format!("no implementation of {}", abs.name)));
    }
    Ok(here)
}

#[allow(clippy::too_many_arguments)]
fn call_result(
    ctx: &mut Ctx,
    name_pos: Pos,
    callee: &str,
    args: Vec<ExprRef>,
    receiver: Option<ExprRef>,
    file: &Path,
    site: &Site,
    depth: u32,
) -> Result<Node, TraceError> {
    let defs = distinct(&ctx.definition(file, name_pos)?);
    if defs.is_empty() {
        return Ok(unresolved(ctx, site, "callee not found"));
    }
    let defs = match implementations_of(ctx, &defs, &mut Vec::new()) {
        Ok(defs) => defs,
        Err(Halt::Stop(detail)) => return Ok(unresolved(ctx, site, detail)),
        Err(Halt::Fail(e)) => return Err(e),
    };

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
    let mut targets: Vec<Frame> = Vec::new();
    for d in &defs {
        let Some(doc) = ctx.doc_if_known(&d.file)? else {
            return Ok(unresolved(ctx, site, no_language(ctx, &d.file)));
        };
        let Some(decl) = doc.declares_function(d.range.start) else {
            return Ok(unresolved(
                ctx,
                site,
                format!("definition of {callee} is not a function"),
            ));
        };
        let func_id = FuncId {
            file: d.file.clone(),
            group: doc.function_group(&decl),
            name: decl.name.clone(),
            arity: decl.params.len(),
        };
        if targets.iter().any(|t| t.func_id == func_id) {
            continue;
        }
        let (receiver, args) = as_declared(&decl, receiver.clone(), args.clone());
        targets.push(Frame {
            func_id,
            args,
            receiver,
        });
    }

    let mut plan: Vec<(usize, Span)> = Vec::new();
    let mut recursive: Vec<usize> = Vec::new();
    for (i, t) in targets.iter().enumerate() {
        if ctx.frames.contains(t) {
            recursive.push(i);
            continue;
        }
        let doc = ctx.doc(&t.func_id.file)?;
        let mut returns: Vec<Span> = doc
            .clauses_of(t.func_id.group, &t.func_id.name, t.func_id.arity)
            .iter()
            .flat_map(|c| doc.returns_of(c))
            .map(Span::of)
            .collect();
        returns.sort_by_key(|s| s.start);
        plan.extend(returns.into_iter().map(|s| (i, s)));
    }
    if plan.is_empty() && !recursive.is_empty() {
        let t = &targets[recursive[0]].func_id;
        return Ok(unresolved(
            ctx,
            site,
            format!("recursive call to {}/{}", t.name, t.arity),
        ));
    }

    let mut all: Vec<Option<(usize, Span)>> = plan.into_iter().map(Some).collect();
    all.extend(recursive.iter().map(|_| None));
    let (dropped, collapsed) = candidates(ctx, site, &mut all, "return expressions");
    let plan: Vec<(usize, Span)> = all.iter().flatten().copied().collect();
    let recursive: Vec<usize> = if all.is_empty() {
        Vec::new()
    } else {
        recursive
    };
    let mut children = Vec::new();
    for group in plan.chunk_by(|a, b| a.0 == b.0) {
        let t = &targets[group[0].0];
        let f = t.func_id.file.clone();
        ctx.frames.push(t.clone());
        let expanded: Result<Vec<Node>, TraceError> = group
            .iter()
            .map(|(_, s)| expand(ctx, &Expr::Value(f.clone(), *s), depth + 1))
            .collect();
        ctx.frames.pop();
        children.extend(expanded?);
    }
    for (nth, i) in recursive.into_iter().enumerate() {
        let t = &targets[i].func_id;
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
    children.extend(collapsed);

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

fn project<'t>(
    ctx: &mut Ctx,
    doc: &'t Doc<'_>,
    file: &Path,
    n: N<'t>,
    site: &Site,
    proj: &Proj,
    depth: u32,
) -> Result<Node, TraceError> {
    let selected = match proj {
        Proj::Field(f) => doc.construct_field(n, f),
        Proj::Index(i) => doc.construct_element(n, *i),
    };
    if let Some(value) = selected {
        let pending = ctx.proj.pop();
        let child = expand(
            ctx,
            &Expr::Value(file.to_path_buf(), Span::of(value)),
            depth + 1,
        );
        ctx.proj.extend(pending);
        let mut child = child?;
        child.via = Some(Via::FieldSet);
        return Ok(child);
    }
    if let Some(base) = doc.construct_base(n) {
        let mut child = expand(
            ctx,
            &Expr::Value(file.to_path_buf(), Span::of(base)),
            depth + 1,
        )?;
        child.via = Some(Via::Field);
        return Ok(child);
    }
    Ok(unresolved(
        ctx,
        site,
        format!("no {} in this {}", proj.describe(), n.0.kind()),
    ))
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
    let (dropped, collapsed) = candidates(ctx, site, &mut tails, "branch tails");
    let mut children = Vec::new();
    for s in tails {
        let mut child = expand(ctx, &Expr::Value(file.to_path_buf(), s), depth + 1)?;
        child.via = Some(Via::Match);
        children.push(child);
    }
    children.extend(collapsed);
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
