---
date: 2026-11-22
type: design
status: proposed
components: [evaluator]
aspects: [answer-soundness]
tags: [answers, bindings, term, library, solved-form, genite]
---

# An answer carries its bindings as terms, beside the text

## Context

An `Answer` is text. Its `equations`, `disequations` and `constraints`
are `Vec<String>` — `A = 'cl-1472…'`, `X = f(X)`, `|T| >= 1` — produced
by the solved-form printer in `answer.rs` (design D7 of
`2026-08-17-evaluator.md`), and `--json` and the REPL print those strings
as they are. That is the right shape for a reader and for the JSON
contract, and until 2026-11-20 every consumer was a reader: the batch
mode, the REPL, aicogito's adapter over `--json`.

**genite links the library.** GEN-141 (`rust/crates/genite-mind-rules`,
incubation note `2026-11-20-answers-as-values.md`, graduated by this
record) compiles a rule file with `Program::compile_spanned`, runs each
query with `Program::solve`, and needs, for a rule
`contradiction(Rule, A, B)`, the three identifiers the answer bound. It
gets them today by splitting each equation at ` = `, comparing the left
side with the variable's name, and undoing the printer's quoting of an
atom by hand. That is a piece of the printer run backwards, outside the
crate: it breaks if the printer changes a space, it cannot tell the atom
`'3'` from the number `3` or the atom `'<>'` from the empty list, and it
would have to grow a term parser the day a rule answers with a compound.

**What the printer knows and throws away.** `render_answer` walks the
heap for every query variable with the cycle structure already found
(`find_cycles`), the display names already fixed — a query variable's
own name, `_N` for an internal variable or a cyclic node — and each
cell's kind in hand: an atom (a `Const`), a number (`Num`, or a numeric
variable whose value the store has determined), a list cell or the empty
list (`'<>'`), a compound, an unbound variable. It writes text at each
step. The same walk can build a value.

**The shapes to keep straight.** A cyclic term has no finite tree; the
solved form writes it as equations, `X = f(X)` and `_1 = g(_1, X)`, which
is the finite representation the evaluator design chose. A string is a
list of one-character atoms and prints in list form, `<a, b>`, because
the printer never guesses a string reading
(`2026-11-09-strings-as-character-lists-and-type-relations.md`, decision
2). An atom's name is spelled quoted only where the syntax needs it. A
variable that is unbound at the answer has no equation; what the answer
knows about it is in `constraints` or `disequations`, or nothing. An
alias `A = B` is an equation between two unbound query variables.

**Prior art.** Every Prolog with an embedding API answers with terms,
not with text: SWI-Prolog's `PL_get_*` over `term_t`, Scryer's and
Trealla's Rust bindings, SICStus' `SP_term_ref`. Each gives a leaf for
atoms, numbers and variables, a compound with functor and arguments, and
a list, and leaves strings to the embedding program. miniKanren's
reifier returns the value with `_.0`, `_.1` for the fresh variables,
which is the `_N` naming the printer already uses.

## Decision

1. **A public term type, `claimr::Term`, is the value of a binding.**

   ```rust
   pub enum Term {
       Atom(String),                       // the name, unquoted
       Number(Number),                     // exact
       Var(String),                        // a query variable's name, or `_N`
       Compound(String, Vec<Term>),        // functor name, unquoted, and arguments
       List(Vec<Term>, Option<Box<Term>>), // items, and the tail when improper
   }
   ```

   It is what an answer's heap holds and nothing more: no arithmetic, no
   comprehension, no concatenation, no length, which exist in `ast::Expr`
   because a program is written with them, and never in a value. An atom
   or functor carries its name as content, `cl-1472…` not `'cl-1472…'`:
   the representation `ast::Expr::Ident`, `ast::Atom::name` and the
   symbol table already use (`2026-09-10-quoted-atoms.md`, decision 2:
   identity is content, quoting is spelling), so a consumer compares it
   with the name it wrote into a fact. The one quoted name in the public
   API, `UndefinedCall::name` (CLM-0036), is a diagnostic to be read, and
   stays so. `Term`'s `Display` writes the solved-form syntax — `display_name`
   quoting, `<a, b>` for a list, `<a> . T` for an improper one, `<>` for
   the empty list — and is the one renderer of a term, so the text and
   the value cannot disagree. A list is `List`, never `Compound("<>", …)`;
   a string is a list of one-character atoms, in this type as in print.
   `Display` and `Drop` are iterative, as `ProofNode`'s are, because a
   term can be deep.

2. **`Answer` gains `bindings: Vec<Binding>`, with
   `Binding { name: String, value: Term }`, and
   `Answer::binding(&str) -> Option<&Term>`.** One entry per query
   variable that has an equation, in query-variable order, then one per
   named cyclic node (`_N`), in the order the printer names them. The
   text fields stay as they are and are produced in the same pass: an
   equation is `name = value` rendered, so a test can assert
   `equations[i] == format!("{} = {}", b.name, b.value)` for every
   non-alias binding. A query variable has at most one binding.

3. **The shapes.**
   - *Bound to a tree*: the tree, `Compound`, `Atom`, `Number`, `List`;
     a query variable or an unbound internal variable inside it is
     `Var(name)`, the name the text uses.
   - *A numeric variable the store has determined*: `Number`, as the
     text prints the value — the equation `X = 3` and the binding agree.
   - *Unbound, or only constrained*: no binding; `binding("X")` is
     `None`. The constraints on it remain in `constraints` and
     `disequations`, text, and `Term::Var("X")` is how it appears in
     another variable's value. There is no `Term::Unbound`: a variable's
     freedom is the absence of an equation, as in the solved form.
   - *An alias* `A = B` (two unbound query variables made one): the text
     keeps its orientation, earlier variable first; the binding is on the
     later variable, `("B", Var("A"))`, so each query variable has one
     binding and the earlier one, which stands for the class, has none.
   - *A cyclic term*: the solved form's equations as bindings.
     `X = f(X)` is `("X", Compound("f", [Var("X")]))`; a cyclic node
     reached inside another value is `Var("_1")` where it recurs, with
     its own binding `("_1", …)` appended after the query variables,
     exactly as the text writes `_1 = …`. A consumer that wants the
     rational tree follows the names; one that wants ground trees sees a
     `Var` and knows.

4. **Not in this record.** Disequations and numeric constraints as
   values: a disequation is a pair of terms and would be cheap, a
   numeric constraint needs a linear form over named unknowns, and
   neither has a consumer; one record when one asks. Proof nodes as
   terms (`ProofNode.goal` as a `Term` beside its text): the same walk
   renders the goal instances, so the extension is mechanical, but
   genite reads explanations as lines for a person. A `bindings` field
   in the `--json` answer object: additive and plausible, but a JSON
   encoding of a term (atom against string against variable) is a
   contract of its own; aicogito's adapter has not asked. A string
   variant or an `as_text()` on `Term`: the strings record decided the
   printer never guesses a string reading, and the value type keeps to
   that; folding a list of one-character atoms is a line for the
   embedding program.

## Alternatives weighed

- **Expose the heap** — `Store`, `Addr`, `Cell` — behind the lifetime of
  a `Solutions`. It gives the full rational tree with no copy, and ties
  every consumer to the machine's representation, its address type and
  its borrow; the next change to the store becomes a consumer break.
  Rejected: the answer is a value, the heap is how it is computed.
- **Reuse `ast::Expr`.** Already public, already has `Atom`, `Number`,
  `Var`, `List`. It also has `Neg`, `Binary`, `Len`, `Concat` and
  `Comprehension`, none of which a value contains, so every consumer
  would match arms that cannot occur; and its `Atom` is a struct that
  mixes `Ident` and `Atom` the way the parser needs, not the way a
  value does. Rejected: a value type should say what can be there.
- **A public term parser** — `claimr::parse_term(&str)` over the printed
  text. The reverse of the printer, inside the crate instead of in
  genite, run on every binding; it would also need the cycle names and
  the `_N` convention to be syntax. Rejected: the value exists before it
  is printed; printing and re-parsing it is work to recover what was
  thrown away.
- **A JSON-like generic value** (`serde_json::Value` or a copy of it).
  Loses the exact rational to a float or a string, and cannot tell an
  atom from a string from a variable without a convention on top.
  Rejected on `exact-arithmetic` alone.
- **Cycles by reference** — `Rc` cells or an explicit `Ref(usize)` into
  a node table. A real graph, but one a consumer cannot pattern-match
  without a traversal API, and different from the text. Rejected in
  favour of the solved form's own device, named equations, which the
  text already uses and Prolog III defined.

## Consequences

- **genite's rule runner drops its printer-in-reverse.** `bound("Rule")`
  becomes `answer.binding("Rule")` matched on `Term::Atom(name)`, and
  the `named` unquoting goes; a rule answering with a compound or a list
  needs no new code on genite's side. aicogito's adapter, on `--json`, is
  untouched.
- **`Answer` grows a field.** `bindings` is public like the others; a
  consumer constructing `Answer` by struct literal would break, and none
  does (the crate builds every answer). This is a 0.x minor change,
  0.7 → 0.8, with genite as the release's reason.
- **One walk, two outputs.** The printer builds the `Term` and renders
  the text from it in the same pass, or renders both from the same walk;
  the task decides, with the test of decision 2 as the guard. Building
  the term costs an allocation per node per answer; the batch mode and
  the REPL pay it for nothing they use, which is acceptable at the
  sizes claimr runs (the benchmarks of `docs/reference/benchmarks.md`
  will say if it is not, and a lazy variant is an option then).
- **Soundness.** Untouched: the bindings are the same solved form the
  text shows, produced from the same store after the same `finalize`;
  nothing is decided differently and nothing is reported that the text
  does not. The aspect doc gains no sentence; `evaluator.md` documents
  the type and the field.
- **Deep and cyclic terms** are tested where the printer is: a term
  deeper than the stack, a cyclic `X = f(X)`, a cyclic list `X = <1> . X`,
  an alias, a determined numeric variable, an improper list, a quoted
  atom, the empty list, and the agreement of text and value on every
  example program with a `.answers` file.
- **One task** adds `Term` and `Binding`, the field and the accessor, the
  renderer change, the tests above, `evaluator.md`, the README's library
  paragraph, and a note in `CHANGELOG.md` under the next version.

## Sources

- Incubation note `2026-11-20-answers-as-values.md` (graduated by this
  record); genite GEN-141, `rust/crates/genite-mind-rules/src/lib.rs`,
  the `bound` closure and `named`.
- `docs/design/2026-08-17-evaluator.md`, D7: answers as the store in
  solved form; cyclic terms as equations.
- `docs/design/2026-11-09-strings-as-character-lists-and-type-relations.md`,
  decision 2: printing is list form.
- `docs/design/2026-11-09-explanations-of-answers.md`: the printer shared
  between an answer and its explanation, and `ProofNode`'s iterative
  `Drop`.
- `src/eval/answer.rs` (`render_answer`, `Printer`), `src/ast.rs`
  (`Expr`, `display_name`).
- SWI-Prolog foreign interface, `PL_get_atom_chars`, `PL_get_arg`,
  `PL_get_list`; Byrd, Friedman, Kiselyov, *The Reasoned Schemer*,
  reification of fresh variables as `_.N`.
