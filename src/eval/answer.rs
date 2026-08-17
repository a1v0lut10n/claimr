//! Answers in solved form: `X = t` for every bound query variable, aliases
//! `X = Y` for query variables that became one, pending disequations, and
//! `true` when nothing remains. Cyclic terms print as equations (`X = f(X)`),
//! which is the finite representation of a rational tree. The printer is
//! iterative so deep terms cannot overflow the stack.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use crate::solver::{Delta, LinExpr, SVar};
use crate::Number;

use super::store::{Addr, Cell, Store};
use super::symbol::Symbols;

/// One answer to a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// `X = t` equations, query variables first, then named cyclic nodes.
    pub equations: Vec<String>,
    /// Pending tree disequations, rendered `t1 != t2`.
    pub disequations: Vec<String>,
    /// Numeric constraints: bounds, linear equations, numeric disequations.
    pub constraints: Vec<String>,
}

impl Answer {
    /// True if the answer carries nothing (`true`).
    pub fn is_true(&self) -> bool {
        self.equations.is_empty() && self.disequations.is_empty() && self.constraints.is_empty()
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
                            // A numeric variable with a determined value prints as the value.
                            let fixed = self.store.numvar.get(&a).and_then(|sv| self.store.class_value(*sv));
                            match fixed {
                                Some(c) => out.push_str(&c.to_string()),
                                None => {
                                    let n = self.name_for(a);
                                    out.push_str(&n);
                                }
                            }
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
        } else if let Some(c) = store.numvar.get(&d).and_then(|sv| store.cheap_value(*sv)) {
            equations.push(format!("{name} = {c}"));
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

    // Pass 6: the numeric store, restricted to what the query can see.
    let constraints = render_numeric(&mut p, store, &reachable);

    Answer { equations, disequations, constraints }
}

/// Numeric constraints over solver variables the query can see: determined
/// values (`age(bob) = 3`), bounds (`X > 3`), linear equations (`Y = X + 1`),
/// constraint rows (`X + Y <= 10`), numeric disequations. Alias classes print
/// as one variable; fully determined lines are omitted (their values already
/// show); constraints of the world (the initial store) show only when they
/// touch something the query mentions.
fn render_numeric(p: &mut Printer<'_>, store: &Store, reachable: &HashSet<Addr>) -> Vec<String> {
    let root = |sv: SVar| store.root(sv);
    let fixed = |sv: SVar| store.class_value(sv);

    // Names for alias classes (by root), in a deterministic order.
    let mut names: BTreeMap<SVar, String> = BTreeMap::new();
    let mut heap_named: Vec<(SVar, Addr)> = reachable
        .iter()
        .filter_map(|v| store.numvar.get(v).map(|sv| (root(*sv), *v)))
        .collect();
    heap_named.sort();
    for (r, v) in heap_named {
        let n = p.name_for(v);
        names.entry(r).or_insert(n);
    }
    // Attribute terms created by the query whose variables are all visible.
    let mut attr_named: Vec<(SVar, Addr)> = store
        .attrs
        .iter()
        .enumerate()
        .filter(|(i, _)| *i >= store.baseline_attrs)
        .filter(|(_, e)| reachable_vars(store, [e.term]).iter().all(|v| reachable.contains(v)))
        .map(|(_, e)| (root(e.svar), e.term))
        .collect();
    attr_named.sort();
    attr_named.dedup_by_key(|(r, _)| *r);
    for (r, term) in attr_named {
        if let std::collections::btree_map::Entry::Vacant(slot) = names.entry(r) {
            let rendered = p.render(term);
            slot.insert(rendered);
        }
    }
    if names.is_empty() {
        return Vec::new();
    }
    // Any attribute term (world or query) can name its class if the closure
    // pulls it in — better than an internal `_N`.
    let mut attr_render: BTreeMap<SVar, Addr> = BTreeMap::new();
    for e in &store.attrs {
        attr_render.entry(root(e.svar)).or_insert(e.term);
    }
    // Normalise an expression: expand defined variables (arithmetic results
    // and constraint slacks) by their definitions — permanent identities —
    // map variables to roots, substitute fixed classes by their values, merge
    // like terms.
    fn normalise_into(store: &Store, e: &LinExpr, k: &Number, out: &mut LinExpr, depth: usize) {
        out.constant += &(&e.constant * k);
        for (v, a) in &e.terms {
            let ak = a * k;
            if depth < 64 {
                if let Some(def) = store.defs.get(v) {
                    normalise_into(store, def, &ak, out, depth + 1);
                    continue;
                }
            }
            let r = store.root(*v);
            match store.class_value(r) {
                Some(c) => out.constant += &(&c * &ak),
                None => out.add_term(r, &ak),
            }
        }
    }
    let normalise = |e: &LinExpr| -> LinExpr {
        let mut out = LinExpr::constant(Number::zero());
        normalise_into(store, e, &Number::one(), &mut out, 0);
        out
    };
    let is_def = |sv: SVar| store.defs.contains_key(&sv);

    // Close over rows and bounded definitions touching named classes; name
    // internal classes `_N` as needed.
    let mut changed = true;
    while changed {
        changed = false;
        let known: Vec<SVar> = names.keys().copied().collect();
        for r in known {
            if fixed(r).is_some() {
                continue;
            }
            if let Some(row) = store.class_row(r) {
                for v in normalise(row).terms.keys() {
                    if !names.contains_key(v) && !is_def(*v) {
                        let n = match attr_render.get(v) {
                            Some(term) => p.render(*term),
                            None => p.fresh_name(),
                        };
                        names.insert(*v, n);
                        changed = true;
                    }
                }
            }
        }
        for (s, def) in &store.defs {
            if store.simplex.lower(*s).is_none() && store.simplex.upper(*s).is_none() {
                continue;
            }
            let e = normalise(def);
            if e.is_constant() || !e.terms.keys().any(|v| names.contains_key(v)) {
                continue;
            }
            for v in e.terms.keys() {
                if !names.contains_key(v) && !is_def(*v) {
                    let n = match attr_render.get(v) {
                        Some(term) => p.render(*term),
                        None => p.fresh_name(),
                    };
                    names.insert(*v, n);
                    changed = true;
                }
            }
        }
    }

    let render_expr = |e: &LinExpr| -> Option<String> {
        let mut out = String::new();
        let mut first = true;
        // A positive constant leads when the first term is negative: `10 - X`.
        let lead_const = e.constant.is_positive()
            && e.terms.iter().next().is_some_and(|(_, a)| a.is_negative());
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
    // A constraint line `terms op c` in a readable shape: an equality is
    // solved for its most recently introduced unit-coefficient variable
    // (`Y = X + 1`); a two-variable difference prints as `X op Y`; otherwise
    // `expr op c`.
    let render_line = |terms: &LinExpr, op: &str, c: &Number| -> Option<String> {
        if op == "=" {
            let pick = terms
                .terms
                .iter()
                .rev()
                .find(|(_, a)| a.abs() == Number::one())
                .or_else(|| terms.terms.iter().next_back())
                .map(|(v, a)| (*v, a.clone()))?;
            let (v, a) = pick;
            let name = names.get(&v)?;
            // a·v + rest = c  ⇒  v = (c − rest) / a
            let mut rest = terms.clone();
            rest.take(v);
            rest.negate();
            rest.constant = c.clone();
            let inv = a.recip()?;
            rest.scale(&inv);
            return Some(format!("{name} = {}", render_expr(&rest)?));
        }
        if terms.terms.len() == 2 && c.is_zero() {
            let mut it = terms.terms.iter();
            let (v1, a1) = it.next().unwrap();
            let (v2, a2) = it.next().unwrap();
            let (pos, neg) = if *a1 == Number::one() && *a2 == -Number::one() {
                (v1, v2)
            } else if *a2 == Number::one() && *a1 == -Number::one() {
                (v2, v1)
            } else {
                return Some(format!("{} {op} {c}", render_expr(terms)?));
            };
            return Some(format!("{} {op} {}", names.get(pos)?, names.get(neg)?));
        }
        Some(format!("{} {op} {c}", render_expr(terms)?))
    };
    let bound_lines = |terms: &LinExpr, lower: Option<Delta>, upper: Option<Delta>, out: &mut Vec<String>| {
        if let (Some(l), Some(u)) = (&lower, &upper) {
            if l == u && l.is_exact() {
                if let Some(line) = render_line(terms, "=", &l.c) {
                    out.push(line);
                }
                return;
            }
        }
        if let Some(l) = lower {
            let op = if l.k.is_positive() { ">" } else { ">=" };
            if let Some(line) = render_line(terms, op, &l.c) {
                out.push(line);
            }
        }
        if let Some(u) = upper {
            let op = if u.k.is_negative() { "<" } else { "<=" };
            if let Some(line) = render_line(terms, op, &u.c) {
                out.push(line);
            }
        }
    };

    let mut out = Vec::new();
    // Fixed heap variables print inline (query variables in the equations
    // pass, inner ones as their value); fixed attribute terms print here.
    let attr_classes: HashSet<SVar> = store.attrs.iter().map(|e| root(e.svar)).collect();
    for (r, name) in &names {
        if let Some(c) = fixed(*r) {
            if attr_classes.contains(r) {
                out.push(format!("{name} = {c}"));
            }
            continue;
        }
        let (lower, upper) = store.class_bounds(*r);
        bound_lines(&LinExpr::var(*r), lower, upper, &mut out);
        if let Some(row) = store.class_row(*r) {
            let e = normalise(row);
            let trivial = e.as_var() == Some(*r) || e.terms.contains_key(r);
            if !trivial {
                if let Some(rhs) = render_expr(&e) {
                    out.push(format!("{name} = {rhs}"));
                }
            }
        }
    }
    let mut defs: Vec<(&SVar, &LinExpr)> = store.defs.iter().collect();
    defs.sort_by_key(|(s, _)| **s);
    for (s, def) in defs {
        let (lower, upper) = (store.simplex.lower(*s).cloned(), store.simplex.upper(*s).cloned());
        if lower.is_none() && upper.is_none() {
            continue;
        }
        let e = normalise(def);
        if e.is_constant() || !e.terms.keys().any(|v| names.contains_key(v)) {
            continue;
        }
        // s = e = terms + k  ⇒  terms op c - k
        let k = e.constant.clone();
        let mut terms = e.clone();
        terms.constant = Number::zero();
        let shift = |d: Option<Delta>| d.map(|d| Delta::new(&d.c - &k, d.k));
        bound_lines(&terms, shift(lower), shift(upper), &mut out);
    }
    for (i, nd) in store.numdifs.iter().enumerate() {
        if !nd.pending || i < store.baseline_numdifs {
            continue;
        }
        let vars = reachable_vars(store, [nd.a, nd.b]);
        if !vars.iter().all(|v| reachable.contains(v)) {
            continue;
        }
        let l = p.render(nd.a);
        let r = p.render(nd.b);
        out.push(format!("{l} != {r}"));
    }
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
    symbols: &Symbols,
    store: &Store,
    query_vars: &[(String, Addr)],
    addrs: &[Addr],
) -> Vec<String> {
    let mut p = Printer::new(symbols, store);
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
