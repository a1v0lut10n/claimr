//! Errors raised while compiling or running a program.

use std::fmt;

use crate::Span;

/// An error compiling or loading a program.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EvalError {
    /// A construct the evaluator does not support yet. `clause` is the
    /// 1-based index of the offending clause in the program, `span` where it
    /// starts (line and column, when the clauses were parsed with spans), and
    /// `text` renders it.
    Unsupported { clause: usize, span: Option<Span>, text: String, what: String },
    /// The program's constraint facts are jointly unsatisfiable: it has no
    /// models, so no query could ever succeed.
    InitialStoreUnsatisfiable,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Unsupported { clause, span, text, what } => {
                if let Some(span) = span {
                    write!(f, "{span}: ")?;
                }
                write!(f, "clause {clause} `{text}`: {what}")
            }
            EvalError::InitialStoreUnsatisfiable => {
                f.write_str("the program's constraint facts are unsatisfiable (no models)")
            }
        }
    }
}

impl EvalError {
    /// The source position this error points at, if known (1-based line and
    /// column, as in `ParseError`).
    pub fn span(&self) -> Option<Span> {
        match self {
            EvalError::Unsupported { span, .. } => *span,
            EvalError::InitialStoreUnsatisfiable => None,
        }
    }
}
