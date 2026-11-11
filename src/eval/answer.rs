// SPDX-License-Identifier: Apache-2.0

//! Answers in solved form: `X = t` for every bound query variable, aliases
//! `X = Y` for query variables that became one, pending disequations, and
//! `true` when nothing remains. Cyclic terms print as equations (`X = f(X)`),
//! which is the finite representation of a rational tree. The printer is
//! iterative so deep terms cannot overflow the stack.

use std::cell::Ref;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;

use crate::Number;
use crate::ast::{Atom, Expr};
use crate::solver::{LinExpr, SVar};

use super::compile::TTerm;
use super::project::{Projected, project};
use super::store::{Addr, Cell, PendingDif, Store, TypeKind};
use super::symbol::{LIST, LIST_LABEL, Symbols};

/// One answer to a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// `X = t` equations, query variables first, then named cyclic nodes.
    pub equations: Vec<String>,
    /// Pending tree disequations, rendered `t1 != t2`.
    pub disequations: Vec<String>,
    /// Numeric constraints: bounds, linear equations, numeric disequations.
    pub constraints: Vec<String>,
    /// The proof tree (CLM-0023), present when the solutions were asked for
    /// explanations (`Solutions::with_explanations`): the query's goals as
    /// roots. Not part of the answer's `Display`.
    pub explanation: Option<Vec<ProofNode>>,
}

/// One node of an answer's proof tree (CLM-0023,
/// docs/design/2026-11-09-explanations-of-answers.md): a goal of the
/// derivation as it stands at the answer, and what resolved it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    /// The goal instance, rendered with the answer's bindings and names; for
    /// a committed or failed condition, the construct as written.
    pub goal: String,
    pub by: Resolution,
    /// The cited clause's position (1-based), when a clause is cited and the
    /// program was compiled with positions.
    pub line: Option<usize>,
    pub column: Option<usize>,
    /// The cited clause as written (facts and rules).
    pub clause: Option<String>,
    pub subgoals: Vec<ProofNode>,
    /// CLM-0031: why the goal failed; present only on a `Resolution::Failed`
    /// node of a failure tree.
    pub failure: Option<Box<Failure>>,
}

/// Why a goal failed (CLM-0031,
/// docs/design/2026-11-11-explanations-of-failure.md): the leaf of a failed
/// branch carries one of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub reason: Reason,
    /// `Unsatisfiable`: the earlier steps of the derivation that, together
    /// with the failed goal, have no solution — shallow nodes (goal, what
    /// resolved it, position, clause), a minimal set unless `note` says
    /// otherwise.
    pub conflict: Vec<ProofNode>,
    /// `Unsatisfiable`: why the set could not be minimised (every step is
    /// then cited).
    pub note: Option<String>,
    /// `NoClause`: the predicate's clauses, and the candidates the
    /// first-argument index admitted.
    pub clauses: Option<usize>,
    pub candidates: Option<usize>,
    /// `NoTableAnswer`: the answers in the table.
    pub answers: Option<usize>,
}

/// The reason a goal failed (CLM-0031).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The step's store effect had no solution; `conflict` says why.
    Unsatisfiable,
    /// No candidate clause resolved the call.
    NoClause,
    /// A negation whose condition succeeded; the solution's proof is the
    /// node's `subgoals`.
    HasSolution,
    /// A tabled call whose complete table has no unifying answer.
    NoTableAnswer,
    /// An if-then-else without an else whose condition had no solution.
    NoSolution,
}

impl Reason {
    /// The JSON spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Unsatisfiable => "unsatisfiable",
            Reason::NoClause => "no-clause",
            Reason::HasSolution => "has-solution",
            Reason::NoTableAnswer => "no-table-answer",
            Reason::NoSolution => "no-solution",
        }
    }
}

impl Failure {
    /// The human rendering after `failed: `.
    pub fn describe(&self) -> String {
        match self.reason {
            Reason::Unsatisfiable => match &self.note {
                None => "unsatisfiable with".to_string(),
                Some(note) => format!("unsatisfiable with every step (not minimised: {note})"),
            },
            Reason::NoClause => {
                let clauses = self.clauses.unwrap_or(0);
                let candidates = self.candidates.unwrap_or(0);
                let why = if clauses == 0 {
                    "undefined predicate".to_string()
                } else if candidates == 0 && clauses == 1 {
                    "the clause cannot match the first argument".to_string()
                } else if candidates == 0 {
                    format!("none of {clauses} clauses can match the first argument")
                } else if candidates == 1 {
                    "the candidate does not unify".to_string()
                } else {
                    format!("none of {candidates} candidates unifies")
                };
                format!("no clause ({why})")
            }
            Reason::HasSolution => "has a solution".to_string(),
            Reason::NoTableAnswer => {
                format!(
                    "no table answer ({} in the table)",
                    self.answers.unwrap_or(0)
                )
            }
            Reason::NoSolution => "no solution".to_string(),
        }
    }
}

/// The failure tree of a query that had no answer (CLM-0031): the failed
/// derivations as branches, in search order, up to the machine's budget;
/// `omitted` counts the rest.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FailureTree {
    pub branches: Vec<Vec<ProofNode>>,
    pub omitted: usize,
}

impl FailureTree {
    /// The tree as lines for a human, to print after `false`: each branch
    /// as a proof tree ending at its failed goal, headed `branch N` when
    /// there are several, and the omitted count last.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let headed = self.branches.len() > 1;
        for (i, roots) in self.branches.iter().enumerate() {
            let depth = if headed {
                out.push(format!("  branch {}", i + 1));
                2
            } else {
                1
            };
            push_tree_lines(&mut out, roots, depth);
        }
        match self.omitted {
            0 => {}
            1 => out.push("  … and 1 more failed branch".to_string()),
            n => out.push(format!("  … and {n} more failed branches")),
        }
        out
    }
}

/// One node per line, indented two spaces per level, in derivation order —
/// the goal, what resolved it, the cited position, and a rule's text; a
/// failed node's reason, then its conflict's citations (without rule text)
/// and the subgoals. Iterative: a derivation can be deep.
fn push_tree_lines(out: &mut Vec<String>, roots: &[ProofNode], depth: usize) {
    // (node, depth, is a citation)
    let mut work: Vec<(&ProofNode, usize, bool)> =
        roots.iter().rev().map(|n| (n, depth, false)).collect();
    while let Some((node, depth, cited)) = work.pop() {
        let indent = "  ".repeat(depth);
        if let (Resolution::Failed, Some(failure)) = (node.by, &node.failure) {
            out.push(format!(
                "{indent}{}  failed: {}",
                node.goal,
                failure.describe()
            ));
            work.extend(node.subgoals.iter().rev().map(|n| (n, depth + 1, false)));
            work.extend(failure.conflict.iter().rev().map(|n| (n, depth + 1, true)));
            continue;
        }
        let mut line = format!("{indent}{}  {}", node.goal, node.by.as_str());
        if let (Some(l), Some(c)) = (node.line, node.column) {
            line.push_str(&format!(" {l}:{c}"));
        }
        if node.by == Resolution::Rule && !cited {
            if let Some(text) = &node.clause {
                line.push_str("  ");
                line.push_str(text);
            }
        }
        out.push(line);
        work.extend(node.subgoals.iter().rev().map(|n| (n, depth + 1, false)));
    }
}

/// A proof tree of any depth drops in constant stack (as terms do, CLM-0019):
/// the subgoals are moved into a worklist and dropped in a loop.
impl Drop for ProofNode {
    fn drop(&mut self) {
        let mut work = std::mem::take(&mut self.subgoals);
        if let Some(failure) = self.failure.take() {
            work.extend(failure.conflict);
        }
        while let Some(mut n) = work.pop() {
            work.append(&mut n.subgoals);
            if let Some(failure) = n.failure.take() {
                work.extend(failure.conflict);
            }
        }
    }
}

/// What resolved a goal (CLM-0023).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// A clause with no body.
    Fact,
    /// A clause with a body: its goals are the subgoals.
    Rule,
    /// A constraint goal, as the instance posted; cites the clause it is in.
    Constraint,
    /// A built-in predicate.
    Builtin,
    /// An if-then-else whose condition succeeded: the condition's proofs
    /// are the subgoals; the then branch follows as siblings.
    Committed,
    /// A condition (of an if-then-else or a negation) that had no solution.
    NoSolution,
    /// A list comprehension (CLM-0025): the goal is the comprehension as
    /// written with the list it produced; the solutions' own proofs are
    /// discarded with their derivations.
    Collected,
    /// A call to a tabled predicate (CLM-0027) answered from its table: the
    /// subgoals are the derivation that first produced the answer.
    Tabled,
    /// CLM-0031: the goal at which a failed branch died; `ProofNode::failure`
    /// says why.
    Failed,
}

impl Resolution {
    /// The JSON spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Resolution::Fact => "fact",
            Resolution::Rule => "rule",
            Resolution::Constraint => "constraint",
            Resolution::Builtin => "builtin",
            Resolution::Committed => "committed",
            Resolution::NoSolution => "no-solution",
            Resolution::Collected => "collected",
            Resolution::Tabled => "tabled",
            Resolution::Failed => "failed",
        }
    }
}

impl Answer {
    /// True if the answer carries nothing (`true`).
    pub fn is_true(&self) -> bool {
        self.equations.is_empty() && self.disequations.is_empty() && self.constraints.is_empty()
    }

    /// The explanation as lines for a human (CLM-0023): one node per line,
    /// indented two spaces per level, in derivation order — the goal, what
    /// resolved it, the cited position, and a rule's text. Empty when the
    /// answer carries no explanation. Iterative: a derivation can be deep.
    pub fn explanation_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(roots) = &self.explanation {
            push_tree_lines(&mut out, roots, 1);
        }
        out
    }

    /// All lines of the answer, in print order.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.equations
            .iter()
            .chain(self.disequations.iter())
            .chain(self.constraints.iter())
            .map(String::as_str)
    }
}

impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_true() {
            return f.write_str("true");
        }
        let mut first = true;
        for part in self
            .equations
            .iter()
            .chain(self.disequations.iter())
            .chain(self.constraints.iter())
        {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            f.write_str(part)?;
        }
        Ok(())
    }
}

/// A unit of the iterative renderer's work list.
enum Item {
    Term(Addr),
    Text(&'static str),
}

struct Printer<'a> {
    symbols: Ref<'a, Symbols>,
    store: &'a Store,
    /// Display names for unbound variables and for cyclic nodes.
    names: HashMap<Addr, String>,
    /// Compound cells that are targets of a back edge (cyclic nodes).
    cyclic: HashSet<Addr>,
    /// Cyclic nodes that still need their own `_N = …` equation.
    pending_nodes: Vec<Addr>,
    next_internal: usize,
}

impl<'a> Printer<'a> {
    fn new(store: &'a Store) -> Self {
        Printer {
            symbols: store.symbols.borrow(),
            store,
            names: HashMap::new(),
            cyclic: HashSet::new(),
            pending_nodes: Vec::new(),
            next_internal: 1,
        }
    }

    fn fresh_name(&mut self) -> String {
        let n = format!("_{}", self.next_internal);
        self.next_internal += 1;
        n
    }

    fn name_for(&mut self, a: Addr) -> String {
        if let Some(n) = self.names.get(&a) {
            return n.clone();
        }
        let n = self.fresh_name();
        self.names.insert(a, n.clone());
        n
    }

    /// Mark compound nodes reachable from `root` that are revisited while
    /// still on the DFS path (back-edge targets): those are cyclic.
    fn find_cycles(&mut self, root: Addr) {
        enum Step {
            Enter(Addr),
            Leave(Addr),
        }
        let mut on_path: HashSet<Addr> = HashSet::new();
        let mut done: HashSet<Addr> = HashSet::new();
        let mut stack = vec![Step::Enter(root)];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(a) => {
                    let a = self.store.deref(a);
                    let Cell::Struct(_, args) = self.store.cell(a) else {
                        continue;
                    };
                    if on_path.contains(&a) {
                        self.cyclic.insert(a);
                        continue;
                    }
                    if done.contains(&a) {
                        continue;
                    }
                    on_path.insert(a);
                    stack.push(Step::Leave(a));
                    for &arg in args.iter().rev() {
                        stack.push(Step::Enter(arg));
                    }
                }
                Step::Leave(a) => {
                    on_path.remove(&a);
                    done.insert(a);
                }
            }
        }
    }

    /// CLM-0014: queue the rendering of the list whose first cell is `cell`
    /// (a `'<>'/2` structure): `<h1, h2, h3>` when the spine ends in `'<>'`,
    /// `<h1, h2> . t` when it ends in anything else. The spine stops at a
    /// cyclic node, which prints by name, so `X = <1> . X` is the finite form
    /// of a cyclic list.
    fn push_list(&self, stack: &mut Vec<Item>, cell: Addr) {
        let mut heads = Vec::new();
        let mut tail = cell;
        let mut first = true;
        loop {
            let a = self.store.deref(tail);
            match self.store.cell(a) {
                // The first cell is walked even when it is cyclic (its inner
                // occurrences print by name); any later cyclic cell ends the
                // spine, which also stops a cycle back to the first cell.
                Cell::Struct(f, args)
                    if args.len() == 2
                        && self.symbols.name(*f) == LIST_LABEL
                        && (first || !self.cyclic.contains(&a)) =>
                {
                    heads.push(args[0]);
                    tail = args[1];
                    first = false;
                }
                _ => break,
            }
        }
        let a = self.store.deref(tail);
        let proper =
            matches!(self.store.cell(a), Cell::Const(c) if self.symbols.name(*c) == LIST_LABEL);
        if proper {
            stack.push(Item::Text(">"));
        } else {
            stack.push(Item::Term(tail));
            stack.push(Item::Text("> . "));
        }
        for (i, &h) in heads.iter().enumerate().rev() {
            if i + 1 < heads.len() {
                stack.push(Item::Text(", "));
            }
            stack.push(Item::Term(h));
        }
        stack.push(Item::Text("<"));
    }

    /// Render the term at `root`. If `root` is itself a cyclic node it is
    /// expanded here (its inner occurrences print as its name); other cyclic
    /// nodes print as names and are queued for their own equations.
    fn render(&mut self, root: Addr) -> String {
        let root = self.store.deref(root);
        let mut out = String::new();
        let mut stack = vec![Item::Term(root)];
        let mut first = true;
        while let Some(item) = stack.pop() {
            match item {
                Item::Text(t) => out.push_str(t),
                Item::Term(a) => {
                    let a = self.store.deref(a);
                    let is_root = first && a == root;
                    first = false;
                    match self.store.cell(a).clone() {
                        Cell::Var(_) => {
                            // A numeric variable with a determined value prints as the value.
                            let fixed = self
                                .store
                                .numvar
                                .get(&a)
                                .and_then(|sv| self.store.class_value(*sv));
                            match fixed {
                                Some(c) => out.push_str(&c.to_string()),
                                None => {
                                    let n = self.name_for(a);
                                    out.push_str(&n);
                                }
                            }
                        }
                        Cell::Const(c) if self.symbols.name(c) == LIST_LABEL => out.push_str("<>"),
                        Cell::Const(c) => {
                            out.push_str(&crate::ast::display_name(self.symbols.name(c)))
                        }
                        Cell::Num(n) => out.push_str(&n.to_string()),
                        Cell::Struct(f, args) => {
                            if self.cyclic.contains(&a) && !is_root {
                                if !self.names.contains_key(&a) {
                                    let n = self.fresh_name();
                                    self.names.insert(a, n);
                                    self.pending_nodes.push(a);
                                }
                                out.push_str(&self.names[&a]);
                                continue;
                            }
                            if args.len() == 2 && self.symbols.name(f) == LIST_LABEL {
                                self.push_list(&mut stack, a);
                                continue;
                            }
                            out.push_str(&crate::ast::display_name(self.symbols.name(f)));
                            out.push('(');
                            stack.push(Item::Text(")"));
                            for (i, &arg) in args.iter().enumerate().rev() {
                                if i + 1 < args.len() {
                                    stack.push(Item::Text(", "));
                                }
                                stack.push(Item::Term(arg));
                            }
                        }
                    }
                }
            }
        }
        out
    }
}

/// Build the answer for the query variables from the current store.
/// Render an answer, and (CLM-0023) the terms at `extra` — an explanation's
/// goal instances — with the same printer, so internal names agree between
/// the answer and its explanation.
pub(crate) fn render_answer(
    store: &Store,
    query_vars: &[(String, Addr)],
    extra: &[Addr],
) -> (Answer, Vec<String>) {
    let mut p = Printer::new(store);
    let mut equations = Vec::new();

    // Projection of tree disequations (design D7): one belongs to the answer
    // only if it mentions a variable reachable from the query variables.
    // Disequations over internal variables alone are always satisfiable
    // (finitely many, over an infinite universe) and are dropped.
    let reachable = reachable_vars(store, query_vars.iter().map(|(_, a)| *a));
    let difs: Vec<PendingDif> = store
        .pending_difs()
        .into_iter()
        .filter(|(a, b, _)| {
            reachable_vars(store, [*a, *b])
                .iter()
                .any(|v| reachable.contains(v))
        })
        .collect();

    // Pass 1: cycle detection over everything we will print.
    for (_, addr) in query_vars {
        p.find_cycles(*addr);
    }
    for (a, b, _) in &difs {
        p.find_cycles(*a);
        p.find_cycles(*b);
    }
    for &a in extra {
        p.find_cycles(a);
    }

    // Pass 2: name every query variable before rendering anything, so that
    // (a) unbound variables print under their own names wherever they occur,
    // (b) aliases print `A = B` in variable order, and (c) a cyclic value
    // node is labelled by the first variable bound to it (`X = f(X)`).
    let mut aliases: Vec<Option<String>> = Vec::with_capacity(query_vars.len());
    for (name, addr) in query_vars {
        let d = store.deref(*addr);
        match store.cell(d) {
            Cell::Var(_) => {
                if let Some(existing) = p.names.get(&d) {
                    aliases.push(Some(format!("{existing} = {name}")));
                } else {
                    p.names.insert(d, name.clone());
                    aliases.push(None);
                }
            }
            _ => {
                if p.cyclic.contains(&d) && !p.names.contains_key(&d) {
                    p.names.insert(d, name.clone());
                }
                aliases.push(None);
            }
        }
    }

    // Pass 3: equations, in query-variable order. An unbound numeric
    // variable whose value is determined prints as `X = value`.
    for ((name, addr), alias) in query_vars.iter().zip(aliases) {
        if let Some(alias) = alias {
            equations.push(alias);
            continue;
        }
        let d = store.deref(*addr);
        if !matches!(store.cell(d), Cell::Var(_)) {
            let rendered = p.render(d);
            equations.push(format!("{name} = {rendered}"));
        } else if let Some(c) = store.numvar.get(&d).and_then(|sv| store.class_value(*sv)) {
            equations.push(format!("{name} = {c}"));
        }
    }

    // Pass 4: tree disequations, in reduced form when there is one.
    let mut disequations = Vec::new();
    for (a, b, reduced) in difs {
        let (l, r) = if reduced.len() == 1 {
            let (v, t) = reduced[0];
            // Two variables: earlier-created first, for stable output.
            let t_is_var = matches!(store.cell(store.deref(t)), Cell::Var(_));
            let (v, t) = if t_is_var && store.deref(t) < store.deref(v) {
                (t, v)
            } else {
                (v, t)
            };
            (p.render(v), p.render(t))
        } else {
            (p.render(a), p.render(b))
        };
        disequations.push(format!("{l} != {r}"));
    }

    // Pass 5: cyclic nodes that were named but not yet expanded.
    while let Some(node) = p.pending_nodes.pop() {
        let name = p.names[&node].clone();
        let rendered = p.render(node);
        equations.push(format!("{name} = {rendered}"));
    }

    // CLM-0023: the explanation's goal instances, after the answer's names
    // are fixed; a cyclic node first seen here gets its equation too.
    let extra_rendered: Vec<String> = extra.iter().map(|a| p.render(*a)).collect();
    while let Some(node) = p.pending_nodes.pop() {
        let name = p.names[&node].clone();
        let rendered = p.render(node);
        equations.push(format!("{name} = {rendered}"));
    }

    // Pass 6: the numeric store, projected onto what the query can see and
    // simplified (design D7, stage 4).
    let mut constraints = render_numeric(&mut p, store, &reachable);

    // Pass 7 (CLM-0016): pending concatenations that mention something the
    // query can see, as `Z = X . Y`.
    for (z, x, y) in store.pending_concats() {
        if !reachable_vars(store, [z, x, y])
            .iter()
            .any(|v| reachable.contains(v))
        {
            continue;
        }
        p.find_cycles(z);
        p.find_cycles(x);
        p.find_cycles(y);
        let (rz, rx, ry) = (p.render(z), p.render(x), p.render(y));
        constraints.push(format!("{rz} = {rx} . {ry}"));
    }

    // Pass 8 (CLM-0017): pending type relations and `atom_chars`, in goal
    // form, when they mention something the query can see.
    for (v, kind) in store.pending_types() {
        if !reachable.contains(&store.deref(v)) {
            continue;
        }
        let name = match kind {
            TypeKind::Atom => "atom",
            TypeKind::List => "list",
        };
        let r = p.render(v);
        constraints.push(format!("{name}({r})"));
    }
    for (a, l) in store.pending_chars() {
        if !reachable_vars(store, [a, l])
            .iter()
            .any(|v| reachable.contains(v))
        {
            continue;
        }
        p.find_cycles(a);
        p.find_cycles(l);
        let (ra, rl) = (p.render(a), p.render(l));
        constraints.push(format!("atom_chars({ra}, {rl})"));
    }
    // CLM-0025: pending sums, in goal form.
    for (l, s) in store.pending_sums() {
        if !reachable_vars(store, [l, s])
            .iter()
            .any(|v| reachable.contains(v))
        {
            continue;
        }
        p.find_cycles(l);
        p.find_cycles(s);
        let (rl, rs) = (p.render(l), p.render(s));
        constraints.push(format!("sum({rl}, {rs})"));
    }

    (
        Answer {
            equations,
            disequations,
            constraints,
            explanation: None,
        },
        extra_rendered,
    )
}

/// The public numeric variables (alias-class roots) with their names: numeric
/// heap variables reachable from the query, and attribute terms created by
/// the query whose variables are all visible.
fn public_names(
    p: &mut Printer<'_>,
    store: &Store,
    reachable: &HashSet<Addr>,
) -> BTreeMap<SVar, String> {
    let mut names: BTreeMap<SVar, String> = BTreeMap::new();
    let mut heap_named: Vec<(SVar, Addr)> = reachable
        .iter()
        .filter_map(|v| store.numvar.get(v).map(|sv| (store.root(*sv), *v)))
        .collect();
    heap_named.sort();
    for (r, v) in heap_named {
        let n = p.name_for(v);
        names.entry(r).or_insert(n);
    }
    let mut attr_named: Vec<(SVar, Addr)> = store
        .attrs
        .iter()
        .enumerate()
        .filter(|(i, _)| store.attr_visible(*i))
        .filter(|(_, e)| {
            reachable_vars(store, [e.term])
                .iter()
                .all(|v| reachable.contains(v))
        })
        .map(|(_, e)| (store.root(e.svar), e.term))
        .collect();
    attr_named.sort();
    attr_named.dedup_by_key(|(r, _)| *r);
    for (r, term) in attr_named {
        if let std::collections::btree_map::Entry::Vacant(slot) = names.entry(r) {
            let rendered = p.render(term);
            slot.insert(rendered);
        }
    }
    // CLM-0015: the length of a reachable unbound list variable is public,
    // named `|L|` after the variable.
    let mut len_named: Vec<(SVar, Addr)> = store
        .lengths
        .iter()
        .filter(|e| e.pending && reachable.contains(&store.deref(e.list)))
        .map(|e| (store.root(e.svar), e.list))
        .collect();
    len_named.sort();
    len_named.dedup_by_key(|(r, _)| *r);
    for (r, list) in len_named {
        if let std::collections::btree_map::Entry::Vacant(slot) = names.entry(r) {
            let rendered = format!("|{}|", p.render(list));
            slot.insert(rendered);
        }
    }
    names
}

fn render_numeric(p: &mut Printer<'_>, store: &Store, reachable: &HashSet<Addr>) -> Vec<String> {
    let mut names = public_names(p, store, reachable);
    if names.is_empty() {
        return Vec::new();
    }
    let public: BTreeSet<SVar> = names.keys().copied().collect();
    let Projected {
        eqs,
        ineqs,
        difs,
        survivors,
    } = project(store, &public);
    // World attribute terms pulled in as survivors print by their term;
    // other survivors get internal names.
    let mut attr_render: BTreeMap<SVar, Addr> = BTreeMap::new();
    for e in &store.attrs {
        attr_render.entry(store.root(e.svar)).or_insert(e.term);
    }
    // Lengths of list variables (CLM-0015): rendered `|L|`; their implicit
    // lower bound `|L| >= 0` is part of the notation and is not printed.
    let mut len_render: BTreeMap<SVar, Addr> = BTreeMap::new();
    for e in store.lengths.iter().filter(|e| e.pending) {
        len_render.entry(store.root(e.svar)).or_insert(e.list);
    }
    for v in &survivors {
        if !names.contains_key(v) {
            let n = match (attr_render.get(v), len_render.get(v)) {
                (Some(term), _) => p.render(*term),
                (None, Some(list)) => format!("|{}|", p.render(*list)),
                (None, None) => p.fresh_name(),
            };
            names.insert(*v, n);
        }
    }
    let implicit_length_bound = |e: &LinExpr, strict: bool| -> bool {
        !strict
            && e.constant.is_zero()
            && e.terms.len() == 1
            && e.terms
                .iter()
                .next()
                .is_some_and(|(v, a)| *a == Number::one() && len_render.contains_key(v))
    };

    let render_expr = |e: &LinExpr| -> Option<String> {
        let mut out = String::new();
        let mut first = true;
        // A positive constant leads when the first term is negative: `10 - X`.
        let lead_const =
            e.constant.is_positive() && e.terms.iter().next().is_some_and(|(_, a)| a.is_negative());
        if lead_const {
            out.push_str(&e.constant.to_string());
            first = false;
        }
        for (v, a) in &e.terms {
            let name = names.get(v)?;
            if first {
                if a.is_negative() {
                    out.push('-');
                }
            } else if a.is_negative() {
                out.push_str(" - ");
            } else {
                out.push_str(" + ");
            }
            first = false;
            let m = a.abs();
            if m != Number::one() {
                out.push_str(&format!("{m}*"));
            }
            out.push_str(name);
        }
        if first {
            out.push_str(&e.constant.to_string());
        } else if lead_const {
            // already printed
        } else if e.constant.is_positive() {
            out.push_str(&format!(" + {}", e.constant));
        } else if e.constant.is_negative() {
            out.push_str(&format!(" - {}", e.constant.abs()));
        }
        Some(out)
    };
    // `terms op c` in a readable shape: a single variable `X op c`; a
    // two-variable difference `X op Y`; otherwise `expr op c`.
    let render_rel = |e: &LinExpr, op: &str| -> Option<String> {
        let c = -&e.constant;
        let mut terms = e.clone();
        terms.constant = Number::zero();
        if terms.terms.len() == 2 && c.is_zero() {
            let mut it = terms.terms.iter();
            let (v1, a1) = it.next().unwrap();
            let (v2, a2) = it.next().unwrap();
            if *a1 == Number::one() && *a2 == -Number::one() {
                return Some(format!("{} {op} {}", names.get(v1)?, names.get(v2)?));
            }
            if *a2 == Number::one() && *a1 == -Number::one() {
                return Some(format!("{} {op} {}", names.get(v2)?, names.get(v1)?));
            }
        }
        Some(format!("{} {op} {c}", render_expr(&terms)?))
    };
    let first_var = |e: &LinExpr| e.terms.keys().next().copied().unwrap_or(SVar(usize::MAX));

    let mut out = Vec::new();
    // Fixed public attribute terms: their value.
    for (r, name) in &names {
        if attr_render.contains_key(r) {
            if let Some(c) = store.class_value(*r) {
                if !reachable
                    .iter()
                    .any(|v| store.numvar.get(v).map(|sv| store.root(*sv)) == Some(*r))
                {
                    out.push(format!("{name} = {c}"));
                }
            }
        }
    }
    // The length relation `|Z| = |X| + |Y|` of a pending concatenation is
    // implied by the printed `Z = X . Y` (CLM-0016): drop it when it appears
    // as such, over three unbound list variables.
    let implied: Vec<BTreeMap<SVar, Number>> = store
        .pending_concats()
        .into_iter()
        .filter_map(|(z, x, y)| {
            let (lz, lx, ly) = (
                store.length_svar(z)?,
                store.length_svar(x)?,
                store.length_svar(y)?,
            );
            let mut m = BTreeMap::new();
            *m.entry(store.root(lz)).or_insert_with(Number::zero) += &Number::one();
            *m.entry(store.root(lx)).or_insert_with(Number::zero) -= &Number::one();
            *m.entry(store.root(ly)).or_insert_with(Number::zero) -= &Number::one();
            Some(m)
        })
        .collect();
    let is_implied = |subject: SVar, rhs: &LinExpr| -> bool {
        if !rhs.constant.is_zero() {
            return false;
        }
        let mut e = rhs.clone();
        e.negate();
        e.add_term(subject, &Number::one());
        implied.iter().any(|m| {
            e.terms.len() == m.len()
                && (e.terms.iter().all(|(v, a)| m.get(v) == Some(a))
                    || e.terms.iter().all(|(v, a)| m.get(v) == Some(&-a)))
        })
    };
    // Equalities in solved form, by subject.
    for (subject, rhs) in &eqs {
        if is_implied(*subject, rhs) {
            continue;
        }
        if let (Some(name), Some(r)) = (names.get(subject), render_expr(rhs)) {
            out.push(format!("{name} = {r}"));
        }
    }
    // Inequalities, deterministic order.
    let mut lines: Vec<(SVar, bool, String)> = ineqs
        .iter()
        .filter(|(e, strict)| !implicit_length_bound(e, *strict))
        .filter_map(|(e, strict)| {
            // e >= 0 (or > 0): orient by the leading coefficient's sign so a
            // single variable reads `X >= c` / `X <= c`; lower bounds first.
            let leading_negative = e.terms.iter().next().is_some_and(|(_, a)| a.is_negative());
            let (expr, op) = if leading_negative {
                let mut n = e.clone();
                n.negate();
                (n, if *strict { "<" } else { "<=" })
            } else {
                (e.clone(), if *strict { ">" } else { ">=" })
            };
            render_rel(&expr, op).map(|s| (first_var(&expr), leading_negative, s))
        })
        .collect();
    lines.sort();
    out.extend(lines.into_iter().map(|(_, _, s)| s));
    // Numeric disequations.
    let mut dlines: Vec<(SVar, String)> = difs
        .iter()
        .filter_map(|e| render_rel(e, "!=").map(|s| (first_var(e), s)))
        .collect();
    dlines.sort();
    out.extend(dlines.into_iter().map(|(_, s)| s));
    out
}
/// Unbound variables reachable from `roots` through bindings and structures.
fn reachable_vars(store: &Store, roots: impl IntoIterator<Item = Addr>) -> HashSet<Addr> {
    let mut vars = HashSet::new();
    let mut seen = HashSet::new();
    let mut stack: Vec<Addr> = roots.into_iter().collect();
    while let Some(a) = stack.pop() {
        let a = store.deref(a);
        if !seen.insert(a) {
            continue;
        }
        match store.cell(a) {
            Cell::Var(_) => {
                vars.insert(a);
            }
            Cell::Struct(_, args) => stack.extend(args.iter().copied()),
            _ => {}
        }
    }
    vars
}

/// Render terms with the query's variable names (for runtime error messages).
pub(crate) fn render_terms(
    store: &Store,
    query_vars: &[(String, Addr)],
    addrs: &[Addr],
) -> Vec<String> {
    let mut p = Printer::new(store);
    for (name, addr) in query_vars {
        let d = store.deref(*addr);
        if matches!(store.cell(d), Cell::Var(_)) {
            p.names.entry(d).or_insert_with(|| name.clone());
        }
    }
    for &a in addrs {
        p.find_cycles(a);
    }
    addrs.iter().map(|a| p.render(*a)).collect()
}

/// CLM-0032: a constraint goal's side as written — the clause template with
/// each variable replaced by its rendered binding (`vars`, by clause
/// variable index), arithmetic, lengths, concatenations and lists in source
/// form — printed by the AST's precedence-aware renderer, so parentheses
/// and quoting come out as the parser would read them back. Iterative: a
/// list spine is flattened in a loop (CLM-0019).
pub(crate) fn render_template(store: &Store, tpl: &TTerm, vars: &HashMap<u32, String>) -> String {
    enum Walk<'a> {
        Enter(&'a TTerm),
        Build(&'a TTerm),
        List { items: usize, tail: bool },
    }
    fn pop(out: &mut Vec<Expr>) -> Expr {
        out.pop().unwrap_or_else(|| Expr::Ident(String::from("?")))
    }
    let symbols = store.symbols.borrow();
    let mut work = vec![Walk::Enter(tpl)];
    let mut out: Vec<Expr> = Vec::new();
    while let Some(step) = work.pop() {
        match step {
            Walk::Enter(t) => match t {
                TTerm::Compound(f, args) if *f == LIST && args.len() == 2 => {
                    let mut items: Vec<&TTerm> = vec![&args[0]];
                    let mut cur = &args[1];
                    while let TTerm::Compound(g, next) = cur {
                        if *g != LIST || next.len() != 2 {
                            break;
                        }
                        items.push(&next[0]);
                        cur = &next[1];
                    }
                    let tail = !matches!(cur, TTerm::Const(c) if *c == LIST);
                    work.push(Walk::List {
                        items: items.len(),
                        tail,
                    });
                    if tail {
                        work.push(Walk::Enter(cur));
                    }
                    work.extend(items.iter().rev().map(|i| Walk::Enter(i)));
                }
                TTerm::Var(_) | TTerm::Number(_) | TTerm::Const(_) => work.push(Walk::Build(t)),
                TTerm::Compound(_, args) => {
                    work.push(Walk::Build(t));
                    work.extend(args.iter().rev().map(Walk::Enter));
                }
                TTerm::Neg(x) | TTerm::Len(x) => {
                    work.push(Walk::Build(t));
                    work.push(Walk::Enter(x));
                }
                TTerm::Arith(_, l, r) | TTerm::Concat(l, r) => {
                    work.push(Walk::Build(t));
                    work.push(Walk::Enter(r));
                    work.push(Walk::Enter(l));
                }
            },
            Walk::Build(t) => {
                let e = match t {
                    TTerm::Var(i) => {
                        Expr::Var(vars.get(i).cloned().unwrap_or_else(|| String::from("_")))
                    }
                    TTerm::Number(n) => Expr::Number(n.clone()),
                    TTerm::Const(c) if *c == LIST => Expr::List {
                        items: Vec::new(),
                        tail: None,
                    },
                    TTerm::Const(c) => Expr::Ident(symbols.name(*c).to_string()),
                    TTerm::Compound(f, args) => {
                        let args = out.split_off(out.len() - args.len());
                        Expr::Atom(Box::new(Atom {
                            name: symbols.name(*f).to_string(),
                            args,
                        }))
                    }
                    TTerm::Neg(_) => Expr::Neg(Box::new(pop(&mut out))),
                    TTerm::Len(_) => Expr::Len(Box::new(pop(&mut out))),
                    TTerm::Arith(op, ..) => {
                        let right = Box::new(pop(&mut out));
                        let left = Box::new(pop(&mut out));
                        Expr::Binary {
                            op: *op,
                            left,
                            right,
                        }
                    }
                    TTerm::Concat(..) => {
                        let r = Box::new(pop(&mut out));
                        let l = Box::new(pop(&mut out));
                        Expr::Concat(l, r)
                    }
                };
                out.push(e);
            }
            Walk::List { items, tail } => {
                let tail = tail.then(|| Box::new(pop(&mut out)));
                let items = out.split_off(out.len() - items);
                out.push(Expr::List { items, tail });
            }
        }
    }
    out.pop().map(|e| e.to_string()).unwrap_or_default()
}
