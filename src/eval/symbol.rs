// SPDX-License-Identifier: Apache-2.0

//! Interned symbols for functor and constant names.

use std::collections::HashMap;

/// An interned name. Equality is identity; the text lives in [`Symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

/// The reserved label of list cells and of the empty list (CLM-0014):
/// `'<>'(H, T)` is a cell, the nullary `'<>'` the empty list.
pub(crate) const LIST_LABEL: &str = "<>";

/// The symbol of [`LIST_LABEL`]: interned first in every table, so the
/// store can recognise and build list cells without the table.
pub(crate) const LIST: Symbol = Symbol(0);

/// The intern table.
#[derive(Debug, Clone)]
pub struct Symbols {
    names: Vec<String>,
    index: HashMap<String, Symbol>,
}

impl Default for Symbols {
    fn default() -> Self {
        let mut s = Symbols {
            names: Vec::new(),
            index: HashMap::new(),
        };
        let list = s.intern(LIST_LABEL);
        debug_assert_eq!(list, LIST);
        s
    }
}

impl Symbols {
    pub fn intern(&mut self, name: &str) -> Symbol {
        if let Some(&s) = self.index.get(name) {
            return s;
        }
        let s = Symbol(self.names.len() as u32);
        self.names.push(name.to_string());
        self.index.insert(name.to_string(), s);
        s
    }

    pub fn name(&self, s: Symbol) -> &str {
        &self.names[s.0 as usize]
    }
}
