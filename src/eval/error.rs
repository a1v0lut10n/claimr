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
            EvalError::ReservedName { span, .. } | EvalError::ReservedBuiltin { span, .. } => *span,
            EvalError::InitialStoreUnsatisfiable
            | EvalError::NonLinear { .. }
            | EvalError::CyclicAttributeTerm { .. }
            | EvalError::InternalInvariant { .. } => None,
        }
    }
}
