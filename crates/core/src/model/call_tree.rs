use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Aggregated call tree node used by Left Heavy and Sandwich views.
/// Multiple frames with the same call path are merged into one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTreeNode {
    pub id: u64,
    pub name: String,
    /// Total time across all merged instances.
    pub total_time: f64,
    /// Self time (exclusive of children).
    pub self_time: f64,
    pub children: Vec<u64>,
}

/// An aggregated call tree built from a `Profile`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallTree {
    pub nodes: HashMap<u64, CallTreeNode>,
    pub roots: Vec<u64>,
}

impl CallTree {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            roots: Vec::new(),
        }
    }

    pub fn node(&self, id: u64) -> Option<&CallTreeNode> {
        self.nodes.get(&id)
    }
}

impl Default for CallTree {
    fn default() -> Self {
        Self::new()
    }
}
