//! Structural questions over a parsed document, answered only through the
//! capture vocabulary in [`crate::lang::vocab`]: no grammar names live here.

use crate::lang::{Language, Returns, vocab};
use crate::pos::{self, Pos};
use std::collections::HashSet;
use std::path::PathBuf;
use tree_sitter::StreamingIterator;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Cap {
    cap: u32,
    start: usize,
    end: usize,
    kind_id: u16,
}

#[derive(Clone, Copy)]
pub struct N<'t>(pub tree_sitter::Node<'t>);

pub struct FnDecl<'t> {
    pub node: N<'t>,
    pub name: String,
    pub params: Vec<N<'t>>,
    pub body: N<'t>,
}

pub struct CallSite<'t> {
    pub node: N<'t>,
    pub callee: N<'t>,
    pub args: Vec<N<'t>>,
}

pub enum Role<'t> {
    BoundBy { pattern: N<'t>, value: N<'t> },
    Param { func: FnDecl<'t>, index: usize },
    BranchPattern { pattern: N<'t>, subject: N<'t> },
    Opaque(N<'t>),
    Use,
}

pub struct Doc<'l> {
    pub path: PathBuf,
    pub text: String,
    pub tree: tree_sitter::Tree,
    lang: &'l Language,
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
                caps.push(Cap {
                    cap: c.index,
                    start: c.node.start_byte(),
                    end: c.node.end_byte(),
                    kind_id: c.node.kind_id(),
                });
            }
        }
        caps.sort_by_key(|c| (c.start, c.end, c.cap));
        caps.dedup();
        let set = caps.iter().copied().collect();

        Doc {
            path,
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

    fn node_of(&self, c: &Cap) -> Option<N<'_>> {
        let mut n = self
            .tree
            .root_node()
            .descendant_for_byte_range(c.start, c.end)?;
        loop {
            if n.start_byte() == c.start && n.end_byte() == c.end && n.kind_id() == c.kind_id {
                return Some(N(n));
            }
            n = n.parent()?;
        }
    }

    fn caps_within(&self, cap: &str, start: usize, end: usize) -> Vec<N<'_>> {
        let Some(idx) = self.cap_index(cap) else {
            return Vec::new();
        };
        self.caps
            .iter()
            .filter(|c| c.cap == idx && c.start >= start && c.end <= end)
            .filter_map(|c| self.node_of(c))
            .collect()
    }

    fn caps_owned_by<'a>(&'a self, cap: &str, owner_cap: &str, owner: N<'a>) -> Vec<N<'a>> {
        self.caps_within(cap, owner.0.start_byte(), owner.0.end_byte())
            .into_iter()
            .filter(|n| {
                self.nearest_ancestor_with(*n, owner_cap)
                    .is_some_and(|a| a.0.id() == owner.0.id())
            })
            .collect()
    }

    fn nearest_ancestor_with<'a>(&'a self, n: N<'a>, cap: &str) -> Option<N<'a>> {
        let mut cur = n.0.parent();
        while let Some(c) = cur {
            if self.has_cap(N(c), cap) {
                return Some(N(c));
            }
            cur = c.parent();
        }
        None
    }

    fn caps_child_of<'a>(&'a self, cap: &str, parent: N<'a>) -> Vec<N<'a>> {
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
            start: n.0.start_byte(),
            end: n.0.end_byte(),
            kind_id: n.0.kind_id(),
        })
    }

    pub fn text_of(&self, n: N) -> &str {
        &self.text[n.0.start_byte()..n.0.end_byte()]
    }

    pub fn line_of(&self, n: N) -> &str {
        let start = self.text[..n.0.start_byte()]
            .rfind('\n')
            .map_or(0, |i| i + 1);
        let end = self.text[start..]
            .find('\n')
            .map_or(self.text.len(), |i| start + i);
        self.text[start..end].trim()
    }

    pub fn pos_of(&self, n: N) -> Pos {
        pos::pos_of(&self.text, n.0.start_byte())
    }

    pub fn ident_at(&self, p: Pos) -> Option<N<'_>> {
        let off = pos::byte_offset(&self.text, p)?;
        let idx = self.cap_index(vocab::IDENT)?;
        self.caps
            .iter()
            .filter(|c| c.cap == idx && c.start <= off && off < c.end)
            .min_by_key(|c| c.end - c.start)
            .and_then(|c| self.node_of(c))
    }

    pub fn enclosing_function(&self, n: N) -> Option<FnDecl<'_>> {
        let mut cur = Some(n.0);
        while let Some(c) = cur {
            if self.has_cap(N(c), vocab::FUNCTION) {
                return self.fn_decl(N(c));
            }
            cur = c.parent();
        }
        None
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
            .copied()?;
        let node = self.node_of(&Cap {
            cap: self.cap_index(vocab::FUNCTION)?,
            start: s,
            end: e,
            kind_id: func.0.kind_id(),
        })?;
        Some(FnDecl {
            node,
            name,
            params: named_children(params),
            body,
        })
    }

    pub fn role_of(&self, ident: N) -> Role<'_> {
        let mut cur = Some(ident.0);
        while let Some(c) = cur {
            let n = N(c);
            if self.has_cap(n, vocab::BINDING_PATTERN)
                && let Some(binding) = c.parent()
                && self.has_cap(N(binding), vocab::BINDING)
                && let Some(binding) = self.reborrow(N(binding))
                && let Some(value) = self
                    .caps_child_of(vocab::BINDING_VALUE, binding)
                    .into_iter()
                    .next()
                && let Some(pattern) = self.reborrow(n)
            {
                return Role::BoundBy { pattern, value };
            }
            if self.has_cap(n, vocab::FUNCTION_PARAMS)
                && let Some(func) = self.enclosing_function(n)
                && let Some(index) = index_of_child_containing(c, ident.0)
            {
                return Role::Param { func, index };
            }
            // A branch clause need not have a subject: receive and try reuse it.
            if self.has_cap(n, vocab::BRANCH_PATTERN)
                && let Some(branch) = c.parent()
                && let Some(case) = branch.parent()
                && let Some(case) = self.reborrow(N(case))
                && let Some(subject) = self
                    .caps_child_of(vocab::BRANCH_SUBJECT, case)
                    .into_iter()
                    .next()
                && let Some(pattern) = self.reborrow(n)
            {
                return Role::BranchPattern { pattern, subject };
            }
            if self.has_cap(n, vocab::OPAQUE)
                && let Some(op) = self.reborrow(n)
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

    fn reborrow(&self, n: N) -> Option<N<'_>> {
        let mut cur = self
            .tree
            .root_node()
            .descendant_for_byte_range(n.0.start_byte(), n.0.end_byte())?;
        loop {
            if cur.start_byte() == n.0.start_byte()
                && cur.end_byte() == n.0.end_byte()
                && cur.kind_id() == n.0.kind_id()
            {
                return Some(N(cur));
            }
            cur = cur.parent()?;
        }
    }

    pub fn call_at(&self, n: N) -> Option<CallSite<'_>> {
        let n = self.through(n)?;
        if self.has_cap(n, vocab::CALL) {
            return self.call_site(n);
        }
        if let Some(p) = n.0.parent()
            && self.has_cap(N(p), vocab::CALL)
        {
            return self.call_site(self.reborrow(N(p))?);
        }
        None
    }

    pub fn through(&self, n: N) -> Option<N<'_>> {
        if !self.has_cap(n, vocab::THROUGH) {
            return self.reborrow(n);
        }
        self.caps_within(vocab::THROUGH_INNER, n.0.start_byte(), n.0.end_byte())
            .first()
            .copied()
            .or_else(|| self.reborrow(n))
    }

    fn call_site<'a>(&'a self, call: N<'a>) -> Option<CallSite<'a>> {
        let callee = self
            .caps_owned_by(vocab::CALL_CALLEE, vocab::CALL, call)
            .first()
            .copied()?;
        let args = self
            .caps_owned_by(vocab::CALL_ARGS, vocab::CALL, call)
            .first()
            .copied()?;
        Some(CallSite {
            node: call,
            callee,
            args: named_children(args),
        })
    }

    pub fn calls_containing(&self, p: Pos) -> Vec<CallSite<'_>> {
        let Some(off) = pos::byte_offset(&self.text, p) else {
            return Vec::new();
        };
        let Some(idx) = self.cap_index(vocab::CALL) else {
            return Vec::new();
        };
        let mut hits: Vec<Cap> = self
            .caps
            .iter()
            .copied()
            .filter(|c| c.cap == idx && c.start <= off && off < c.end)
            .collect();
        hits.sort_by_key(|c| c.end - c.start);
        hits.iter()
            .filter_map(|c| self.node_of(c))
            .filter_map(|n| self.call_site(n))
            .collect()
    }

    pub fn arg_index(&self, call: &CallSite, n: N) -> Option<usize> {
        call.args
            .iter()
            .position(|a| a.0.start_byte() <= n.0.start_byte() && n.0.end_byte() <= a.0.end_byte())
    }

    pub fn callee_text(&self, call: &CallSite) -> String {
        let name = self.text_of(call.callee);
        if let Some(p) = call.node.0.parent()
            && self.has_cap(N(p), vocab::THROUGH)
            && let Some(m) = self
                .caps_within(vocab::CALLEE_MODULE, p.start_byte(), p.end_byte())
                .into_iter()
                .find(|m| m.0.end_byte() <= call.node.0.start_byte())
        {
            return format!("{}:{}", self.text_of(m), name);
        }
        name.to_string()
    }

    pub fn callee_name_pos(&self, call: &CallSite) -> Pos {
        self.pos_of(call.callee)
    }

    pub fn returns_of(&self, f: &FnDecl) -> Vec<N<'_>> {
        if self.lang.quirks.returns != Returns::Tail {
            warn_returns_unsupported(self.lang.name, self.lang.quirks.returns);
        }
        let mut cursor = f.body.0.walk();
        let Some(last) = f
            .body
            .0
            .named_children(&mut cursor)
            .filter(|c| !c.is_extra())
            .last()
        else {
            return Vec::new();
        };
        let Some(last) = self.reborrow(N(last)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        self.expand_return(last, &mut out);
        out
    }

    fn expand_return<'a>(&'a self, n: N<'a>, out: &mut Vec<N<'a>>) {
        if !self.has_cap(n, vocab::RETURN_CONTAINER) {
            out.push(n);
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
        let mut cur = n.0.parent();
        while let Some(c) = cur {
            if self.has_cap(N(c), vocab::RETURN_CONTAINER) {
                return Some(N(c));
            }
            if self.has_cap(N(c), vocab::OPAQUE) || self.has_cap(N(c), vocab::FUNCTION) {
                return None;
            }
            cur = c.parent();
        }
        None
    }

    pub fn is_literal(&self, n: N) -> bool {
        if self.has_cap(n, vocab::LITERAL) {
            return true;
        }
        if !self.has_cap(n, vocab::CONSTRUCT) {
            return false;
        }
        named_children(n).iter().all(|c| self.is_literal(*c))
    }

    pub fn is_opaque(&self, n: N) -> bool {
        self.has_cap(n, vocab::OPAQUE)
    }

    pub fn field_access<'a>(&'a self, n: N<'a>) -> Option<(N<'a>, String)> {
        if !self.has_cap(n, vocab::FIELD) {
            return None;
        }
        let container = self
            .caps_owned_by(vocab::FIELD_CONTAINER, vocab::FIELD, n)
            .first()
            .copied()?;
        let name = self
            .caps_owned_by(vocab::FIELD_NAME, vocab::FIELD, n)
            .first()
            .copied()?;
        Some((container, self.text_of(name).to_string()))
    }

    pub fn construct_field(&self, construct: N, field: &str) -> Option<N<'_>> {
        let (s, e) = (construct.0.start_byte(), construct.0.end_byte());
        for name in self.caps_within(vocab::CONSTRUCT_FIELD_NAME, s, e) {
            let Some(entry) = name.0.parent() else {
                continue;
            };
            if entry.parent().map(|p| (p.start_byte(), p.end_byte())) != Some((s, e)) {
                continue;
            }
            if self.text_of(name) != field {
                continue;
            }
            return self
                .caps_within(
                    vocab::CONSTRUCT_FIELD_VALUE,
                    entry.start_byte(),
                    entry.end_byte(),
                )
                .first()
                .copied();
        }
        None
    }

    pub fn destructure(&self, pattern: N, ident: N, value: N) -> Option<N<'_>> {
        if pattern.0.start_byte() == ident.0.start_byte()
            && pattern.0.end_byte() == ident.0.end_byte()
        {
            return self.reborrow(value);
        }
        if pattern.0.kind_id() != value.0.kind_id()
            || !self.has_cap(pattern, vocab::CONSTRUCT)
            || !self.has_cap(value, vocab::CONSTRUCT)
            || self.has_cap(pattern, vocab::CONSTRUCT_CONS)
            || self.has_cap(value, vocab::CONSTRUCT_CONS)
        {
            return None;
        }

        let (ps, pe) = (pattern.0.start_byte(), pattern.0.end_byte());
        let field_names = self.caps_within(vocab::CONSTRUCT_FIELD_NAME, ps, pe);
        let direct: Vec<N> = field_names
            .into_iter()
            .filter(|n| {
                n.0.parent()
                    .and_then(|e| e.parent())
                    .map(|p| (p.start_byte(), p.end_byte()))
                    == Some((ps, pe))
            })
            .collect();

        if direct.is_empty() {
            let (pc, vc) = (named_children(pattern), named_children(value));
            if pc.len() != vc.len() {
                return None;
            }
            let index = index_of_child_containing(pattern.0, ident.0)?;
            return self.destructure(*pc.get(index)?, ident, *vc.get(index)?);
        }

        for name in direct {
            let entry = name.0.parent()?;
            let Some(pv) = self
                .caps_within(
                    vocab::CONSTRUCT_FIELD_VALUE,
                    entry.start_byte(),
                    entry.end_byte(),
                )
                .first()
                .copied()
            else {
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
    n.0.named_children(&mut cursor).map(N).collect()
}

fn contains(outer: tree_sitter::Node, inner: tree_sitter::Node) -> bool {
    outer.start_byte() <= inner.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn index_of_child_containing(parent: tree_sitter::Node, inner: tree_sitter::Node) -> Option<usize> {
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .position(|c| contains(c, inner))
}

fn warn_returns_unsupported(lang: &str, returns: Returns) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::warn!(
            "{lang}: quirks.returns = {returns:?} is not implemented (M2); \
             falling back to tail-expression returns"
        );
    });
}
