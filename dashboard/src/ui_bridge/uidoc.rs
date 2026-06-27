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

// The race panels are 480×320 (320×480 physical, rotated 90°).
pub const SCREEN_W: u32 = 480;
pub const SCREEN_H: u32 = 320;

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
    // The shared builtin() defines each widget — decomposable ones (stat) come
    // back as a composable Widget(El) tree the editor can rearrange.
    pith_ui::builtin(
        &m.kind,
        field,
        &m.label,
        fmt,
        m.scale,
        &m.unit,
        base,
        rules_of(m),
        size,
    )
}

/// Build a pith-ui `Screen` from the freeform nodes assigned to `display`.
pub fn build_screen(s: &State, display: u8) -> Screen {
    let nodes: Vec<Node> = s
        .nodes
        .iter()
        .filter(|m| m.display == display && m.enabled)
        .map(|m| Node {
            rect: Rect {
                x: m.x,
                y: m.y,
                w: m.w.max(0) as u32,
                h: m.h.max(0) as u32,
            },
            kind: kind_of(m),
        })
        .collect();
    Screen {
        display,
        w: SCREEN_W,
        h: SCREEN_H,
        bg: Pal::Bg,
        nodes,
    }
}

/// Build the full UiDoc: always a display-0 screen, plus display 1 when it has
/// nodes (so the device renders both panels via pith-ui).
pub fn build_uidoc(s: &State) -> UiDoc {
    let mut screens = vec![build_screen(s, 0)];
    if s.nodes.iter().any(|m| m.display == 1) {
        screens.push(build_screen(s, 1));
    }
    UiDoc {
        version: 1,
        screens,
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

/// Render `screen` against live telemetry into a slint image, using the device's
/// own pith-ui renderer + fonts (pixel-identical mirror).
fn render_image(screen: &Screen, t: &Telemetry) -> slint::Image {
    let mut fb = pith_ui::Framebuffer::new(screen.w, screen.h);
    pith_ui::render_screen(screen, t, 0, &mut fb);
    let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(screen.w, screen.h);
    buf.make_mut_bytes().copy_from_slice(&fb.to_rgba8());
    slint::Image::from_rgba8(buf)
}

/// Render the display currently being edited with live telemetry, and push the
/// resulting image into the RaceLayout preview. Called on every layout edit and
/// telemetry tick so the mirror stays live + exact.
pub fn push_preview(ui: &AppWindow, s: &State) {
    let screen = build_screen(s, s.edit_display);
    let t = current_telemetry(s);
    let img = render_image(&screen, &t);
    ui.global::<RaceLayout>().set_preview_image(img);
}

#[cfg(test)]
mod tests {
    use crate::state::{ModSpec, State};

    // The firmware decodes @UI with serde_json::from_str::<UiDoc>; prove the
    // dashboard's serde_json::to_string output round-trips to the same type.
    #[test]
    fn uidoc_json_roundtrips_for_firmware() {
        let mut s = State::default();
        s.nodes = vec![
            ModSpec { id: "a".into(), kind: "gearSpeed".into(), x: 170, y: 120, w: 140, h: 80, display: 0, ..Default::default() },
            ModSpec { id: "b".into(), kind: "stat".into(), field: "fuel_dl".into(), label: "FUEL".into(), x: 10, y: 10, w: 100, h: 50, display: 1, ..Default::default() },
        ];
        let json = super::build_uidoc_json(&s);
        let doc: pith_ui::UiDoc = serde_json::from_str(&json).expect("firmware-side decode");
        assert_eq!(doc.screens.len(), 2); // display 0 + display 1
        assert_eq!(doc.screens[0].display, 0);
        assert_eq!(doc.screens[1].display, 1);
        assert_eq!(doc.screens[0].nodes.len(), 1);
    }
}
