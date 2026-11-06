//! The Merkle tree over all one-time keys.

use alloc::vec::Vec;

use crate::adrs::Adrs;
use crate::hash::{Hash, keccak256};
use crate::keys::KeySet;

/// Tree height.
pub const HEIGHT: usize = 10;
const HEIGHT_U32: u32 = 10;
/// One-time keys per tree.
pub const LEAVES: u32 = 1 << HEIGHT;
/// Nodes in a whole tree, leaves and root included.
pub const NODES: usize = 2 * (1 << HEIGHT) - 1;

/// Where node `index` at `height` sits in the flat node list: leaves first, root last.
#[must_use]
pub fn node_slot(height: u32, index: u32) -> usize {
    let below = (2 * LEAVES) - ((2 * LEAVES) >> height);
    (below + index) as usize
}

/// Hashes two children into their parent at `height`, index `index`.
pub(crate) fn parent(pub_seed: &Hash, height: u32, index: u32, left: &Hash, right: &Hash) -> Hash {
    keccak256(&[pub_seed, &Adrs::node(height, index), left, right])
}

/// A whole tree, in `node_slot` order.
pub struct Tree {
    nodes: Vec<Hash>,
}

impl Tree {
    /// Builds a tree, calling `progress` with the number of leaves finished so far.
    pub fn build(keys: &KeySet, mut progress: impl FnMut(u32)) -> Self {
        let mut nodes = Vec::with_capacity(NODES);
        for leaf in 0..LEAVES {
            nodes.push(keys.leaf(leaf));
            progress(leaf + 1);
        }
        for height in 1..=HEIGHT_U32 {
            for index in 0..LEAVES >> height {
                let left = &nodes[node_slot(height - 1, 2 * index)];
                let right = &nodes[node_slot(height - 1, 2 * index + 1)];
                let node = parent(keys.pub_seed(), height, index, left, right);
                nodes.push(node);
            }
        }
        Self { nodes }
    }

    /// Wraps nodes read back from storage. Returns `None` unless there are exactly `NODES`.
    #[must_use]
    pub fn from_nodes(nodes: Vec<Hash>) -> Option<Self> {
        (nodes.len() == NODES).then_some(Self { nodes })
    }

    /// All nodes, in `node_slot` order.
    #[must_use]
    pub fn nodes(&self) -> &[Hash] {
        &self.nodes
    }

    /// The root.
    #[must_use]
    pub fn root(&self) -> Hash {
        self.nodes[NODES - 1]
    }

    /// The sibling at each height on the way from leaf `index` to the root.
    #[must_use]
    pub fn auth_path(&self, index: u32) -> [Hash; HEIGHT] {
        let mut path = [[0; 32]; HEIGHT];
        for (height, node) in (0..).zip(&mut path) {
            *node = self.nodes[node_slot(height, (index >> height) ^ 1)];
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::{LEAVES, NODES, node_slot};

    #[test]
    fn slots_run_from_the_leaves_to_the_root() {
        assert_eq!(node_slot(0, 0), 0);
        assert_eq!(node_slot(0, LEAVES - 1), 1023);
        assert_eq!(node_slot(1, 0), 1024);
        assert_eq!(node_slot(2, 1), 1537);
        assert_eq!(node_slot(10, 0), NODES - 1);
    }
}
