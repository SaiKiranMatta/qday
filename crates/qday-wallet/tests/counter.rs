//! The counter must never go back, and must survive losing power anywhere.

mod common;

use common::SimFlash;
use qday_wallet::counter::{CounterError, FlashCounter, OneWayCounter};
use qday_wallet::flash::layout;

fn counter(flash: &SimFlash) -> FlashCounter<SimFlash> {
    FlashCounter::new(flash.clone(), layout::COUNTER)
}

fn advance_to(flash: &SimFlash, value: u32) {
    let mut counter = counter(flash);
    while counter.value().expect("the flash is healthy") < value {
        counter.advance().expect("the flash is healthy");
    }
}

#[test]
fn a_new_counter_is_zero() {
    assert_eq!(counter(&SimFlash::new()).value(), Ok(0));
}

#[test]
fn advance_returns_the_old_value_and_moves_on() {
    let flash = SimFlash::new();
    let mut counter = counter(&flash);
    assert_eq!(counter.advance(), Ok(0));
    assert_eq!(counter.advance(), Ok(1));
    assert_eq!(counter.value(), Ok(2));
}

#[test]
fn the_value_survives_a_restart() {
    let flash = SimFlash::new();
    advance_to(&flash, 7);
    assert_eq!(counter(&flash).value(), Ok(7));
}

#[test]
fn it_counts_past_many_sector_changes() {
    let flash = SimFlash::new();
    let mut live = counter(&flash);
    for expected in 0..3000 {
        assert_eq!(live.advance(), Ok(expected));
    }
    assert_eq!(counter(&flash).value(), Ok(3000));
}

#[test]
fn it_wears_the_two_sectors_evenly_by_erasing_rarely() {
    let flash = SimFlash::new();
    advance_to(&flash, 2048);
    // 512 entries fit in a sector.
    assert!(flash.erases() <= 4, "{} erases", flash.erases());
}

#[test]
fn a_failed_read_is_an_error() {
    let flash = SimFlash::new();
    let mut live = counter(&flash);
    flash.cut_after(0);
    assert_eq!(live.advance(), Err(CounterError));
    assert_eq!(live.value(), Err(CounterError));
}

#[test]
fn a_write_that_silently_fails_is_an_error() {
    let flash = SimFlash::new();
    advance_to(&flash, 3);
    flash.lose_writes();
    assert_eq!(counter(&flash).advance(), Err(CounterError));
    assert_eq!(counter(&flash).value(), Ok(3));
}

/// Cuts the power at every point of an advance from `start`, then checks the value afterwards.
fn power_cut_anywhere(start: u32) {
    let mut cuts = 0;
    for budget in 0.. {
        let flash = SimFlash::new();
        advance_to(&flash, start);
        flash.cut_after(budget);
        let result = counter(&flash).advance();
        let was_cut = flash.was_cut();
        flash.power_on();

        let after = counter(&flash).value().expect("the flash is back");
        if !was_cut {
            assert_eq!(result, Ok(start));
            assert_eq!(after, start + 1);
            break;
        }
        cuts += 1;
        assert!(result.is_err(), "a cut advance must not report success");
        assert!(
            after == start || after == start + 1,
            "{start} became {after}"
        );

        // The next advance must always land higher than anything before it.
        let mut next = counter(&flash);
        assert_eq!(next.advance(), Ok(after));
        assert_eq!(next.value(), Ok(after + 1));
        assert_eq!(counter(&flash).value(), Ok(after + 1));
    }
    assert!(cuts > 0, "the test must really cut the power");
}

#[test]
fn a_power_cut_in_a_plain_advance_never_goes_back() {
    power_cut_anywhere(5);
}

#[test]
fn a_power_cut_at_a_sector_change_never_goes_back() {
    power_cut_anywhere(511);
    power_cut_anywhere(512);
    power_cut_anywhere(513);
}

#[test]
fn a_power_cut_when_the_other_sector_must_be_erased_never_goes_back() {
    power_cut_anywhere(1023);
    power_cut_anywhere(1024);
}

#[test]
fn repeated_cuts_at_the_same_slot_still_move_forward() {
    let flash = SimFlash::new();
    advance_to(&flash, 3);
    for _ in 0..40 {
        flash.cut_after(0);
        assert!(counter(&flash).advance().is_err());
        flash.power_on();
    }
    let before = counter(&flash).value().expect("the flash is back");
    assert!(before >= 3);
    assert_eq!(counter(&flash).advance(), Ok(before));
}

#[test]
fn a_corrupted_entry_is_ignored() {
    let flash = SimFlash::new();
    advance_to(&flash, 4);
    // Flip the newest entry so its two halves disagree.
    flash.poke(layout::COUNTER as usize + 3 * 8, &[0, 0, 0, 0xEE]);
    assert_eq!(counter(&flash).value(), Ok(3));
}
