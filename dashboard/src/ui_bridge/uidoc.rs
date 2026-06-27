//! Bridge from the authoring model (zones + ModSpecs) to a pith-ui `UiDoc`, plus
//! the live desktop preview. The preview renders through the EXACT same pith-ui
//! engine the device runs (same fonts, same dirty-rect draw), so the GUI mirror
//! is pixel-identical to the ST7796 panel — and the `UiDoc` we render here is the
//! same one pushed to the device via `@UI`.

use slint::ComponentHandle;

use crate::state::{ModSpec, State};
use crate::telemetry::field_id_from_str;
use crate::{AppWindow, RaceLayout};

use pith_core::format::{Fmt, Pal, RuleOp};
use pith_core::registry::telemetry_from_fields;
use pith_core::simhub::Telemetry;
use pith_ui::{Kind, Node, Rect, Rule, Screen, UiDoc};

// The race panel is 480×320 (320×480 physical, rotated 90°). These zones mirror
// the firmware's ZONES table so existing layouts map to the same on-screen rects.
const SCREEN_W: u32 = 480;
const SCREEN_H: u32 = 320;
const ZONES: [(&str, i32, i32, i32, i32, bool); 5] = [
    ("topStrip", 0, 2, 480, 42, true),
    ("leftRail", 4, 50, 128, 220, false),
    ("center", 136, 50, 208, 220, false),
    ("rightRail", 348, 50, 128, 220, false),
    ("bottom", 0, 276, 480, 42, true),
];

fn rules_of(m: &ModSpec) -> Vec<Rule> {
    m.rules
        .iter()
        .map(|r| Rule {
            op: RuleOp::from_str(&r.op),
            threshold: r.v,
            color: Pal::from_str(&r.color),
        })
        .collect()
}

fn kind_of(m: &ModSpec) -> Kind {
    let field = field_id_from_str(&m.field) as u8;
    let base = Pal::from_str(&m.base);
    let fmt = if m.fmt_type.is_empty() {
        None
    } else {
        Some(Fmt::from_str(&m.fmt_type))
    };
    let size = m.size_pct.clamp(0, 255) as u8;
    match m.kind.as_str() {
        "bar" => Kind::Bar {
            field,
            label: m.label.clone(),
            scale: m.scale,
            base,
            rules: rules_of(m),
        },
        "gearSpeed" => Kind::GearSpeed { speed: true },
        "gear" => Kind::GearSpeed { speed: false },
        "rpmStrip" => Kind::RpmStrip,
        "tyreGrid" => Kind::TyreGrid,
        "tcDual" => Kind::TcDual,
        "sectors" => Kind::Sectors,
        "lapPair" => Kind::LapPair,
        "position" => Kind::Position {
            label: m.label.clone(),
        },
        "flag" => Kind::Flag {
            field,
            base,
            rules: rules_of(m),
        },
        "map" => Kind::Map,
        // "stat" and any unknown kind fall back to a stat (the most general widget).
        _ => Kind::Stat {
            field,
            label: m.label.clone(),
            fmt,
            scale: m.scale,
            unit: m.unit.clone(),
            base,
            rules: rules_of(m),
            size,
        },
    }
}

/// Lay the active zone layout out into a freeform pith-ui `Screen` for `display`,
/// matching the firmware's per-zone auto-layout (so this is WYSIWYG with the
/// current device render).
fn build_screen(s: &State, display: u8) -> Screen {
    let mut nodes: Vec<Node> = Vec::new();
    for &(key, zx, zy, zw, zh, horiz) in ZONES.iter() {
        let zone = match s.zones.iter().find(|z| z.key == key) {
            Some(z) => z,
            None => continue,
        };
        let mods: Vec<&ModSpec> = zone.modules.iter().filter(|m| m.enabled).collect();
        let n = mods.len() as i32;
        if n == 0 {
            continue;
        }
        for (i, m) in mods.iter().enumerate() {
            let i = i as i32;
            let (x, y, w, h) = if horiz {
                (zx + zw * i / n, zy, zw / n, zh)
            } else {
                (zx, zy + zh * i / n, zw, zh / n)
            };
            nodes.push(Node {
                rect: Rect {
                    x,
                    y,
                    w: w.max(0) as u32,
                    h: h.max(0) as u32,
                },
                kind: kind_of(m),
            });
        }
    }
    Screen {
        display,
        w: SCREEN_W,
        h: SCREEN_H,
        bg: Pal::Bg,
        nodes,
    }
}

/// Build the full UiDoc for the device (currently a single race screen on
/// display 0; the side display is added when the editor gains a second screen).
pub fn build_uidoc(s: &State) -> UiDoc {
    UiDoc {
        version: 1,
        screens: vec![build_screen(s, 0)],
    }
}

/// Serialize the UiDoc to JSON for the `@UI` wire command (text-safe, matches the
/// firmware's `serde_json::from_str::<UiDoc>` decode).
pub fn build_uidoc_json(s: &State) -> String {
    serde_json::to_string(&build_uidoc(s)).unwrap_or_else(|_| "{}".to_string())
}

/// Rehydrate a pith-ui Telemetry from the dashboard's flat field array + gear.
fn current_telemetry(s: &State) -> Telemetry {
    let mut t = telemetry_from_fields(&s.telem);
    t.gear = s.gear_ch as u8;
    t
}

/// Render `doc`'s display-0 screen against live telemetry into a slint image,
/// using the device's own pith-ui renderer + fonts (pixel-identical mirror).
fn render_image(doc: &UiDoc, t: &Telemetry) -> Option<slint::Image> {
    let screen = doc.screens.iter().find(|s| s.display == 0)?;
    let mut fb = pith_ui::Framebuffer::new(screen.w, screen.h);
    pith_ui::render_screen(screen, t, 0, &mut fb);
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(screen.w, screen.h);
    buf.make_mut_bytes().copy_from_slice(&fb.to_rgba8());
    Some(slint::Image::from_rgba8(buf))
}

/// Build the UiDoc from the current layout, render it with live telemetry, and
/// push the resulting image into the RaceLayout preview. Called on every layout
/// edit and telemetry tick so the mirror stays live + exact.
pub fn push_preview(ui: &AppWindow, s: &State) {
    let doc = build_uidoc(s);
    let t = current_telemetry(s);
    if let Some(img) = render_image(&doc, &t) {
        ui.global::<RaceLayout>().set_preview_image(img);
    }
}
