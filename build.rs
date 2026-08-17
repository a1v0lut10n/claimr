//! Generates the claimr parser from `src/parser/claimr.rustemo` with rustemo.
//!
//! The parser tables go to `OUT_DIR` (regenerated, not committed); the actions
//! file `src/parser/claimr_actions.rs` stays in the source tree, is committed,
//! and may be customised — rustemo preserves manual changes and only appends
//! actions for new productions.

use std::process::exit;

fn main() {
    let settings = rustemo_compiler::Settings::new().actions_in_source_tree();

    if let Err(e) = settings.process_dir() {
        eprintln!("rustemo grammar compilation failed:\n{e}");
        exit(1);
    }
}
