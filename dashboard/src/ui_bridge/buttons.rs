use slint::ComponentHandle;
use slint::SharedString;

use super::{col, model, sstr};
use crate::catalog::BUTTON_FIELDS;
use crate::state::State;
use crate::telemetry::idx_of;
use crate::{AppWindow, Buttons, DashButton};

pub fn btn_model(s: &State, page: i32) -> Vec<DashButton> {
    if page < 0 || page as usize >= s.btn_pages.len() {
        return Vec::new();
    }
    s.btn_pages[page as usize]
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let mut fi = idx_of(&BUTTON_FIELDS, &b.field);
            if fi < 0 {
                fi = 0;
            }
            DashButton {
                pos: i as i32,
                label: sstr(&b.label),
                is_toggle: b.toggle,
                is_on: b.on,
                action: sstr(&b.action),
                active_col: col(b.col),
                sync_from_game: b.sync,
                game_field: sstr(&b.field),
                field_available: b.avail,
                field_idx: fi,
            }
        })
        .collect()
}

pub fn push_buttons_model(ui: &AppWindow, s: &State) {
    let bt = ui.global::<Buttons>();
    let pc = s.btn_pages.len() as i32;
    bt.set_page_count(pc);
    bt.set_page_list(model((0..pc).collect::<Vec<i32>>()));
    let mut pg = bt.get_page();
    if pg >= pc {
        pg = pc - 1;
        bt.set_page(pg);
    }
    bt.set_tiles(model(btn_model(s, pg)));
    let fo: Vec<SharedString> = BUTTON_FIELDS.iter().map(|f| sstr(f)).collect();
    bt.set_field_options(model(fo));
}
