use slint::ComponentHandle;
use slint::SharedString;

use super::{model, sstr};
use crate::catalog::{PINDEFS, PIN_N};
use crate::state::State;
use crate::{AppWindow, DeviceCfg, PinRow};

pub fn push_pins(ui: &AppWindow, s: &State) {
    let dc = ui.global::<DeviceCfg>();
    let boards: Vec<SharedString> = s.boards.iter().map(|b| sstr(&b.name)).collect();
    dc.set_boards(model(boards));
    dc.set_board(s.board);
    let opts: Vec<SharedString> = s.board_pins().iter().map(|x| sstr(&x.label)).collect();
    dc.set_pin_options(model(opts));
    let rows: Vec<PinRow> = (0..PIN_N)
        .map(|i| PinRow {
            key: sstr(PINDEFS[i].0),
            label: sstr(PINDEFS[i].1),
            gpio: s.pin_gpio[i],
            opt_idx: s.board_idx_of_gpio(s.pin_gpio[i]),
        })
        .collect();
    dc.set_pins(model(rows));
    dc.set_race_screen(s.race_screen);
    dc.set_led_rev(s.led_rev);
    dc.set_led_tc(s.led_tc);
    dc.set_led_abs(s.led_abs);
    dc.set_led_rgbw(s.led_rgbw != 0);
}
