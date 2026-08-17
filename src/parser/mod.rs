//! Parser module — rustemo-generated LR parser for the Claimr grammar.
//!
//! - `claimr.rustemo` is the authoritative grammar.
//! - The parser tables are generated into `OUT_DIR` at build time.
//! - `claimr_actions.rs` (source tree, committed) maps productions onto the
//!   AST in [`crate::ast`]; rustemo preserves manual edits to it.

rustemo::rustemo_mod!(pub(crate) claimr, "/src/parser");

#[allow(unused)]
#[rustfmt::skip]
pub(crate) mod claimr_actions;
