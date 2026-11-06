//! A wrong code must always be counted, whatever the power does.

mod common;

use common::{SimFlash, TestRng};
use qday_wallet::flash::layout;
use qday_wallet::keystore::StoreError;
use qday_wallet::passcode::{Attempt, Code, MAX_FAILURES, Passcode, shuffled_digits, wait_after};
use qday_xmss::keccak256;

const RIGHT: Code = [1, 2, 3, 4, 5, 6];
const WRONG: Code = [6, 5, 4, 3, 2, 1];
const MINUTE: u64 = 60_000;
/// Far enough ahead that no wait is still running.
const LATER: u64 = 1 << 40;

fn store(flash: &SimFlash) -> Passcode<SimFlash> {
    Passcode::new(flash.clone(), layout::PASSCODE)
}

fn with_code() -> SimFlash {
    let flash = SimFlash::new();
    store(&flash)
        .create(&RIGHT, &mut TestRng(1))
        .expect("the flash is healthy");
    flash
}

fn fail_times(flash: &SimFlash, count: u32) {
    let mut passcode = store(flash);
    for i in 0..count {
        let result = passcode.attempt(&WRONG, LATER * (u64::from(i) + 1));
        assert!(matches!(result, Ok(Attempt::Wrong { .. })), "{result:?}");
    }
}

fn is_correct(result: &Result<Attempt, StoreError>) -> bool {
    matches!(result, Ok(Attempt::Correct(_)))
}

#[test]
fn waits_follow_the_table() {
    let waits: Vec<u64> = (0..=9).map(wait_after).collect();
    let want = [
        0,
        0,
        0,
        MINUTE / 2,
        MINUTE,
        2 * MINUTE,
        5 * MINUTE,
        15 * MINUTE,
        30 * MINUTE,
        60 * MINUTE,
    ];
    assert_eq!(waits, want);
}

#[test]
fn no_passcode_exists_at_first() {
    assert_eq!(store(&SimFlash::new()).exists(), Ok(false));
}

#[test]
fn a_created_passcode_exists_after_a_restart() {
    assert_eq!(store(&with_code()).exists(), Ok(true));
}

#[test]
fn a_second_create_is_refused() {
    let flash = with_code();
    assert_eq!(
        store(&flash).create(&WRONG, &mut TestRng(2)),
        Err(StoreError)
    );
    assert!(is_correct(&store(&flash).attempt(&RIGHT, 0)));
}

#[test]
fn the_right_code_is_correct() {
    assert!(is_correct(&store(&with_code()).attempt(&RIGHT, 0)));
}

#[test]
fn the_code_is_not_stored_in_the_clear() {
    let flash = with_code();
    assert!(!flash.contains(&RIGHT));
    assert!(!flash.contains(b"123456"));
}

#[test]
fn the_stored_hash_is_salted_keccak() {
    let flash = with_code();
    let mut salt = [0; 16];
    let mut rng = TestRng(1);
    qday_wallet::rng::Rng::fill(&mut rng, &mut salt);
    let hash = keccak256(&[b"QDAY-PIN-V1", &salt, b"123456"]);
    assert!(flash.contains(&salt));
    assert!(flash.contains(&hash));
}

#[test]
fn the_same_code_with_another_salt_stores_another_hash() {
    let (first, second) = (SimFlash::new(), SimFlash::new());
    store(&first)
        .create(&RIGHT, &mut TestRng(1))
        .expect("the flash is healthy");
    store(&second)
        .create(&RIGHT, &mut TestRng(2))
        .expect("the flash is healthy");
    assert_ne!(first.image(), second.image());
}

#[test]
fn a_wrong_code_reports_the_count() {
    let flash = with_code();
    let got = store(&flash).attempt(&WRONG, 0);
    assert_eq!(
        got,
        Ok(Attempt::Wrong {
            failures: 1,
            tries_left: 9,
            wait_ms: 0
        })
    );
}

#[test]
fn the_third_failure_starts_a_wait() {
    let flash = with_code();
    fail_times(&flash, 2);
    let got = store(&flash).attempt(&WRONG, 1000);
    assert_eq!(
        got,
        Ok(Attempt::Wrong {
            failures: 3,
            tries_left: 7,
            wait_ms: MINUTE / 2
        })
    );
}

#[test]
fn a_locked_entry_does_not_check_or_count() {
    let flash = with_code();
    fail_times(&flash, 2);
    let mut passcode = store(&flash);
    passcode
        .attempt(&WRONG, 1000)
        .expect("the flash is healthy");

    let got = passcode.attempt(&RIGHT, 1000 + MINUTE / 2 - 1);
    assert_eq!(got, Ok(Attempt::Locked { remaining_ms: 1 }));
    assert_eq!(passcode.failures(), Ok(3));
    assert!(is_correct(&passcode.attempt(&RIGHT, 1000 + MINUTE / 2)));
}

#[test]
fn failures_survive_a_restart() {
    let flash = with_code();
    fail_times(&flash, 4);
    assert_eq!(store(&flash).failures(), Ok(4));
}

#[test]
fn a_restart_starts_the_wait_again() {
    let flash = with_code();
    fail_times(&flash, 4);
    let mut passcode = store(&flash);
    passcode.on_boot(500).expect("the flash is healthy");
    assert_eq!(passcode.locked_for(500), MINUTE);
    assert_eq!(passcode.locked_for(500 + MINUTE), 0);
}

#[test]
fn the_right_code_clears_the_failures() {
    let flash = with_code();
    fail_times(&flash, 2);
    assert!(is_correct(&store(&flash).attempt(&RIGHT, 0)));
    assert_eq!(store(&flash).failures(), Ok(0));
}

#[test]
fn the_tenth_failure_is_exhausted() {
    let flash = with_code();
    fail_times(&flash, MAX_FAILURES - 1);
    assert_eq!(
        store(&flash).attempt(&WRONG, LATER * 100),
        Ok(Attempt::Exhausted)
    );
    assert_eq!(store(&flash).failures(), Ok(10));
}

#[test]
fn the_right_code_on_the_tenth_try_still_works() {
    let flash = with_code();
    fail_times(&flash, MAX_FAILURES - 1);
    assert!(is_correct(&store(&flash).attempt(&RIGHT, LATER * 100)));
}

#[test]
fn changing_the_code_needs_proof_and_replaces_it() {
    let flash = with_code();
    let mut passcode = store(&flash);
    let Ok(Attempt::Correct(proof)) = passcode.attempt(&RIGHT, 0) else {
        panic!("the code is right");
    };
    passcode
        .change(proof, &WRONG, &mut TestRng(9))
        .expect("the flash is healthy");
    assert!(is_correct(&store(&flash).attempt(&WRONG, 0)));
    assert!(matches!(
        store(&flash).attempt(&RIGHT, 0),
        Ok(Attempt::Wrong { .. })
    ));
}

#[test]
fn wiping_removes_everything() {
    let flash = with_code();
    fail_times(&flash, 2);
    store(&flash).wipe().expect("the flash is healthy");
    assert_eq!(store(&flash).exists(), Ok(false));
    assert_eq!(store(&flash).failures(), Ok(0));
}

#[test]
fn thousands_of_attempts_keep_the_count_across_log_rollovers() {
    let flash = with_code();
    let mut passcode = store(&flash);
    for round in 0..3000_u64 {
        assert!(
            is_correct(&passcode.attempt(&RIGHT, round)),
            "round {round}"
        );
    }
    fail_times(&flash, 2);
    assert_eq!(store(&flash).failures(), Ok(2));
    assert!(is_correct(&store(&flash).attempt(&RIGHT, LATER)));
}

#[test]
fn failures_carry_across_a_log_rollover() {
    let flash = with_code();
    let mut passcode = store(&flash);
    for round in 0..1983_u64 {
        assert!(
            is_correct(&passcode.attempt(&RIGHT, round)),
            "round {round}"
        );
    }
    fail_times(&flash, 5);
    assert_eq!(store(&flash).failures(), Ok(5));
}

/// Cuts the power at every point of one attempt, starting from `failures_before` failures.
fn cut_anywhere(code: Code, failures_before: u32, attempts_before: u32) {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = with_code();
        for round in 0..attempts_before {
            let result = store(&flash).attempt(&RIGHT, u64::from(round));
            assert!(is_correct(&result));
        }
        fail_times(&flash, failures_before);
        flash.cut_after(budget);
        let result = store(&flash).attempt(&code, LATER * 50);
        let was_cut = flash.was_cut();
        flash.power_on();

        let after = store(&flash).failures().expect("the flash is back");
        if !was_cut {
            break;
        }
        cuts += 1;
        if code == WRONG {
            assert!(
                !matches!(result, Ok(Attempt::Wrong { .. })) || after > failures_before,
                "a wrong answer was given but not counted"
            );
        }
        // A cut may lose an attempt that never reached the comparison, but must never
        // reveal a right code without the count being safe, and never lower the count.
        assert!(
            after >= failures_before,
            "{failures_before} fell to {after}"
        );
        if is_correct(&result) {
            assert_eq!(after, 0);
        }
        assert_eq!(store(&flash).exists(), Ok(true));
        assert!(
            is_correct(&store(&flash).attempt(&RIGHT, LATER * 60)),
            "the code was lost"
        );
    }
    assert!(cuts > 0);
}

#[test]
fn a_cut_during_a_wrong_attempt_cannot_dodge_the_count() {
    cut_anywhere(WRONG, 0, 0);
    cut_anywhere(WRONG, 2, 0);
}

#[test]
fn a_cut_after_the_fail_mark_counts_even_for_the_right_code() {
    let flash = with_code();
    // Budget 1 lets the FAIL mark land and cuts the OK mark.
    flash.cut_after(1);
    let result = store(&flash).attempt(&RIGHT, 0);
    flash.power_on();
    assert!(result.is_err());
    assert_eq!(store(&flash).failures(), Ok(1));
}

#[test]
fn a_cut_at_a_log_rollover_cannot_dodge_the_count() {
    // 1,984 marks fit in a sector, so the attempt under test has to start a new one.
    cut_anywhere(WRONG, 3, 1981);
}

#[test]
fn a_cut_while_creating_leaves_no_half_code() {
    for budget in 0..4 {
        let flash = SimFlash::new();
        flash.cut_after(budget);
        let result = store(&flash).create(&RIGHT, &mut TestRng(1));
        let was_cut = flash.was_cut();
        flash.power_on();
        if !was_cut {
            assert_eq!(result, Ok(()));
            break;
        }
        assert_eq!(store(&flash).exists(), Ok(false));
    }
}

#[test]
fn a_cut_while_changing_keeps_one_working_code() {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = with_code();
        let mut passcode = store(&flash);
        let Ok(Attempt::Correct(proof)) = passcode.attempt(&RIGHT, 0) else {
            panic!("the code is right");
        };
        flash.cut_after(budget);
        let result = passcode.change(proof, &WRONG, &mut TestRng(9));
        let was_cut = flash.was_cut();
        flash.power_on();
        if !was_cut {
            assert_eq!(result, Ok(()));
            break;
        }
        cuts += 1;
        let old = is_correct(&store(&flash).attempt(&RIGHT, 0));
        let new = is_correct(&store(&flash).attempt(&WRONG, 0));
        assert!(old || new, "both codes were lost");
    }
    assert!(cuts > 0);
}

#[test]
fn a_cut_while_wiping_a_full_count_stays_wiped_or_exhausted() {
    for budget in 0..3 {
        let flash = with_code();
        fail_times(&flash, MAX_FAILURES - 1);
        assert_eq!(
            store(&flash).attempt(&WRONG, LATER * 100),
            Ok(Attempt::Exhausted)
        );
        flash.cut_after(budget);
        let _ = store(&flash).wipe();
        flash.power_on();
        let exists = store(&flash).exists().expect("the flash is back");
        let failures = store(&flash).failures().expect("the flash is back");
        assert!(
            !exists || failures >= MAX_FAILURES,
            "exists {exists}, failures {failures}"
        );
    }
}

#[test]
fn the_keypad_holds_every_digit_once() {
    let mut digits = shuffled_digits(&mut TestRng(3));
    digits.sort_unstable();
    assert_eq!(digits, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn the_keypad_changes_each_time() {
    let mut rng = TestRng(3);
    let first = shuffled_digits(&mut rng);
    let second = shuffled_digits(&mut rng);
    assert_ne!(first, second);
}

#[test]
fn every_digit_reaches_every_position() {
    let mut rng = TestRng(5);
    let mut seen = [[false; 10]; 10];
    for _ in 0..5000 {
        for (position, digit) in shuffled_digits(&mut rng).into_iter().enumerate() {
            seen[position][usize::from(digit)] = true;
        }
    }
    assert!(seen.iter().flatten().all(|&hit| hit));
}
