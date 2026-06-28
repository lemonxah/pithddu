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

// ---------------------------------------------------------------------------
// Self-learned track map.
//
// Real circuits (Spa, Silverstone, …) aren't in the bundled list, so when the
// sim streams a car position we learn the outline ourselves: bucket the world
// X/Z by lap progress (track_pct), and once enough of the lap is covered, emit a
// normalized closed polyline the device's Map widget renders. Keyed by track_pct
// (not time) so the points stay correctly ordered regardless of sample rate.
// ---------------------------------------------------------------------------

/// Number of lap-progress buckets (≈ one point every 0.55% of the lap).
pub const MAP_BUCKETS: usize = 180;

#[derive(Clone)]
pub struct MapLearner {
    samples: Vec<Option<(i32, i32)>>, // per-bucket world (x, z)
    filled: usize,
    /// Track this learner is currently mapping (so a track change resets it).
    pub track: String,
}

impl Default for MapLearner {
    fn default() -> Self {
        Self { samples: vec![None; MAP_BUCKETS], filled: 0, track: String::new() }
    }
}

impl MapLearner {
    /// Start over for a new track.
    pub fn reset(&mut self, track: &str) {
        self.samples = vec![None; MAP_BUCKETS];
        self.filled = 0;
        self.track = track.to_string();
    }

    /// Record one position sample keyed by lap progress. Returns true if it filled
    /// a previously-empty bucket (i.e. the map gained detail).
    pub fn record(&mut self, track_pct: i32, x: i32, z: i32) -> bool {
        // Ignore the all-zero frame a game sends before it's feeding position.
        if track_pct <= 0 && x == 0 && z == 0 {
            return false;
        }
        let b = ((track_pct.clamp(0, 1000) as usize) * MAP_BUCKETS / 1001).min(MAP_BUCKETS - 1);
        let was_empty = self.samples[b].is_none();
        self.samples[b] = Some((x, z));
        if was_empty {
            self.filled += 1;
        }
        was_empty
    }

    /// Enough of the lap is covered to push a usable outline.
    pub fn complete(&self) -> bool {
        self.filled >= MAP_BUCKETS * 7 / 10
    }

    /// Build a normalized closed outline (flat x,y in 0..=1000), aspect-preserved
    /// and centred, Y flipped so the world's +Z reads as "up". Empty if too sparse.
    pub fn outline(&self) -> Vec<u16> {
        if self.filled < MAP_BUCKETS / 2 {
            return Vec::new();
        }
        let pts: Vec<(i32, i32)> = self.samples.iter().filter_map(|s| *s).collect();
        let (mut minx, mut maxx, mut minz, mut maxz) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for &(x, z) in &pts {
            minx = minx.min(x);
            maxx = maxx.max(x);
            minz = minz.min(z);
            maxz = maxz.max(z);
        }
        let dx = (maxx - minx).max(1);
        let dz = (maxz - minz).max(1);
        let span = dx.max(dz) as i64; // uniform scale → no distortion
        let ox = (span - dx as i64) / 2;
        let oz = (span - dz as i64) / 2;
        // Inset to ~3..=997 so the outline never touches the widget edge.
        let mut out = Vec::with_capacity(pts.len() * 2);
        for &(x, z) in &pts {
            let nx = 3 + ((x - minx) as i64 + ox) * 994 / span;
            let ny = (z - minz) as i64 + oz;
            let ny = 3 + (994 - ny * 994 / span);
            out.push(nx.clamp(0, 1000) as u16);
            out.push(ny.clamp(0, 1000) as u16);
        }
        out
    }
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
