// SPDX-License-Identifier: Apache-2.0

//! The linear constraint solver over exact rationals — the pure part of the
//! `solver` component (design: `docs/design/2026-08-17-evaluator.md` D5,
//! `docs/tasks/2026-08/2026-08-18-clm-0006-linear-store.md`).
//!
//! A general simplex in the style of Dutertre & de Moura (*A Fast
//! Linear-Arithmetic Solver for DPLL(T)*, CAV 2006): a tableau of linear rows,
//! per-variable bounds, Bland's rule, incremental bound assertion with
//! backtracking by restoring bounds. Strict bounds use δ-rationals. All
//! arithmetic is exact ([`crate::Number`]); there is no floating point.

mod delta;
mod linexpr;
mod simplex;

pub use delta::Delta;
pub use linexpr::LinExpr;
pub use simplex::{Bound, Mark, RelOp, SVar, Simplex};
