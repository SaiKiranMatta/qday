//! The published public keys for the vector seed.

mod common;

use common::{SEED, hash, tree0};
use qday_xmss::{HEIGHT, KeySet, LEAVES, NODES, Tree, keccak256, node_slot};

const TREE0_ROOT: &str = "0xbfefd6eccd30b7f3b763b5289150e2ee0124555cd5ac7f14601005179d1273e9";
const TREE0_PUB_SEED: &str = "0x78e135a918cf039e569ea83fdefdb29ddd1985b7b33a9867da1edd9026adceb8";
const TREE1_ROOT: &str = "0x18479eb13d693cdb0474c6de337a4544b3a93939bc20476ac23364491c04e881";
const TREE1_PUB_SEED: &str = "0x8c587f5c22ec4115ef0688f0cb254e8fe084ab4ce56416257d8494bddd56a754";

#[test]
fn tree_0_pub_seed_matches_the_vector() {
    assert_eq!(KeySet::derive(&SEED, 0).pub_seed(), &hash(TREE0_PUB_SEED));
}

#[test]
fn tree_1_pub_seed_matches_the_vector() {
    assert_eq!(KeySet::derive(&SEED, 1).pub_seed(), &hash(TREE1_PUB_SEED));
}

#[test]
fn tree_0_root_matches_the_vector() {
    assert_eq!(tree0().root(), hash(TREE0_ROOT));
}

#[test]
fn tree_1_root_matches_the_vector() {
    let tree = Tree::build(&KeySet::derive(&SEED, 1), |_| {});
    assert_eq!(tree.root(), hash(TREE1_ROOT));
}

#[test]
fn a_tree_keeps_every_node() {
    assert_eq!(tree0().nodes().len(), NODES);
}

#[test]
fn stored_leaves_match_a_fresh_leaf() {
    let keys = KeySet::derive(&SEED, 0);
    assert_eq!(tree0().nodes()[node_slot(0, 777)], keys.leaf(777));
}

#[test]
fn progress_counts_every_leaf_in_order() {
    let mut seen = Vec::new();
    Tree::build(&KeySet::derive(&SEED, 2), |done| seen.push(done));
    assert_eq!(seen, (1..=LEAVES).collect::<Vec<_>>());
}

#[test]
fn stored_nodes_round_trip() {
    let nodes = tree0().nodes().to_vec();
    let tree = Tree::from_nodes(nodes).expect("a full node list is accepted");
    assert_eq!(tree.root(), hash(TREE0_ROOT));
}

#[test]
fn a_short_node_list_is_refused() {
    let mut nodes = tree0().nodes().to_vec();
    nodes.pop();
    assert!(Tree::from_nodes(nodes).is_none());
}

#[test]
fn auth_path_holds_the_sibling_at_each_height() {
    let index = 5_u32;
    let path = tree0().auth_path(index);
    assert_eq!(path.len(), HEIGHT);
    for (height, node) in (0_u32..).zip(path) {
        let sibling = (index >> height) ^ 1;
        assert_eq!(node, tree0().nodes()[node_slot(height, sibling)]);
    }
}

#[test]
fn different_seeds_give_different_trees() {
    let other = keccak256(&[&SEED]);
    assert_ne!(
        KeySet::derive(&other, 0).pub_seed(),
        KeySet::derive(&SEED, 0).pub_seed()
    );
}
