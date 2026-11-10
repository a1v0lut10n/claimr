// SPDX-License-Identifier: Apache-2.0

//! CLM-0027 (docs/design/2026-11-10-tabling-for-recursive-rules.md): the
//! tables of the program's tabled predicates. A table is keyed by a call's
//! *variant* — the call as a template with its unbound variables numbered
//! in order of first occurrence — and holds the ground answers of that call
//! as closed templates, a set in insertion order, each with the derivation
//! that first produced it when explanations were on. A table's content is
//! a function of the program alone (it is computed over the world store),
//! so the registry lives on the `Program` and survives queries.

use std::collections::{HashMap, HashSet};

use super::answer::ProofNode;
use super::compile::TTerm;

/// Where a table stands in the linear-tabling evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableState {
    /// No evaluation can add to it: consumed as a list of facts.
    Complete,
    /// Being computed by the evaluation frame at this depth of the
    /// machine's evaluation stack; a call to it is a recursive variant call,
    /// which consumes the answers so far.
    Evaluating(usize),
    /// Its evaluation returned while it depended on a frame still being
    /// evaluated (the leader); re-evaluated when called again in the
    /// leader's next pass, completed with the leader.
    Incomplete,
}

#[derive(Debug, Clone)]
pub(crate) struct Table {
    pub answers: Vec<TTerm>,
    /// Per answer: the derivation that first produced it, when recorded.
    pub proofs: Vec<Option<Vec<ProofNode>>>,
    seen: HashSet<TTerm>,
    pub state: TableState,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Tables {
    map: HashMap<TTerm, Table>,
    /// Answers recorded in all tables, ever: a leader compares it across a
    /// pass to know whether to iterate.
    pub total: usize,
}

impl Tables {
    pub fn state(&self, key: &TTerm) -> Option<TableState> {
        self.map.get(key).map(|t| t.state)
    }

    /// Start (or restart) the evaluation of `key` at `depth`, keeping the
    /// answers found so far.
    pub fn begin(&mut self, key: &TTerm, depth: usize) {
        let t = self.map.entry(key.clone()).or_insert_with(|| Table {
            answers: Vec::new(),
            proofs: Vec::new(),
            seen: HashSet::new(),
            state: TableState::Evaluating(depth),
        });
        t.state = TableState::Evaluating(depth);
    }

    pub fn set_state(&mut self, key: &TTerm, state: TableState) {
        if let Some(t) = self.map.get_mut(key) {
            t.state = state;
        }
    }

    /// Record an answer; false if it was already there.
    pub fn add(&mut self, key: &TTerm, answer: TTerm, proof: Option<Vec<ProofNode>>) -> bool {
        let Some(t) = self.map.get_mut(key) else {
            return false;
        };
        if !t.seen.insert(answer.clone()) {
            return false;
        }
        t.answers.push(answer);
        t.proofs.push(proof);
        self.total += 1;
        true
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    pub fn len(&self, key: &TTerm) -> usize {
        self.map.get(key).map_or(0, |t| t.answers.len())
    }

    pub fn answer(&self, key: &TTerm, i: usize) -> Option<(&TTerm, &Option<Vec<ProofNode>>)> {
        let t = self.map.get(key)?;
        Some((t.answers.get(i)?, t.proofs.get(i)?))
    }

    /// A query stopped inside an evaluation: forget every table that is not
    /// complete, so the next query starts clean.
    pub fn abandon(&mut self) {
        self.map.retain(|_, t| t.state == TableState::Complete);
    }
}
