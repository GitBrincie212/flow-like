use std::collections::{HashMap, HashSet};

use crate::{
    flow::{
        board::{
            Board,
            cleanup::{BoardCleanupLogic, PinLookup},
        },
        node::Node,
        pin::Pin,
    },
    utils::hash::hash_string_non_cryptographic,
};

#[derive(Default)]
pub struct FixRefsCleanup {
    pub refs: HashMap<String, String>,
    pub abandoned: HashSet<String>,
}

impl FixRefsCleanup {
    fn ensure_ref(&mut self, s: &mut String) {
        if self.refs.contains_key(s) {
            self.abandoned.remove(s);
            return;
        }
        let hash = hash_string_non_cryptographic(s).to_string();
        self.refs.insert(hash.clone(), std::mem::take(s));
        self.abandoned.remove(&hash);
        *s = hash;
    }

    fn ensure_ref_opt(&mut self, s: &mut Option<String>) {
        if let Some(inner) = s {
            self.ensure_ref(inner);
        }
    }
}

impl BoardCleanupLogic for FixRefsCleanup {
    fn init(board: &mut Board) -> Self
    where
        Self: Sized,
    {
        Self {
            refs: board.refs.clone(),
            abandoned: board.refs.keys().cloned().collect(),
        }
    }

    fn main_node_iteration(&mut self, node: &mut Node, _pin_lookup: &PinLookup) {
        self.ensure_ref(&mut node.description);
    }

    fn main_pin_iteration(&mut self, pin: &mut Pin, _pin_lookup: &PinLookup) {
        self.ensure_ref(&mut pin.description);
        self.ensure_ref_opt(&mut pin.schema);
    }

    fn post_process(&mut self, board: &mut Board, _pin_lookup: &PinLookup) {
        board.refs = std::mem::take(&mut self.refs);
        board.refs.retain(|k, _| !self.abandoned.contains(k));
        board.refs.shrink_to_fit();
    }
}
