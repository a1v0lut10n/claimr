// SPDX-License-Identifier: Apache-2.0

//! Errors raised while compiling or running a program.

use std::fmt;

use crate::Span;

/// An error compiling or loading a program.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
    /// The program's constraint facts are jointly unsatisfiable: it has no
    /// models, so no query could ever succeed.
    InitialStoreUnsatisfiable,
    /// A non-linear constraint (a product or quotient of two unknowns) was
    /// still pending when an answer would have been produced. Claimr does not
    /// approximate: the query stops instead of printing an unchecked answer.
    NonLinear { constraint: String },
    /// An attribute term (a compound in numeric position) is cyclic; such
    /// terms have no finite key and are not supported.
    CyclicAttributeTerm { term: String },
    /// A clause defines a predicate whose name is reserved: `'<>'` is the
    /// list constructor (CLM-0014). `span` is the clause's position when the
    /// program was parsed with positions.
    ReservedName { name: String, span: Option<Span> },
    /// A clause defines a predicate that is a built-in (CLM-0017):
    /// `atom_chars/2`, `number/1`, `atom/1`, `list/1`.
    ReservedBuiltin {
        name: String,
        arity: usize,
        span: Option<Span>,
    },
    /// A condition — of an if-then-else or a negation (CLM-0022) — was
    /// reached while one of its external variables (shared with the clause
    /// outside the construct) was not ground. Deciding it would depend on
    /// goal order, so the query stops instead
    /// (docs/design/2026-11-09-negation-disjunction-and-if-then-else.md).
    NonGroundCondition {
        /// The construct as written.
        construct: String,
        /// The variable by its source name.
        variable: String,
        /// The clause (or query) as written.
        clause: String,
    },
    /// A list comprehension (CLM-0025) in a clause head or a constraint
    /// fact: a head is a pattern that is matched, not a goal that runs.
    /// `span` is the clause's position when parsed with positions.
    ComprehensionInHead { span: Option<Span> },
    /// A solution of a comprehension's body (CLM-0025) whose instance of
    /// the template is not ground — it carries an unbound variable, an
    /// undetermined numeric variable, or a cycle — so it has no single value
    /// to collect; the query stops rather than copying it with fresh
    /// variables and dropping its constraints.
    NonGroundSolution {
        /// The comprehension as written.
        construct: String,
        /// The instance, rendered.
        instance: String,
    },
    /// A directive (CLM-0027) the compile step rejects: an unknown
    /// directive name, an indicator whose arity is not a non-negative
    /// integer, or a table declared for a built-in or the list constructor.
    /// `span` is the clause's position when parsed with positions.
    BadDirective { message: String, span: Option<Span> },
    /// A solution of a tabled predicate (CLM-0027) whose call is not a
    /// finite ground term — it carries an unbound variable, an undetermined
    /// numeric variable or a cycle — so it cannot be recorded as an answer;
    /// tables hold ground answers only.
    NonGroundTabledAnswer {
        /// The predicate, as `name/arity`.
        predicate: String,
        /// The call instance, rendered.
        instance: String,
    },
    /// A solution of a tabled predicate (CLM-0027) whose call is ground but
    /// which holds under a constraint on the world's unknowns (an attribute
    /// term's bound, a disequation): recording it as a fact would drop the
    /// condition, and a later caller whose store contradicts it would be
    /// answered wrongly. Tables hold unconditional answers only.
    ConstrainedTabledAnswer {
        /// The predicate, as `name/arity`.
        predicate: String,
        /// The call instance, rendered.
        instance: String,
        /// The residual constraints, rendered.
        residual: String,
    },
    /// A condition or a comprehension body (a sub-search) consumed a table
    /// still under evaluation (CLM-0027): deciding it on a partial answer
    /// set would be unsound, so the query stops.
    IncompleteTable {
        /// The predicate, as `name/arity`.
        predicate: String,
        /// The construct that would be decided.
        construct: String,
    },
    /// A call to a tabled predicate (CLM-0027) is a cyclic term, which has
    /// no finite variant key.
    CyclicTabledCall { call: String },
    /// The query reached its budget of resolution steps (CLM-0029,
    /// `--steps N` / `:steps N` / `Solutions::with_step_limit`): a resource
    /// bound the caller set, not a judgment about the query. `goal` is the
    /// goal the machine was about to resolve, rendered (in backticks) or
    /// named by kind.
    StepLimit { steps: u64, goal: String },
    /// An internal invariant of the evaluator failed; reported rather than
    /// panicked so the CLI and REPL stay up. Never expected.
    InternalInvariant { what: String },
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::InitialStoreUnsatisfiable => {
                f.write_str("the program's constraint facts are unsatisfiable (no models)")
            }
            EvalError::NonLinear { constraint } => write!(
                f,
                "non-linear constraint `{constraint}` is still undetermined; claimr does not approximate (evaluator stage 3 supports linear constraints only)"
            ),
            EvalError::CyclicAttributeTerm { term } => {
                write!(
                    f,
                    "cyclic attribute term `{term}` in numeric position is not supported"
                )
            }
            EvalError::ReservedName { name, .. } => write!(
                f,
                "`{}` is reserved (the list constructor) and cannot be defined as a predicate",
                crate::ast::display_name(name)
            ),
            EvalError::ReservedBuiltin { name, arity, .. } => write!(
                f,
                "`{}/{arity}` is a built-in predicate and cannot be defined",
                crate::ast::display_name(name)
            ),
            EvalError::NonGroundCondition {
                construct,
                variable,
                clause,
            } => write!(
                f,
                "condition `{construct}` reached with `{variable}` unbound, in `{clause}`; a variable shared with the clause outside the construct must be bound before it (bind it earlier, or keep it local to the construct)"
            ),
            EvalError::ComprehensionInHead { .. } => f.write_str(
                "a list comprehension `<T | Body>` cannot appear in a clause head or a constraint fact; move it into the body",
            ),
            EvalError::NonGroundSolution {
                construct,
                instance,
            } => write!(
                f,
                "a solution of `{construct}` has the instance `{instance}`, which is not ground, so there is no single value to collect; constrain the template's variables inside the comprehension"
            ),
            EvalError::BadDirective { message, .. } => write!(f, "bad directive: {message}"),
            EvalError::NonGroundTabledAnswer {
                predicate,
                instance,
            } => write!(
                f,
                "a solution of the tabled predicate `{predicate}` is `{instance}`, which is not a finite ground term; a table holds ground answers only, so constrain the arguments inside the predicate or leave it untabled"
            ),
            EvalError::ConstrainedTabledAnswer {
                predicate,
                instance,
                residual,
            } => write!(
                f,
                "a solution of the tabled predicate `{predicate}` is `{instance}` only under `{residual}`; a table holds unconditional answers, so a constrained one cannot be recorded (decide the constraint inside the predicate, or leave it untabled)"
            ),
            EvalError::IncompleteTable {
                predicate,
                construct,
            } => write!(
                f,
                "`{construct}` consumes the table of `{predicate}` while it is still being computed (a negation or an aggregate inside the predicate's own recursion); its answers are not complete there, so the construct cannot be decided"
            ),
            EvalError::CyclicTabledCall { call } => write!(
                f,
                "the call `{call}` to a tabled predicate is a cyclic term, which has no finite table key"
            ),
            EvalError::StepLimit { steps, goal } => write!(
                f,
                "step limit of {steps} reached at {goal}; the query may not terminate (an untabled recursion over cyclic data?) — raise the limit, or table the predicate"
            ),
            EvalError::InternalInvariant { what } => {
                write!(f, "internal error: {what} (please report)")
            }
        }
    }
}

impl EvalError {
    /// The source position this error points at, if known (1-based line and
    /// column, as in `ParseError`); the CLI prints `file:line:col:` when
    /// there is one.
    pub fn span(&self) -> Option<Span> {
        match self {
            EvalError::ReservedName { span, .. }
            | EvalError::ReservedBuiltin { span, .. }
            | EvalError::ComprehensionInHead { span }
            | EvalError::BadDirective { span, .. } => *span,
            EvalError::InitialStoreUnsatisfiable
            | EvalError::NonLinear { .. }
            | EvalError::CyclicAttributeTerm { .. }
            | EvalError::NonGroundCondition { .. }
            | EvalError::NonGroundSolution { .. }
            | EvalError::NonGroundTabledAnswer { .. }
            | EvalError::ConstrainedTabledAnswer { .. }
            | EvalError::IncompleteTable { .. }
            | EvalError::CyclicTabledCall { .. }
            | EvalError::StepLimit { .. }
            | EvalError::InternalInvariant { .. } => None,
        }
    }
}
