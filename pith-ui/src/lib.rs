//! pith-ui — the shared, runtime-interpreted UI engine for the Pith DDU.
//!
//! A screen is a [`UiDoc`]: a serializable, **freeform** tree of [`Node`]s (each an
//! absolute [`Rect`] + a [`Kind`]). The doc is serialized with `postcard` and
//! **interpreted + rendered at runtime** against any `embedded_graphics::DrawTarget`
//! — so screens change by loading a new blob from flash or the wire, no recompile,
//! with full layout control.
//!
//! The renderer uses the *same* `u8g2-fonts`, palette and `pith-core` formatting /
//! shift-light logic the firmware uses, so the desktop preview is **pixel-identical**
//! to the device. [`render_screen_diff`] does **dirty-rect** redraws — only the nodes
//! whose telemetry changed repaint, so the device only pushes changed pixels over SPI.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, PrimitiveStyle, Rectangle, RoundedRectangle},
};
use serde::{Deserialize, Serialize};
use u8g2_fonts::{
    fonts,
    types::{FontColor, HorizontalAlignment, VerticalPosition},
    FontRenderer,
};

use pith_core::format::{self, Fmt, RuleOp};
pub use pith_core::format::{Fmt as ValueFmt, Pal, RuleOp as Op};
use pith_core::registry::{field_def, field_value};
use pith_core::shift::{segment_rgb, CarData, RevCfg};
use pith_core::simhub::Telemetry;

// ---- palette (Pal token -> RGB565), identical to the firmware ----
pub fn pal(p: Pal) -> Rgb565 {
    match p {
        Pal::Bg => rgb(8, 10, 14),
        Pal::Panel => rgb(28, 32, 40),
        Pal::White => rgb(235, 238, 245),
        Pal::Dim => rgb(120, 128, 140),
        Pal::Green => rgb(40, 220, 90),
        Pal::Amber => rgb(255, 180, 40),
        Pal::Red => rgb(240, 60, 60),
        Pal::Cyan => rgb(40, 210, 230),
        Pal::Blue => rgb(60, 130, 255),
        Pal::Purple => rgb(180, 110, 255),
    }
}
fn rgb(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}
fn rgb888(c: u32) -> Rgb565 {
    rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

// ============ model ============

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn h(self) -> HorizontalAlignment {
        match self {
            Align::Left => HorizontalAlignment::Left,
            Align::Center => HorizontalAlignment::Center,
            Align::Right => HorizontalAlignment::Right,
        }
    }
}

/// A colour rule: when `op(value, threshold)` holds, use `color` (first match wins).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub op: RuleOp,
    pub threshold: i32,
    pub color: Pal,
}

/// What a node draws. `field` is a 1-based telemetry field id (0 = none). Composite
/// kinds read a fixed set of fields; `Stat`/`Bar`/`Flag` are data-bound.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Kind {
    /// Filled (optionally rounded) background panel.
    Panel { color: Pal, radius: u8 },
    /// Static text.
    Label { text: String, color: Pal, size: u8, align: Align },
    /// Caption + a live value formatted via the field registry (overridable).
    Stat { field: u8, label: String, fmt: Option<Fmt>, scale: i32, unit: String, base: Pal, rules: Vec<Rule>, size: u8 },
    /// Horizontal level bar; value/scale -> 0..=100%.
    Bar { field: u8, label: String, scale: i32, base: Pal, rules: Vec<Rule> },
    /// Big gear glyph, optionally with speed below.
    GearSpeed { speed: bool },
    /// 12-segment rev/shift strip (uses the shared shift-light colours).
    RpmStrip,
    /// 2x2 tyre-temperature grid.
    TyreGrid,
    /// TC / ABS levels side by side.
    TcDual,
    /// S1/S2/S3 sector times (green if <= personal best).
    Sectors,
    /// Current + best lap times.
    LapPair,
    /// Race position P{pos}/{field}.
    Position { label: String },
    /// Solid flag-colour panel (driven by `field` + rules).
    Flag { field: u8, base: Pal, rules: Vec<Rule> },
    /// Track-map placeholder.
    Map,
}

/// A positioned node: an absolute rectangle + what to draw in it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Node {
    pub rect: Rect,
    pub kind: Kind,
}

/// One screen, targeting one display.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Screen {
    pub display: u8,
    pub w: u32,
    pub h: u32,
    pub bg: Pal,
    pub nodes: Vec<Node>,
}

/// A complete UI: one or more screens (one per display).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UiDoc {
    pub version: u16,
    pub screens: Vec<Screen>,
}

impl UiDoc {
    pub fn to_postcard(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("serialize UiDoc")
    }
    pub fn from_postcard(bytes: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes(bytes)
    }
}

// ============ render primitives (ported from firmware ui.rs) ============

#[allow(clippy::too_many_arguments)]
fn text<D: DrawTarget<Color = Rgb565>>(
    d: &mut D,
    s: &str,
    x: i32,
    y: i32,
    size: u32,
    color: Rgb565,
    h: HorizontalAlignment,
    v: VerticalPosition,
) {
    let p = Point::new(x, y);
    let fc = FontColor::Transparent(color);
    macro_rules! draw {
        ($f:ty) => {{
            let _ = FontRenderer::new::<$f>().render_aligned(s, p, v, h, fc, d);
        }};
    }
    match size {
        0..=11 => draw!(fonts::u8g2_font_6x13_tf),
        12..=15 => draw!(fonts::u8g2_font_helvB12_tf),
        16..=23 => draw!(fonts::u8g2_font_helvB18_tf),
        24..=33 => draw!(fonts::u8g2_font_helvB24_tf),
        _ => draw!(fonts::u8g2_font_logisoso32_tf),
    }
}

fn fill_rect<D: DrawTarget<Color = Rgb565>>(d: &mut D, x: i32, y: i32, w: i32, h: i32, c: Rgb565) {
    let _ = Rectangle::new(Point::new(x, y), Size::new(w.max(0) as u32, h.max(0) as u32))
        .into_styled(PrimitiveStyle::with_fill(c))
        .draw(d);
}
fn fill_round<D: DrawTarget<Color = Rgb565>>(d: &mut D, x: i32, y: i32, w: i32, h: i32, r: i32, c: Rgb565) {
    let _ = RoundedRectangle::with_equal_corners(
        Rectangle::new(Point::new(x, y), Size::new(w.max(0) as u32, h.max(0) as u32)),
        Size::new(r as u32, r as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(c))
    .draw(d);
}

fn rule_color(raw: i32, base: Pal, rules: &[Rule]) -> Rgb565 {
    for r in rules {
        if r.op.matches(raw, r.threshold) {
            return pal(r.color);
        }
    }
    pal(base)
}

// ============ widget rendering ============

fn draw_kind<D: DrawTarget<Color = Rgb565>>(d: &mut D, r: &Rect, kind: &Kind, t: &Telemetry, now_ms: i64) {
    let (x, y, w, h) = (r.x, r.y, r.w as i32, r.h as i32);
    let cx = x + w / 2;
    match kind {
        Kind::Panel { color, radius } => {
            fill_round(d, x, y, w, h, *radius as i32, pal(*color));
        }
        Kind::Label { text: s, color, size, align } => {
            let sz = if *size == 0 { 14 } else { *size as u32 };
            let ax = match align {
                Align::Left => x + 2,
                Align::Center => cx,
                Align::Right => x + w - 2,
            };
            text(d, s, ax, y + h / 2, sz, pal(*color), align.h(), VerticalPosition::Center);
        }
        Kind::Stat { field, label, fmt, scale, unit, base, rules, size } => {
            let raw = field_value(t, *field as usize);
            let def = field_def(*field as usize);
            let f = fmt.unwrap_or_else(|| def.map(|d| d.fmt).unwrap_or(Fmt::Int));
            let sc = if *scale > 0 { *scale } else { def.map(|d| d.scale).unwrap_or(1) };
            let sz = if *size == 0 { 22 } else { *size as u32 };
            text(d, label, cx, y + 11, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            let s = format::format(raw, f, sc, unit);
            text(d, &s, cx, y + h / 2 + 6, sz, rule_color(raw, *base, rules), HorizontalAlignment::Center, VerticalPosition::Center);
        }
        Kind::Bar { field, label, scale, base, rules } => {
            let raw = field_value(t, *field as usize);
            let pct = if *scale > 0 { (raw * 100 / *scale).clamp(0, 100) } else { 0 };
            text(d, label, x + 4, y + 10, 11, pal(Pal::Dim), HorizontalAlignment::Left, VerticalPosition::Center);
            fill_rect(d, x + 4, y + h / 2, w - 8, h / 3, pal(Pal::Panel));
            fill_rect(d, x + 4, y + h / 2, (w - 8) * pct / 100, h / 3, rule_color(raw, *base, rules));
        }
        Kind::GearSpeed { speed } => {
            let g = if t.gear == 0 { 'N' } else { t.gear as char };
            text(d, &g.to_string(), cx, y + h / 2 - 6, 40, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
            if *speed {
                text(d, &t.speed_kmh.to_string(), cx, y + h - 26, 24, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
                text(d, "KM/H", cx, y + h - 8, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            }
        }
        Kind::RpmStrip => {
            let seg = 12;
            let sw = w / seg;
            for i in 0..seg {
                let c = segment_rgb(t, i, seg, &RevCfg::default(), &CarData::default(), now_ms);
                let col = if c == 0 { pal(Pal::Panel) } else { rgb888(c) };
                fill_round(d, x + i * sw + 1, y + 4, sw - 2, h - 8, 3, col);
            }
        }
        Kind::TyreGrid => {
            let temps = [t.tt_fl_m, t.tt_fr_m, t.tt_rl_m, t.tt_rr_m];
            let (bw, bh) = (w / 2, h / 2);
            for i in 0..4 {
                let (cxx, cyy) = (x + (i as i32 % 2) * bw, y + (i as i32 / 2) * bh);
                let col = if temps[i] > 95 { pal(Pal::Red) } else if temps[i] > 80 { pal(Pal::Amber) } else { pal(Pal::Green) };
                fill_round(d, cxx + 2, cyy + 2, bw - 4, bh - 4, 4, pal(Pal::Panel));
                text(d, &temps[i].to_string(), cxx + bw / 2, cyy + bh / 2, 14, col, HorizontalAlignment::Center, VerticalPosition::Center);
            }
        }
        Kind::TcDual => {
            text(d, "TC", x + w / 4, y + 12, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, "ABS", x + 3 * w / 4, y + 12, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, &t.tc.to_string(), x + w / 4, y + h / 2 + 6, 22, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, &t.abs.to_string(), x + 3 * w / 4, y + h / 2 + 6, 22, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
        }
        Kind::Sectors => {
            let secs = [t.s1_ms, t.s2_ms, t.s3_ms];
            let bs = [t.bs1_ms, t.bs2_ms, t.bs3_ms];
            let sw = w / 3;
            for i in 0..3 {
                let col = if secs[i] > 0 && bs[i] > 0 && secs[i] <= bs[i] { pal(Pal::Green) } else { pal(Pal::Amber) };
                let s = format::format(secs[i], Fmt::Sector, 1, "");
                text(d, &s, x + i as i32 * sw + sw / 2, y + h / 2, 12, col, HorizontalAlignment::Center, VerticalPosition::Center);
            }
        }
        Kind::LapPair => {
            let cur = format::format(t.cur_lap_ms, Fmt::Time, 1, "");
            let best = format::format(t.best_lap_ms, Fmt::Time, 1, "");
            text(d, "CURRENT", cx, y + 10, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, &cur, cx, y + h / 4 + 6, 18, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, "BEST", cx, y + h / 2 + 8, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            text(d, &best, cx, y + 3 * h / 4 + 4, 18, pal(Pal::Cyan), HorizontalAlignment::Center, VerticalPosition::Center);
        }
        Kind::Position { label } => {
            text(d, label, cx, y + 12, 11, pal(Pal::Dim), HorizontalAlignment::Center, VerticalPosition::Center);
            let s = alloc::format!("P{}/{}", t.position, t.field_size);
            text(d, &s, cx, y + h / 2 + 4, 22, pal(Pal::White), HorizontalAlignment::Center, VerticalPosition::Center);
        }
        Kind::Flag { field, base, rules } => {
            let raw = field_value(t, *field as usize);
            fill_round(d, x + 4, y + 4, w - 8, h - 8, 4, rule_color(raw, *base, rules));
        }
        Kind::Map => {
            let _ = Circle::new(Point::new(cx - h / 3, y + h / 6), (h / 3).max(1) as u32)
                .into_styled(PrimitiveStyle::with_stroke(pal(Pal::Dim), 1))
                .draw(d);
        }
    }
}

// ============ rendering: full + dirty-rect ============

/// Per-node content signature (FNV-1a of the telemetry that affects the node's
/// pixels). Static kinds hash to a constant so they draw once and never repaint.
fn node_sig(kind: &Kind, t: &Telemetry, now_ms: i64) -> u64 {
    fn h(vals: &[i64]) -> u64 {
        let mut x: u64 = 0xcbf29ce484222325;
        for &v in vals {
            x ^= v as u64;
            x = x.wrapping_mul(0x100000001b3);
        }
        x
    }
    let fv = |id: u8| field_value(t, id as usize) as i64;
    match kind {
        Kind::Panel { .. } | Kind::Label { .. } | Kind::Map => 0,
        Kind::Stat { field, .. } | Kind::Bar { field, .. } | Kind::Flag { field, .. } => h(&[fv(*field)]),
        Kind::GearSpeed { speed } => h(&[t.gear as i64, if *speed { t.speed_kmh as i64 } else { 0 }]),
        // blink phase keeps the rev strip live
        Kind::RpmStrip => h(&[t.rpm as i64, t.max_rpm as i64, t.shift_rpm as i64, now_ms / 80]),
        Kind::TyreGrid => h(&[t.tt_fl_m as i64, t.tt_fr_m as i64, t.tt_rl_m as i64, t.tt_rr_m as i64]),
        Kind::TcDual => h(&[t.tc as i64, t.abs as i64]),
        Kind::Sectors => h(&[t.s1_ms as i64, t.s2_ms as i64, t.s3_ms as i64, t.bs1_ms as i64, t.bs2_ms as i64, t.bs3_ms as i64]),
        Kind::LapPair => h(&[t.cur_lap_ms as i64, t.best_lap_ms as i64]),
        Kind::Position { .. } => h(&[t.position as i64, t.field_size as i64]),
    }
}

/// Cache of last-drawn node signatures for [`render_screen_diff`].
#[derive(Default)]
pub struct RenderCache {
    sigs: Vec<u64>,
}

impl RenderCache {
    pub fn new() -> Self {
        Self { sigs: Vec::new() }
    }
    /// Force a full repaint on the next [`render_screen_diff`] (e.g. after a layout
    /// swap or display wake).
    pub fn invalidate(&mut self) {
        self.sigs.clear();
    }
}

/// Full repaint: clear to the screen background and draw every node.
pub fn render_screen<D: DrawTarget<Color = Rgb565>>(s: &Screen, t: &Telemetry, now_ms: i64, d: &mut D) {
    let _ = d.clear(pal(s.bg));
    for node in &s.nodes {
        draw_kind(d, &node.rect, &node.kind, t, now_ms);
    }
}

/// Dirty-rect repaint: only redraw nodes whose telemetry changed since last call
/// (the rest of the panel — static chrome — is left untouched, so the device only
/// pushes the changed pixels over SPI). Pass a fresh [`RenderCache`] the first time.
pub fn render_screen_diff<D: DrawTarget<Color = Rgb565>>(
    s: &Screen,
    t: &Telemetry,
    now_ms: i64,
    cache: &mut RenderCache,
    d: &mut D,
) {
    let full = cache.sigs.len() != s.nodes.len();
    if full {
        let _ = d.clear(pal(s.bg));
        cache.sigs.clear();
        cache.sigs.resize(s.nodes.len(), 0);
    }
    for (i, node) in s.nodes.iter().enumerate() {
        let sig = node_sig(&node.kind, t, now_ms);
        if full || cache.sigs[i] != sig {
            if !full {
                // erase the node's rect with the screen background before repaint
                let r = &node.rect;
                fill_rect(d, r.x, r.y, r.w as i32, r.h as i32, pal(s.bg));
            }
            draw_kind(d, &node.rect, &node.kind, t, now_ms);
            cache.sigs[i] = sig;
        }
    }
}

// ============ desktop preview ============

/// A heap-backed RGB565 framebuffer + RGBA8 export — the desktop preview target
/// (what the dashboard blits into a Slint image). Behind the `std` feature.
#[cfg(feature = "std")]
mod framebuffer {
    use super::*;

    pub struct Framebuffer {
        pub w: u32,
        pub h: u32,
        pub buf: Vec<Rgb565>,
    }

    impl Framebuffer {
        pub fn new(w: u32, h: u32) -> Self {
            Self { w, h, buf: alloc::vec![Rgb565::BLACK; (w * h) as usize] }
        }
        pub fn to_rgba8(&self) -> Vec<u8> {
            let mut out = Vec::with_capacity((self.w * self.h * 4) as usize);
            for px in &self.buf {
                let (r, g, b) = (px.r(), px.g(), px.b());
                out.push((r << 3) | (r >> 2));
                out.push((g << 2) | (g >> 4));
                out.push((b << 3) | (b >> 2));
                out.push(255);
            }
            out
        }
    }

    impl OriginDimensions for Framebuffer {
        fn size(&self) -> Size {
            Size::new(self.w, self.h)
        }
    }

    impl DrawTarget for Framebuffer {
        type Color = Rgb565;
        type Error = core::convert::Infallible;
        fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Pixel<Self::Color>>,
        {
            for Pixel(p, c) in pixels {
                if p.x >= 0 && p.y >= 0 && (p.x as u32) < self.w && (p.y as u32) < self.h {
                    self.buf[(p.y as u32 * self.w + p.x as u32) as usize] = c;
                }
            }
            Ok(())
        }
    }
}

#[cfg(feature = "std")]
pub use framebuffer::Framebuffer;

// ============ demo ============

/// A built-in sample race screen (480x320) exercising the widget set, for previews,
/// tests and bootstrapping the editor.
pub fn demo_doc() -> UiDoc {
    let stat = |x: i32, y: i32, w: u32, h: u32, label: &str, field: u8, base: Pal, rules: Vec<Rule>| Node {
        rect: Rect { x, y, w, h },
        kind: Kind::Stat { field, label: label.into(), fmt: None, scale: 0, unit: String::new(), base, rules, size: 0 },
    };
    let panel = |x: i32, y: i32, w: u32, h: u32| Node {
        rect: Rect { x, y, w, h },
        kind: Kind::Panel { color: Pal::Panel, radius: 12 },
    };
    let nodes = alloc::vec![
        Node { rect: Rect { x: 0, y: 2, w: 480, h: 42 }, kind: Kind::RpmStrip },
        panel(136, 50, 208, 200),
        Node { rect: Rect { x: 136, y: 50, w: 208, h: 200 }, kind: Kind::GearSpeed { speed: true } },
        panel(4, 50, 128, 90),
        stat(4, 50, 128, 90, "DELTA", 10 /*delta_ms*/, Pal::Amber, alloc::vec![
            Rule { op: RuleOp::Lt, threshold: 0, color: Pal::Green },
            Rule { op: RuleOp::Gt, threshold: 0, color: Pal::Red },
        ]),
        panel(4, 150, 128, 120),
        Node { rect: Rect { x: 4, y: 150, w: 128, h: 120 }, kind: Kind::TyreGrid },
        panel(348, 50, 128, 90),
        stat(348, 50, 128, 90, "FUEL", 23 /*fuel_dl*/, Pal::Amber, alloc::vec![]),
        panel(348, 150, 128, 120),
        Node { rect: Rect { x: 348, y: 150, w: 128, h: 120 }, kind: Kind::Position { label: "POS".into() } },
        Node { rect: Rect { x: 0, y: 276, w: 480, h: 42 }, kind: Kind::LapPair },
    ];
    UiDoc { version: 1, screens: alloc::vec![Screen { display: 0, w: 480, h: 320, bg: Pal::Bg, nodes }] }
}

/// Demo telemetry (a believable race frame) for previews without a device.
pub fn demo_telem() -> Telemetry {
    let mut t = Telemetry::idle();
    t.gear = b'4';
    t.speed_kmh = 212;
    t.rpm = 7100;
    t.max_rpm = 8200;
    t.shift_rpm = 7800;
    t.delta_ms = -3000;
    t.fuel_dl = 486;
    t.position = 4;
    t.field_size = 20;
    t.cur_lap_ms = 84318;
    t.best_lap_ms = 82900;
    t.tt_fl_m = 88;
    t.tt_fr_m = 90;
    t.tt_rl_m = 97;
    t.tt_rr_m = 86;
    t
}
