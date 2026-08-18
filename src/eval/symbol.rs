// SPDX-License-Identifier: Apache-2.0

//! Interned symbols for functor and constant names.

use std::collections::HashMap;

/// An interned name. Equality is identity; the text lives in [`Symbols`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(u32);

/// The intern table.
#[derive(Debug, Default, Clone)]
pub struct Symbols {
    names: Vec<String>,
    index: HashMap<String, Symbol>,
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
