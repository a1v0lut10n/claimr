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
                write!(f, "cyclic attribute term `{term}` in numeric position is not supported")
            }
        }
    }
}

impl EvalError {
    /// The source position this error points at, if known (1-based line and
    /// column, as in `ParseError`). No current error carries one; the hook
    /// stays so the CLI prints `file:line:col:` uniformly when one does.
    pub fn span(&self) -> Option<Span> {
        match self {
            EvalError::InitialStoreUnsatisfiable
            | EvalError::NonLinear { .. }
            | EvalError::CyclicAttributeTerm { .. } => None,
        }
    }
}
