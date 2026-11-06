//! Calibration maths and saved settings.

mod common;

use common::SimFlash;
use qday_wallet::flash::layout;
use qday_wallet::settings::{CROSSES, Calibration, Settings, ThemeChoice};

/// Readings from the real panel, whose axes are swapped relative to the screen.
const RAW: [(i32, i32); 3] = [(523, 567), (523, 3673), (3523, 567)];

fn settings(flash: &SimFlash) -> Settings<SimFlash> {
    Settings::new(flash.clone())
}

#[test]
fn each_cross_maps_to_itself() {
    let calibration = Calibration::from_points(RAW).expect("the points form a triangle");
    for (raw, cross) in RAW.iter().zip(CROSSES) {
        assert_eq!(calibration.to_screen(raw.0, raw.1), cross);
    }
}

#[test]
fn the_fourth_corner_lands_where_it_should() {
    let calibration = Calibration::from_points(RAW).expect("the points form a triangle");
    assert_eq!(calibration.to_screen(3523, 3673), (300, 220));
}

#[test]
fn a_panel_with_unswapped_axes_also_works() {
    let raw = [(500, 600), (3500, 600), (500, 3400)];
    let calibration = Calibration::from_points(raw).expect("the points form a triangle");
    assert_eq!(calibration.to_screen(3500, 3400), (300, 220));
}

#[test]
fn readings_past_the_edge_stay_on_screen() {
    let calibration = Calibration::factory();
    assert_eq!(calibration.to_screen(0, 0), (0, 0));
    assert_eq!(calibration.to_screen(4095, 4095), (319, 239));
}

#[test]
fn the_factory_values_match_the_tested_panel() {
    assert_eq!(Calibration::factory().to_screen(523, 3673), (300, 20));
    assert_eq!(Calibration::factory().to_screen(3523, 567), (20, 220));
}

#[test]
fn readings_on_a_line_are_refused() {
    assert_eq!(Calibration::from_points([(1, 1), (2, 2), (3, 3)]), None);
    assert_eq!(Calibration::from_points([(9, 9), (9, 9), (9, 9)]), None);
}

#[test]
fn the_theme_defaults_to_light() {
    assert_eq!(settings(&SimFlash::new()).theme(), Ok(ThemeChoice::Light));
}

#[test]
fn the_theme_survives_a_restart() {
    let flash = SimFlash::new();
    settings(&flash)
        .set_theme(ThemeChoice::Dark)
        .expect("the flash is healthy");
    assert_eq!(settings(&flash).theme(), Ok(ThemeChoice::Dark));
    settings(&flash)
        .set_theme(ThemeChoice::Light)
        .expect("the flash is healthy");
    assert_eq!(settings(&flash).theme(), Ok(ThemeChoice::Light));
}

#[test]
fn a_damaged_theme_reads_as_the_default() {
    let flash = SimFlash::new();
    settings(&flash)
        .set_theme(ThemeChoice::Dark)
        .expect("the flash is healthy");
    flash.poke(layout::SETTINGS as usize + 9, &[0x00]);
    assert_eq!(settings(&flash).theme(), Ok(ThemeChoice::Light));
}

#[test]
fn a_cut_while_saving_the_theme_leaves_a_valid_theme() {
    for budget in 0..3 {
        let flash = SimFlash::new();
        settings(&flash)
            .set_theme(ThemeChoice::Dark)
            .expect("the flash is healthy");
        flash.cut_after(budget);
        let _ = settings(&flash).set_theme(ThemeChoice::Light);
        flash.power_on();
        assert!(settings(&flash).theme().is_ok());
    }
}

#[test]
fn no_calibration_at_first() {
    assert_eq!(settings(&SimFlash::new()).calibration(), Ok(None));
}

#[test]
fn a_calibration_survives_a_restart() {
    let flash = SimFlash::new();
    let calibration = Calibration::from_points(RAW).expect("the points form a triangle");
    settings(&flash)
        .set_calibration(&calibration)
        .expect("the flash is healthy");
    assert_eq!(settings(&flash).calibration(), Ok(Some(calibration)));
}

#[test]
fn a_damaged_calibration_reads_as_missing() {
    let flash = SimFlash::new();
    let calibration = Calibration::from_points(RAW).expect("the points form a triangle");
    settings(&flash)
        .set_calibration(&calibration)
        .expect("the flash is healthy");
    flash.poke(layout::CALIBRATION as usize + 12, &[0x55]);
    assert_eq!(settings(&flash).calibration(), Ok(None));
}

#[test]
fn settings_do_not_touch_each_other() {
    let flash = SimFlash::new();
    let calibration = Calibration::factory();
    settings(&flash)
        .set_calibration(&calibration)
        .expect("the flash is healthy");
    settings(&flash)
        .set_theme(ThemeChoice::Dark)
        .expect("the flash is healthy");
    assert_eq!(settings(&flash).calibration(), Ok(Some(calibration)));
}
