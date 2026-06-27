pub mod buttons;
pub mod cars;
pub mod device;
pub mod firmware;
pub mod race;
pub mod shift;
pub mod simhub;
pub mod telemetry;

use slint::{Color, ModelRc, SharedString, VecModel};

pub fn col(rgb: u32) -> Color {
    Color::from_rgb_u8(
        ((rgb >> 16) & 0xFF) as u8,
        ((rgb >> 8) & 0xFF) as u8,
        (rgb & 0xFF) as u8,
    )
}

pub fn to_u32(c: Color) -> u32 {
    ((c.red() as u32) << 16) | ((c.green() as u32) << 8) | (c.blue() as u32)
}

pub fn pal(tok: &str) -> Color {
    col(crate::telemetry::palette_color(tok))
}

pub fn sstr(s: &str) -> SharedString {
    s.into()
}

pub fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

pub fn refresh_race(ui: &crate::AppWindow, s: &crate::state::State) {
    race::push_zones(ui, s);
    race::push_presets(ui, s);
    race::push_resolved(ui, s);
    race::push_edit_module(ui, s);
}
