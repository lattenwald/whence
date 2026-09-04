//! Structural questions over a parsed document, answered only through the
//! capture vocabulary in [`crate::lang::vocab`]: no grammar names live here.

use crate::lang::{Language, vocab};
use crate::pos::{Lines, Pos};
use std::collections::HashSet;
use std::path::PathBuf;
use tree_sitter::StreamingIterator;

/// Names a node across borrows: `tree_sitter` node ids are valid for one tree borrow only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind_id: u16,
}

impl Span {
    pub fn of(n: N) -> Span {
        Span {
            start: n.0.start_byte(),
            end: n.0.end_byte(),
            kind_id: n.0.kind_id(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Cap {
    cap: u32,
    span: Span,
}

#[derive(Clone, Copy)]
pub struct N<'t>(pub tree_sitter::Node<'t>);

pub struct FnDecl<'t> {
    pub node: N<'t>,
    pub name: String,
    pub params: Vec<N<'t>>,
    pub body: Option<N<'t>>,
    pub receiver: Option<N<'t>>,
}

pub struct CallSite<'t> {
    pub node: N<'t>,
    pub callee: N<'t>,
    pub args: Vec<N<'t>>,
    pub receiver: Option<N<'t>>,
}

#[derive(Clone, Copy)]
pub enum Slot {
    Arg(usize),
    Receiver,
}

pub struct Assign<'t> {
    pub node: N<'t>,
    pub target: N<'t>,
    pub value: Option<N<'t>>,
    pub compound: bool,
}

pub enum Role<'t> {
    BoundBy {
        pattern: N<'t>,
        value: N<'t>,
    },
    /// A binding with no value. `literal` when the language marks the declaration
    /// `@literal` (Go's zero value); otherwise the variable is assigned later.
    Declared {
        literal: bool,
    },
    /// A loop pattern: the value is the iterable, not the element.
    ElementOf {
        value: N<'t>,
    },
    Param {
        func: FnDecl<'t>,
        slot: Slot,
    },
    BranchPattern {
        pattern: N<'t>,
        subject: N<'t>,
    },
    Opaque(N<'t>),
    Use,
}

pub struct Doc<'l> {
    pub path: PathBuf,
    pub text: String,
    pub tree: tree_sitter::Tree,
    lang: &'l Language,
    lines: Lines,
    caps: Vec<Cap>,
    set: HashSet<Cap>,
}

impl<'l> Doc<'l> {
    pub fn parse(lang: &'l Language, path: PathBuf, text: String) -> Doc<'l> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&lang.ts)
            .expect("embedded grammar matches the linked tree-sitter");
        let tree = parser
            .parse(&text, None)
            .expect("parser has a language and no cancellation flag");

        let mut caps = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut it = cursor.matches(&lang.query, tree.root_node(), text.as_bytes());
        while let Some(m) = it.next() {
            for c in m.captures() {
                // comments are named nodes, so a wildcard pattern captures them too
                if c.node.is_extra() {
                    continue;
                }
                caps.push(Cap {
                    cap: c.index,
                    span: Span::of(N(c.node)),
                });
            }
        }
        caps.sort_by_key(|c| (c.span.start, c.span.end, c.cap));
        caps.dedup();
        let set = caps.iter().copied().collect();

        Doc {
            path,
            lines: Lines::new(&text),
            text,
            tree,
            lang,
            caps,
            set,
        }
    }

    fn cap_index(&self, name: &str) -> Option<u32> {
        self.lang.query.capture_index_for_name(name)
    }

    pub fn node(&self, span: Span) -> Option<N<'_>> {
        let mut n = self
            .tree
            .root_node()
            .descendant_for_byte_range(span.start, span.end)?;
        loop {
            if Span::of(N(n)) == span {
                return Some(N(n));
            }
            n = n.parent()?;
        }
    }

    pub fn caps_within(&self, cap: &str, start: usize, end: usize) -> Vec<N<'_>> {
        let Some(idx) = self.cap_index(cap) else {
            return Vec::new();
        };
        let from = self.caps.partition_point(|c| c.span.start < start);
        self.caps[from..]
            .iter()
            .take_while(|c| c.span.start < end)
            .filter(|c| c.cap == idx && c.span.end <= end)
            .filter_map(|c| self.node(c.span))
            .collect()
    }

    fn caps_containing(&self, cap: &str, off: usize) -> Vec<Cap> {
        let Some(idx) = self.cap_index(cap) else {
            return Vec::new();
        };
        let mut hits: Vec<Cap> = self
            .caps
            .iter()
            .copied()
            .filter(|c| c.cap == idx && c.span.start <= off && off < c.span.end)
            .collect();
        hits.sort_by_key(|c| c.span.end - c.span.start);
        hits
    }

    pub fn covers(&self, cap: &str, p: Pos) -> bool {
        self.byte_offset(p)
            .is_some_and(|off| !self.caps_containing(cap, off).is_empty())
    }

    fn caps_owned_by<'a>(
        &'a self,
        cap: &str,
        owner_caps: &[&str],
        stop: &[&str],
        owner: N<'a>,
    ) -> Vec<N<'a>> {
        self.caps_within(cap, owner.0.start_byte(), owner.0.end_byte())
            .into_iter()
            .filter(|n| {
                self.climb(n.0.parent(), owner_caps, stop)
                    .is_some_and(|a| a.0.id() == owner.0.id())
            })
            .collect()
    }

    /// Walks up from `from` inclusive; a node carrying both `want` and `stop` is wanted.
    fn climb<'t>(
        &self,
        from: Option<tree_sitter::Node<'t>>,
        want: &[&str],
        stop: &[&str],
    ) -> Option<N<'t>> {
        let mut cur = from;
        while let Some(c) = cur {
            if want.iter().any(|cap| self.has_cap(N(c), cap)) {
                return Some(N(c));
            }
            if stop.iter().any(|cap| self.has_cap(N(c), cap)) {
                return None;
            }
            cur = c.parent();
        }
        None
    }

    /// Stops at the enclosing body: an occurrence is not matched to a construct outside its function.
    fn nearest_ancestor_with_or_self<'a>(&'a self, n: N<'a>, cap: &str) -> Option<N<'a>> {
        self.climb(Some(n.0), &[cap], &[vocab::FUNCTION_BODY])
    }

    pub fn caps_child_of<'a>(&'a self, cap: &str, parent: N<'a>) -> Vec<N<'a>> {
        self.caps_within(cap, parent.0.start_byte(), parent.0.end_byte())
            .into_iter()
            .filter(|n| n.0.parent().is_some_and(|p| p.id() == parent.0.id()))
            .collect()
    }

    pub fn has_cap(&self, n: N, cap: &str) -> bool {
        let Some(idx) = self.cap_index(cap) else {
            return false;
        };
        self.set.contains(&Cap {
            cap: idx,
            span: Span::of(n),
        })
    }

    pub fn text_of(&self, n: N) -> &str {
        &self.text[n.0.start_byte()..n.0.end_byte()]
    }

    pub fn line_of(&self, n: N) -> &str {
        self.lines.line_text(&self.text, n.0.start_byte())
    }

    pub fn line_at(&self, p: Pos) -> &str {
        self.lines
            .start(p.line)
            .map_or("", |b| self.lines.line_text(&self.text, b))
    }

    pub fn pos_of(&self, n: N) -> Pos {
        self.pos_at(n.0.start_byte())
    }

    pub fn pos_at(&self, byte: usize) -> Pos {
        self.lines.pos_of(&self.text, byte)
    }

    pub fn byte_offset(&self, p: Pos) -> Option<usize> {
        self.lines.byte_offset(&self.text, p)
    }

    /// The innermost node carrying `cap` at `p`.
    fn node_at(&self, cap: &str, p: Pos) -> Option<N<'_>> {
        let off = self.byte_offset(p)?;
        self.caps_containing(cap, off)
            .first()
            .and_then(|c| self.node(c.span))
    }

    pub fn ident_at(&self, p: Pos) -> Option<N<'_>> {
        self.node_at(vocab::IDENT, p)
    }

    /// Every clause in the file: multi-clause functions are separate `@function` matches.
    pub fn functions(&self) -> Vec<FnDecl<'_>> {
        self.caps_within(vocab::FUNCTION, 0, self.text.len())
            .into_iter()
            .filter_map(|f| self.fn_decl(f))
            .collect()
    }

    /// Start byte of the nearest `@function.group` ancestor of `f`, else of `f` itself:
    /// the key that puts a function's clauses together and keeps same-name functions apart.
    pub fn function_group(&self, f: &FnDecl) -> usize {
        self.climb(f.node.0.parent(), &[vocab::FUNCTION_GROUP], &[])
            .map_or(f.node.0.start_byte(), |g| g.0.start_byte())
    }

    pub fn clauses_of(&self, group: usize, name: &str, arity: usize) -> Vec<FnDecl<'_>> {
        self.functions()
            .into_iter()
            .filter(|c| {
                c.name == name && c.params.len() == arity && self.function_group(c) == group
            })
            .collect()
    }

    fn function_name_at(&self, p: Pos) -> Option<N<'_>> {
        self.node_at(vocab::FUNCTION_NAME, p)
    }

    pub fn declares_function(&self, p: Pos) -> Option<FnDecl<'_>> {
        self.enclosing_function(self.function_name_at(p)?)
    }

    pub fn is_abstract(&self, f: &FnDecl) -> bool {
        self.has_cap(f.node, vocab::FUNCTION_ABSTRACT)
    }

    pub fn name_node<'a>(&'a self, f: &FnDecl<'a>) -> Option<N<'a>> {
        self.caps_within(
            vocab::FUNCTION_NAME,
            f.node.0.start_byte(),
            f.node.0.end_byte(),
        )
        .into_iter()
        .next()
    }

    pub fn enclosing_function(&self, n: N) -> Option<FnDecl<'_>> {
        self.fn_decl(self.enclosing_function_node(n)?)
    }

    pub fn enclosing_function_node<'a>(&'a self, n: N<'a>) -> Option<N<'a>> {
        self.climb(Some(n.0), &[vocab::FUNCTION], &[])
    }

    fn fn_decl(&self, func: N) -> Option<FnDecl<'_>> {
        let (s, e) = (func.0.start_byte(), func.0.end_byte());
        let name = self
            .caps_within(vocab::FUNCTION_NAME, s, e)
            .first()
            .map(|n| self.text_of(*n).to_string())?;
        let params = self
            .caps_within(vocab::FUNCTION_PARAMS, s, e)
            .first()
            .copied()?;
        let body = self
            .caps_within(vocab::FUNCTION_BODY, s, e)
            .first()
            .copied();
        // Only an abstract declaration may lack a body: elsewhere a missing one is a query bug.
        if body.is_none() && !self.has_cap(func, vocab::FUNCTION_ABSTRACT) {
            return None;
        }
        let node = self.node(Span::of(func))?;
        let receiver = self
            .caps_owned_by(vocab::FUNCTION_RECEIVER, &[vocab::FUNCTION], &[], node)
            .first()
            .copied();
        // a parameter of function type nests a parameter list of its own, marked @opaque
        let params = self.caps_owned_by(
            vocab::PARAM,
            &[vocab::FUNCTION_PARAMS],
            &[vocab::OPAQUE],
            params,
        );
        Some(FnDecl {
            node,
            name,
            params,
            body,
            receiver,
        })
    }

    pub fn single_assignment(&self) -> bool {
        self.lang.quirks.single_assignment
    }

    /// The `@assign` whose target contains `occ`, if any.
    pub fn assign_at<'a>(&'a self, occ: N<'a>) -> Option<Assign<'a>> {
        let target = self.nearest_ancestor_with_or_self(occ, vocab::ASSIGN_TARGET)?;
        let node = N(target.0.parent()?);
        if !self.has_cap(node, vocab::ASSIGN) {
            return None;
        }
        Some(Assign {
            node,
            target,
            value: self
                .caps_child_of(vocab::ASSIGN_VALUE, node)
                .first()
                .copied(),
            compound: self.has_cap(node, vocab::ASSIGN_COMPOUND),
        })
    }

    /// The places a write to `target` reaches; the index of `x[i]` is read, not written.
    pub fn place_chain<'a>(&'a self, target: N<'a>) -> Vec<N<'a>> {
        let mut out = Vec::new();
        let mut cur = self.through(target).unwrap_or(target);
        loop {
            out.push(cur);
            let next = self
                .field_access(cur)
                .map(|(c, _)| c)
                .or_else(|| self.caps_child_of(vocab::PLACE_BASE, cur).first().copied());
            match next {
                Some(n) => cur = self.through(n).unwrap_or(n),
                None => return out,
            }
        }
    }

    pub fn escaped<'a>(&'a self, occ: N<'a>) -> Option<N<'a>> {
        self.nearest_ancestor_with_or_self(occ, vocab::ESCAPE)
    }

    /// The innermost call that passes `occ` as its receiver or as argument `i`.
    pub fn call_using<'a>(&'a self, occ: N<'a>) -> Option<(CallSite<'a>, Slot)> {
        let call = self.nearest_ancestor_with_or_self(occ, vocab::CALL)?;
        let call = self.call_site(call)?;
        if call.receiver.is_some_and(|r| contains(r.0, occ.0)) {
            return Some((call, Slot::Receiver));
        }
        let i = index_containing(&call.args, occ)?;
        Some((call, Slot::Arg(i)))
    }

    pub fn has_mutable_receiver(&self, f: &FnDecl) -> bool {
        f.receiver
            .is_some_and(|r| self.has_cap(r, vocab::FUNCTION_RECEIVER_MUTABLE))
    }

    pub fn param_is_mutable(&self, f: &FnDecl, index: usize) -> bool {
        let Some(p) = f.params.get(index) else {
            return false;
        };
        self.climb(
            Some(p.0),
            &[vocab::PARAM_MUTABLE],
            &[vocab::FUNCTION_PARAMS],
        )
        .is_some()
    }

    pub fn role_of(&self, ident: N) -> Role<'_> {
        let mut cur = Some(ident.0);
        while let Some(c) = cur {
            let n = N(c);
            if self.has_cap(n, vocab::BINDING_PATTERN)
                && let Some(binding) = c.parent()
                && self.has_cap(N(binding), vocab::BINDING)
                && let Some(binding) = self.node(Span::of(N(binding)))
                && let Some(pattern) = self.node(Span::of(n))
            {
                let value = self
                    .caps_child_of(vocab::BINDING_VALUE, binding)
                    .into_iter()
                    .next();
                let element = self.has_cap(binding, vocab::BINDING_ELEMENT);
                return match (value, element) {
                    (Some(value), true) => Role::ElementOf { value },
                    (Some(value), false) => Role::BoundBy { pattern, value },
                    (None, _) => Role::Declared {
                        literal: self.has_cap(binding, vocab::LITERAL),
                    },
                };
            }
            if self.has_cap(n, vocab::FUNCTION_RECEIVER)
                && let Some(func) = self.enclosing_function(n)
            {
                return Role::Param {
                    func,
                    slot: Slot::Receiver,
                };
            }
            if self.has_cap(n, vocab::FUNCTION_PARAMS)
                && let Some(func) = self.enclosing_function(n)
                && let Some(index) = index_containing(&func.params, ident)
            {
                return Role::Param {
                    func,
                    slot: Slot::Arg(index),
                };
            }
            // A branch clause need not have a subject: receive and try reuse it.
            if self.has_cap(n, vocab::BRANCH_PATTERN)
                && let Some(branch) = c.parent()
                && let Some(subject) = self.branch_subject(N(branch))
                && let Some(pattern) = self.node(Span::of(n))
            {
                return Role::BranchPattern { pattern, subject };
            }
            if self.has_cap(n, vocab::OPAQUE)
                && let Some(op) = self.node(Span::of(n))
            {
                return Role::Opaque(op);
            }
            if self.has_cap(n, vocab::FUNCTION_BODY) {
                break;
            }
            cur = c.parent();
        }
        Role::Use
    }

    /// Climbs, because some grammars put a block between the clause and the subject.
    fn branch_subject(&self, branch: N) -> Option<N<'_>> {
        let mut cur = self.node(Span::of(branch))?.0.parent();
        while let Some(c) = cur {
            if let Some(subject) = self
                .caps_child_of(vocab::BRANCH_SUBJECT, N(c))
                .into_iter()
                .next()
            {
                return Some(subject);
            }
            // A clause container of its own answers for its clauses, not for ours.
            if self.has_cap(N(c), vocab::BRANCH)
                || self.has_cap(N(c), vocab::RETURN_CONTAINER)
                || self.has_cap(N(c), vocab::FUNCTION_BODY)
                || self.has_cap(N(c), vocab::OPAQUE)
            {
                return None;
            }
            cur = c.parent();
        }
        None
    }

    pub fn call_at(&self, n: N) -> Option<CallSite<'_>> {
        let n = self.through(n)?;
        if self.has_cap(n, vocab::CALL) {
            return self.call_site(n);
        }
        if let Some(p) = n.0.parent()
            && self.has_cap(N(p), vocab::CALL)
        {
            return self.call_site(self.node(Span::of(N(p)))?);
        }
        None
    }

    /// Unwraps to a fixpoint: `(&x)` is `x`, not `&x`.
    pub fn through(&self, n: N) -> Option<N<'_>> {
        let mut cur = self.node(Span::of(n))?;
        while self.has_cap(cur, vocab::THROUGH) {
            match self.caps_child_of(vocab::THROUGH_INNER, cur).first() {
                Some(inner) => cur = *inner,
                None => break,
            }
        }
        Some(cur)
    }

    fn call_site<'a>(&'a self, call: N<'a>) -> Option<CallSite<'a>> {
        let callee = self
            .caps_owned_by(vocab::CALL_CALLEE, &[vocab::CALL], &[], call)
            .first()
            .copied()?;
        let args = self
            .caps_owned_by(vocab::CALL_ARGS, &[vocab::CALL], &[], call)
            .first()
            .copied()?;
        Some(CallSite {
            node: call,
            callee,
            args: named_children(args),
            receiver: self
                .caps_owned_by(vocab::CALL_RECEIVER, &[vocab::CALL], &[], call)
                .first()
                .copied(),
        })
    }

    pub fn calls_containing(&self, p: Pos) -> Vec<CallSite<'_>> {
        let Some(off) = self.byte_offset(p) else {
            return Vec::new();
        };
        self.caps_containing(vocab::CALL, off)
            .iter()
            .filter_map(|c| self.node(c.span))
            .filter_map(|n| self.call_site(n))
            .collect()
    }

    pub fn call_with_callee_at(&self, p: Pos) -> Option<CallSite<'_>> {
        let off = self.byte_offset(p)?;
        self.calls_containing(p)
            .into_iter()
            .find(|c| c.callee.0.start_byte() <= off && off < c.callee.0.end_byte())
    }

    /// Spelled as in the source, qualifier included: no separator lives here.
    pub fn callee_text(&self, call: &CallSite) -> String {
        let end = call.callee.0.end_byte();
        let start = call
            .node
            .0
            .parent()
            .filter(|p| self.has_cap(N(*p), vocab::THROUGH))
            .and_then(|p| {
                self.caps_within(vocab::CALLEE_MODULE, p.start_byte(), p.end_byte())
                    .into_iter()
                    .find(|m| m.0.end_byte() <= call.node.0.start_byte())
            })
            .map_or(call.callee.0.start_byte(), |m| m.0.start_byte());
        self.text[start..end].to_string()
    }

    pub fn returns_of(&self, f: &FnDecl) -> Vec<N<'_>> {
        let mut out = Vec::new();
        let Some(body) = f.body else {
            return out;
        };
        for root in self.caps_within(vocab::RETURN, body.0.start_byte(), body.0.end_byte()) {
            if self.owning_function(root).map(|o| o.0.id()) == Some(f.node.0.id()) {
                self.expand_return(root, &mut out);
            }
        }
        out.sort_by_key(|n| (n.0.start_byte(), n.0.end_byte()));
        out.dedup_by_key(|n| Span::of(*n));
        out
    }

    fn owning_function<'a>(&'a self, n: N<'a>) -> Option<N<'a>> {
        self.climb(n.0.parent(), &[vocab::FUNCTION], &[vocab::OPAQUE])
    }

    /// Every branch tail of a `@return.container`, nested containers expanded in turn.
    pub fn tails_of<'a>(&'a self, container: N<'a>) -> Vec<N<'a>> {
        let mut out = Vec::new();
        self.expand_return(container, &mut out);
        out
    }

    fn expand_return<'a>(&'a self, n: N<'a>, out: &mut Vec<N<'a>>) {
        if !self.has_cap(n, vocab::RETURN_CONTAINER) {
            out.push(self.through(n).unwrap_or(n));
            return;
        }
        for v in self.caps_within(vocab::RETURN_VALUE, n.0.start_byte(), n.0.end_byte()) {
            if v.0.start_byte() == n.0.start_byte() && v.0.end_byte() == n.0.end_byte() {
                continue;
            }
            if self.nearest_return_container(v).map(|c| c.0.id()) != Some(n.0.id()) {
                continue;
            }
            self.expand_return(v, out);
        }
    }

    /// `None` once the walk leaves the current function: a nested fun's tail is not
    /// a return of the enclosing function.
    fn nearest_return_container<'a>(&'a self, n: N<'a>) -> Option<N<'a>> {
        self.climb(
            n.0.parent(),
            &[vocab::RETURN_CONTAINER],
            &[vocab::OPAQUE, vocab::FUNCTION],
        )
    }

    pub fn is_literal<'a>(&'a self, n: N<'a>) -> bool {
        let n = self.through(n).unwrap_or(n);
        if self.has_cap(n, vocab::LITERAL) {
            return true;
        }
        if !self.has_cap(n, vocab::CONSTRUCT) {
            return false;
        }
        named_children(n).iter().all(|c| {
            if self.has_cap(*c, vocab::CONSTRUCT_FIELD_NAME) {
                return true;
            }
            match self.entry_value(c.0) {
                Some(v) => self.is_literal(v),
                None => self.is_literal(*c),
            }
        })
    }

    pub fn is_opaque(&self, n: N) -> bool {
        self.has_cap(n, vocab::OPAQUE)
    }

    pub fn field_access<'a>(&'a self, n: N<'a>) -> Option<(N<'a>, String)> {
        if !self.has_cap(n, vocab::FIELD) {
            return None;
        }
        let container = self
            .caps_owned_by(vocab::FIELD_CONTAINER, &[vocab::FIELD], &[], n)
            .first()
            .copied()?;
        let name = self
            .caps_owned_by(vocab::FIELD_NAME, &[vocab::FIELD], &[], n)
            .first()
            .copied()?;
        Some((container, self.text_of(name).to_string()))
    }

    pub fn construct_base<'a>(&'a self, construct: N<'a>) -> Option<N<'a>> {
        self.caps_child_of(vocab::CONSTRUCT_BASE, construct)
            .first()
            .copied()
    }

    /// `@construct.field.name` captures of this construct's own entries, not of a nested one.
    fn direct_field_names<'a>(&'a self, construct: N<'a>) -> Vec<N<'a>> {
        let (s, e) = (construct.0.start_byte(), construct.0.end_byte());
        self.caps_within(vocab::CONSTRUCT_FIELD_NAME, s, e)
            .into_iter()
            .filter(|n| {
                n.0.parent()
                    .and_then(|entry| entry.parent())
                    .map(|p| (p.start_byte(), p.end_byte()))
                    == Some((s, e))
            })
            .collect()
    }

    fn entry_value(&self, entry: tree_sitter::Node) -> Option<N<'_>> {
        self.caps_within(
            vocab::CONSTRUCT_FIELD_VALUE,
            entry.start_byte(),
            entry.end_byte(),
        )
        .first()
        .copied()
    }

    pub fn construct_field(&self, construct: N, field: &str) -> Option<N<'_>> {
        for name in self.direct_field_names(construct) {
            if self.text_of(name) != field {
                continue;
            }
            return self.entry_value(name.0.parent()?);
        }
        None
    }

    /// Elements of a positional construct; `None` when it is keyed or a cons.
    pub fn positional<'a>(&'a self, construct: N<'a>) -> Option<Vec<N<'a>>> {
        let n = self.through(construct).unwrap_or(construct);
        if !self.has_cap(n, vocab::CONSTRUCT) || self.has_cap(n, vocab::CONSTRUCT_CONS) {
            return None;
        }
        if !self.direct_field_names(n).is_empty() {
            return None;
        }
        Some(
            named_children(n)
                .into_iter()
                .filter(|c| !self.has_cap(*c, vocab::CONSTRUCT_BASE))
                .map(|c| self.through(c).unwrap_or(c))
                .collect(),
        )
    }

    pub fn construct_element<'a>(&'a self, construct: N<'a>, i: usize) -> Option<N<'a>> {
        self.positional(construct)?.get(i).copied()
    }

    pub fn pattern_index(&self, pattern: N, ident: N) -> Option<usize> {
        index_containing(&self.positional(pattern)?, ident)
    }

    pub fn destructure<'a>(&'a self, pattern: N<'a>, ident: N<'a>, value: N<'a>) -> Option<N<'a>> {
        if pattern.0.start_byte() == ident.0.start_byte()
            && pattern.0.end_byte() == ident.0.end_byte()
        {
            return self.node(Span::of(value));
        }
        // Unwrapped only past the whole-pattern case: there the wrapper is the value.
        let value = self.through(value).unwrap_or(value);
        if !self.has_cap(pattern, vocab::CONSTRUCT)
            || !self.has_cap(value, vocab::CONSTRUCT)
            || self.has_cap(pattern, vocab::CONSTRUCT_CONS)
            || self.has_cap(value, vocab::CONSTRUCT_CONS)
        {
            return None;
        }

        let direct = self.direct_field_names(pattern);
        if direct.is_empty() {
            let (pc, vc) = (self.positional(pattern)?, self.positional(value)?);
            if pc.len() != vc.len() {
                return None;
            }
            let index = index_containing(&pc, ident)?;
            return self.destructure(*pc.get(index)?, ident, *vc.get(index)?);
        }

        for name in direct {
            let Some(pv) = self.entry_value(name.0.parent()?) else {
                continue;
            };
            if !contains(pv.0, ident.0) {
                continue;
            }
            let vv = self.construct_field(value, self.text_of(name))?;
            return self.destructure(pv, ident, vv);
        }
        None
    }
}

fn named_children<'t>(n: N<'t>) -> Vec<N<'t>> {
    let mut cursor = n.0.walk();
    n.0.named_children(&mut cursor)
        .filter(|c| !c.is_extra()) // comments are named nodes in tree-sitter
        .map(N)
        .collect()
}

fn contains(outer: tree_sitter::Node, inner: tree_sitter::Node) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

pub(crate) fn index_containing(nodes: &[N], inner: N) -> Option<usize> {
    nodes.iter().position(|n| contains(n.0, inner.0))
}
