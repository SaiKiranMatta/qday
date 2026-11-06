//! Key slots must always leave one working key, whatever the power does.

mod common;

use std::sync::OnceLock;

use common::SimFlash;
use qday_wallet::flash::layout;
use qday_wallet::keystore::{KeyStore, Slots};
use qday_xmss::{Hash, KeySet, PublicKey, Tree};

const SEED: Hash = [0x42; 32];

fn tree(number: u32) -> &'static Tree {
    static TREES: [OnceLock<Tree>; 2] = [OnceLock::new(), OnceLock::new()];
    TREES[number as usize].get_or_init(|| Tree::build(&KeySet::derive(&SEED, number), |_| {}))
}

fn public_key(number: u32) -> PublicKey {
    PublicKey {
        root: tree(number).root(),
        pub_seed: *KeySet::derive(&SEED, number).pub_seed(),
    }
}

fn store(flash: &SimFlash) -> KeyStore<SimFlash> {
    KeyStore::new(flash.clone())
}

fn scan(flash: &SimFlash) -> Slots {
    store(flash).scan().expect("the flash is healthy")
}

/// A flash with tree 0 active in slot 0.
fn with_active_tree() -> SimFlash {
    let flash = SimFlash::new();
    let mut keys = store(&flash);
    keys.write_tree(0, &SEED, 0, 0, tree(0))
        .expect("the flash is healthy");
    keys.activate(0, 1, 0).expect("the flash is healthy");
    flash
}

/// A flash with tree 0 active and tree 1 prepared.
fn with_prepared_tree() -> SimFlash {
    let flash = with_active_tree();
    store(&flash)
        .write_tree(1, &SEED, 1, 0, tree(1))
        .expect("the flash is healthy");
    flash
}

#[test]
fn empty_flash_has_no_keys() {
    assert_eq!(scan(&SimFlash::new()), Slots::default());
}

#[test]
fn a_written_tree_is_not_active_until_activated() {
    let flash = SimFlash::new();
    store(&flash)
        .write_tree(0, &SEED, 0, 0, tree(0))
        .expect("the flash is healthy");
    let slots = scan(&flash);
    assert_eq!(slots.active, None);
    assert_eq!(slots.prepared.map(|key| key.tree), Some(0));
}

#[test]
fn an_activated_tree_reports_its_fields() {
    let flash = SimFlash::new();
    let mut keys = store(&flash);
    keys.write_tree(1, &SEED, 3, 70, tree(0))
        .expect("the flash is healthy");
    keys.activate(1, 9, 123).expect("the flash is healthy");

    let active = scan(&flash).active.expect("a key is active");
    assert_eq!(active.slot, 1);
    assert_eq!(active.tree, 3);
    assert_eq!(active.first_index, 70);
    assert_eq!(active.counter_base, 123);
    assert_eq!(active.serial, 9);
    assert_eq!(active.key.root, tree(0).root());
}

#[test]
fn the_public_seed_comes_from_the_seed_and_tree_number() {
    let active = scan(&with_active_tree()).active.expect("a key is active");
    assert_eq!(active.key, public_key(0));
}

#[test]
fn the_seed_reads_back() {
    assert_eq!(store(&with_active_tree()).seed(0), Ok(SEED));
}

#[test]
fn auth_paths_match_the_tree() {
    let flash = with_active_tree();
    for index in [0, 5, 512, 1023] {
        assert_eq!(
            store(&flash).auth_path(0, index),
            Ok(tree(0).auth_path(index))
        );
    }
}

#[test]
fn the_free_slot_is_never_the_active_one() {
    let flash = with_active_tree();
    assert_eq!(scan(&flash).free_slot(), 1);
    store(&flash)
        .write_tree(1, &SEED, 1, 0, tree(1))
        .expect("the flash is healthy");
    store(&flash)
        .activate(1, 2, 5)
        .expect("the flash is healthy");
    assert_eq!(scan(&flash).free_slot(), 0);
}

#[test]
fn activating_switches_keys_and_retires_the_old_slot() {
    let flash = with_prepared_tree();
    let prepared = scan(&flash).prepared.expect("a tree is prepared");
    assert_eq!(prepared.slot, 1);
    assert_eq!(prepared.key, public_key(1));

    store(&flash)
        .activate(1, 2, 40)
        .expect("the flash is healthy");
    let slots = scan(&flash);
    let active = slots.active.expect("a key is active");
    assert_eq!((active.slot, active.tree, active.counter_base), (1, 1, 40));
    assert_eq!(slots.prepared, None, "the old tree must not look prepared");
}

#[test]
fn damage_to_a_header_empties_the_slot() {
    let flash = with_active_tree();
    flash.poke(layout::KEY_SLOTS as usize + 20, &[0x00]);
    assert_eq!(scan(&flash).active, None);
}

#[test]
fn damage_to_a_tree_node_empties_the_slot() {
    let flash = with_active_tree();
    let node = layout::KEY_SLOTS as usize + 0x1000 + 777 * 32;
    let flipped = !flash.peek(node, 1)[0];
    flash.poke(node, &[flipped & flash.peek(node, 1)[0]]);
    assert_eq!(scan(&flash).active, None);
}

#[test]
fn damage_to_an_activation_record_is_ignored() {
    let flash = with_active_tree();
    flash.poke(layout::KEY_SLOTS as usize + 256, &[0x00; 4]);
    assert_eq!(scan(&flash).active, None);
}

#[test]
fn an_activation_record_copied_to_another_slot_is_ignored() {
    let flash = with_prepared_tree();
    let record_at = layout::KEY_SLOTS as usize + 256;
    let record = flash.peek(record_at, 32);
    flash.poke(record_at + layout::KEY_SLOT_SIZE as usize, &record);
    let slots = scan(&flash);
    assert_eq!(slots.active.map(|active| active.tree), Some(0));
    assert_eq!(slots.prepared.map(|prepared| prepared.tree), Some(1));
}

#[test]
fn wiping_removes_the_seed_from_flash() {
    let flash = with_prepared_tree();
    assert!(flash.contains(&SEED));
    store(&flash).wipe().expect("the flash is healthy");
    assert_eq!(scan(&flash), Slots::default());
    assert!(!flash.contains(&SEED));
}

#[test]
fn a_cut_during_the_first_tree_leaves_nothing_or_a_whole_tree() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = SimFlash::new();
        flash.cut_after(budget);
        let result = store(&flash).write_tree(0, &SEED, 0, 0, tree(0));
        let was_cut = flash.was_cut();
        flash.power_on();
        if !was_cut {
            assert_eq!(result, Ok(()));
            break;
        }
        cuts += 1;
        assert!(result.is_err());
        if let Some(prepared) = scan(&flash).prepared {
            assert_eq!(prepared.key, public_key(0));
        }
    }
    assert!(cuts > 16, "only {cuts} cuts");
}

#[test]
fn a_cut_while_preparing_leaves_the_active_key_working() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = with_active_tree();
        flash.cut_after(budget);
        let result = store(&flash).write_tree(1, &SEED, 1, 0, tree(1));
        let was_cut = flash.was_cut();
        flash.power_on();

        let slots = scan(&flash);
        let active = slots.active.expect("the active key must survive");
        assert_eq!(active.key, public_key(0));
        assert_eq!(store(&flash).auth_path(0, 9), Ok(tree(0).auth_path(9)));
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert!(slots.prepared.is_some());
            break;
        }
        cuts += 1;
    }
    assert!(cuts > 16, "only {cuts} cuts");
}

#[test]
fn a_cut_while_activating_leaves_exactly_one_working_key() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = with_prepared_tree();
        flash.cut_after(budget);
        let result = store(&flash).activate(1, 2, 8);
        let was_cut = flash.was_cut();
        flash.power_on();

        let active = scan(&flash).active.expect("a key must stay active");
        if !was_cut {
            assert_eq!(result, Ok(()));
            assert_eq!(active.tree, 1);
            break;
        }
        cuts += 1;
        assert!(active.tree == 0 || active.tree == 1);

        // Retrying must finish the job, even over a torn record.
        if active.tree == 0 {
            store(&flash).activate(1, 2, 8).expect("the flash is back");
            let active = scan(&flash).active.expect("a key is active");
            assert_eq!((active.tree, active.counter_base), (1, 8));
        }
    }
    assert!(cuts > 0);
}

#[test]
fn a_cut_while_wiping_never_leaves_a_half_key_active() {
    for budget in 0..40 {
        let flash = with_active_tree();
        flash.cut_after(budget);
        let _ = store(&flash).wipe();
        flash.power_on();
        if let Some(active) = scan(&flash).active {
            assert_eq!(active.key, public_key(0));
        }
    }
}
