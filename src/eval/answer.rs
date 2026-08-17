//! Answers in solved form: `X = t` for every bound query variable, aliases
//! `X = Y` for query variables that became one, pending disequations, and
//! `true` when nothing remains. Cyclic terms print as equations (`X = f(X)`),
//! which is the finite representation of a rational tree. The printer is
//! iterative so deep terms cannot overflow the stack.

use std::collections::{HashMap, HashSet};
use std::fmt;

use super::store::{Addr, Cell, Store};
use super::symbol::Symbols;

/// One answer to a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// `X = t` equations, query variables first, then named cyclic nodes.
    pub equations: Vec<String>,
    /// Pending disequations, rendered `t1 != t2`.
    pub disequations: Vec<String>,
}

impl Answer {
    /// True if the answer carries no equations or disequations (`true`).
    pub fn is_true(&self) -> bool {
        self.equations.is_empty() && self.disequations.is_empty()
    }
}

impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_true() {
            return f.write_str("true");
        }
        let mut first = true;
        for part in self.equations.iter().chain(self.disequations.iter()) {
            if !first {
                f.write_str(", ")?;
            }
            first = false;
            f.write_str(part)?;
        }
        Ok(())
    }
}

struct Printer<'a> {
    symbols: &'a Symbols,
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
    fn new(symbols: &'a Symbols, store: &'a Store) -> Self {
        Printer {
            symbols,
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
                    let Cell::Struct(_, args) = self.store.cell(a) else { continue };
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

    /// Render the term at `root`. If `root` is itself a cyclic node it is
    /// expanded here (its inner occurrences print as its name); other cyclic
    /// nodes print as names and are queued for their own equations.
    fn render(&mut self, root: Addr) -> String {
        enum Item {
            Term(Addr),
            Text(&'static str),
        }
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
                            let n = self.name_for(a);
                            out.push_str(&n);
                        }
                        Cell::Const(c) => out.push_str(self.symbols.name(c)),
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
                            out.push_str(self.symbols.name(f));
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
pub(crate) fn render_answer(
    symbols: &Symbols,
    store: &Store,
    query_vars: &[(String, Addr)],
) -> Answer {
    let mut p = Printer::new(symbols, store);
    let mut equations = Vec::new();

    // Projection (design D7, stage-2 form): a pending disequation belongs to
    // the answer only if it mentions a variable reachable from the query
    // variables. Disequations over internal variables alone are always
    // satisfiable (finitely many, over an infinite universe) and are dropped.
    let reachable = reachable_vars(store, query_vars.iter().map(|(_, a)| *a));
    let difs: Vec<(Addr, Addr)> = store
        .pending_difs()
        .into_iter()
        .filter(|(a, b)| {
            reachable_vars(store, [*a, *b]).iter().any(|v| reachable.contains(v))
        })
        .collect();

    // Pass 1: cycle detection over everything we will print.
    for (_, addr) in query_vars {
        p.find_cycles(*addr);
    }
    for (a, b) in &difs {
        p.find_cycles(*a);
        p.find_cycles(*b);
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

    // Pass 3: equations, in query-variable order.
    for ((name, addr), alias) in query_vars.iter().zip(aliases) {
        if let Some(alias) = alias {
            equations.push(alias);
            continue;
        }
        let d = store.deref(*addr);
        if !matches!(store.cell(d), Cell::Var(_)) {
            let rendered = p.render(d);
            equations.push(format!("{name} = {rendered}"));
        }
    }

    // Pass 4: disequations.
    let mut disequations = Vec::new();
    for (a, b) in difs {
        let l = p.render(a);
        let r = p.render(b);
        disequations.push(format!("{l} != {r}"));
    }

    // Pass 5: cyclic nodes that were named but not yet expanded.
    while let Some(node) = p.pending_nodes.pop() {
        let name = p.names[&node].clone();
        let rendered = p.render(node);
        equations.push(format!("{name} = {rendered}"));
    }

    Answer { equations, disequations }
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
