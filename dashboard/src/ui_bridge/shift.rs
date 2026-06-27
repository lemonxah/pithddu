use super::{col, model, sstr};
use crate::state::State;
use crate::{AppWindow, Led, ShiftCfg};
use slint::ComponentHandle;

pub fn push_led_model(ui: &AppWindow, s: &State) {
    let g = s.sel_gear as usize;
    let v: Vec<Led> = (0..12)
        .map(|i| Led {
            col: col(s.leds[g][i].rgb),
            threshold: s.leds[g][i].threshold,
        })
        .collect();
    ui.global::<ShiftCfg>().set_leds(model(v));
}

pub fn push_shift_scalars(ui: &AppWindow, s: &State) {
    let sc = ui.global::<ShiftCfg>();
    sc.set_redline_rpm(s.redline_rpm);
    sc.set_first_led_pct(s.first_led_pct);
    sc.set_blink_enabled(s.blink_enabled);
    sc.set_blink_hz(s.blink_hz);
    sc.set_animation(s.animation);
    sc.set_brightness(s.brightness);
    sc.set_rpm_source(s.rpm_source);
    sc.set_sel_gear(s.sel_gear);
    sc.set_car_name(sstr(&s.car_name));
    sc.set_car_game(sstr(&s.car_game));
}

pub fn pull_shift_scalars(ui: &AppWindow, s: &mut State) {
    let sc = ui.global::<ShiftCfg>();
    s.redline_rpm = sc.get_redline_rpm();
    s.first_led_pct = sc.get_first_led_pct();
    s.blink_enabled = sc.get_blink_enabled();
    s.blink_hz = sc.get_blink_hz();
    s.animation = sc.get_animation();
    s.brightness = sc.get_brightness();
    s.rpm_source = sc.get_rpm_source();
}
