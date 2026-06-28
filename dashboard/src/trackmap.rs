//! Bundled track-map outlines. The Map widget shows a track outline + a position
//! dot driven by the `track_pct` telemetry. Outlines are ordered, closed polylines
//! of points normalized to 0..=1000 (x,y), start/finish first. The app bakes the
//! selected track's outline into the pith-ui `Kind::Map` node it pushes to the
//! device (so the device needs no track data of its own).
//!
//! This ships a few representative shapes; add real circuits by appending to
//! `TRACK_NAMES` + an arm in `outline_for` (a flat [x0,y0,x1,y1,...] in 0..=1000).

/// Selectable track names shown in the GUI picker. Index 0 = no map.
pub const TRACK_NAMES: &[&str] = &["(none)", "Oval", "Club Circuit", "Infield Road"];

/// Normalized outline (flat x,y pairs in 0..=1000) for a track name, or empty.
pub fn outline_for(name: &str) -> Vec<u16> {
    match name {
        "Oval" => ellipse(64, 480, 320),
        "Club Circuit" => CLUB.to_vec(),
        "Infield Road" => INFIELD.to_vec(),
        _ => Vec::new(),
    }
}

/// Index of a track name in `TRACK_NAMES` (0 / none when not found).
pub fn track_index(name: &str) -> i32 {
    TRACK_NAMES.iter().position(|t| *t == name).unwrap_or(0) as i32
}

/// An oval centred at (500,500) with the given half-extents, `n` points.
fn ellipse(n: usize, rx: i32, ry: i32) -> Vec<u16> {
    let mut v = Vec::with_capacity(n * 2);
    for i in 0..n {
        let a = (i as f32) / (n as f32) * core::f32::consts::TAU;
        let x = 500.0 + rx as f32 * a.cos();
        let y = 500.0 + ry as f32 * a.sin();
        v.push(x.clamp(0.0, 1000.0) as u16);
        v.push(y.clamp(0.0, 1000.0) as u16);
    }
    v
}

// Hand-made representative circuit loops (start/finish first, clockwise).
#[rustfmt::skip]
const CLUB: &[u16] = &[
    120, 820,  120, 300,  260, 140,  520, 120,  640, 220,
    560, 360,  700, 420,  880, 360,  900, 560,  760, 700,
    820, 840,  560, 900,  300, 880,
];
#[rustfmt::skip]
const INFIELD: &[u16] = &[
    100, 500,  220, 200,  500, 120,  780, 200,  900, 500,
    760, 560,  640, 420,  500, 520,  640, 660,  760, 560,
    900, 500,  780, 800,  500, 880,  220, 800,
];
