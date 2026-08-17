//! The evaluator — stage 2 of the evaluator design
//! (`docs/design/2026-08-17-evaluator.md`): rational-tree unification, `dif`
//! by trial unification with suspension, a compile step from the AST, an
//! iterative SLD machine with a trail, and answers in solved form.
//!
//! Numeric constraints (relations other than `=`/`!=`, arithmetic terms,
//! attribute terms) are rejected at compile time until stage 3 adds the
//! linear store.

mod answer;
mod compile;
mod error;
mod machine;
mod store;
mod symbol;

#[cfg(test)]
mod tests;

pub use answer::Answer;
pub use compile::{Program, Query};
pub use error::EvalError;
pub use machine::Solutions;
