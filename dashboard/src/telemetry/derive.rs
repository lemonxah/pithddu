//! Dashboard-side derived telemetry: core fields no game transmits directly but
//! that we can compute from the stream — **best lap** (min completed lap),
//! **current lap time** (wall-clock since the lap rolled, for sources that lack
//! it), **fuel per lap / laps-left** (fuel burn across laps) and a **lap delta**
//! (current pace vs best, by track position). Each only fills a field the source
//! left empty, so a source that *does* provide it always wins.

use std::sync::OnceLock;
use std::time::Instant;

use pith_core::simhub::Telemetry;

/// Monotonic milliseconds since first use (for wall-clock lap timing).
fn now_ms() -> u64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// All derived-field trackers, run in order each frame.
pub struct Derived {
    best: BestLap,
    cur: CurLap,
    fuel: Fuel,
    delta: Delta,
}

impl Default for Derived {
    fn default() -> Self {
        Self { best: BestLap::default(), cur: CurLap::default(), fuel: Fuel::default(), delta: Delta::default() }
    }
}

impl Derived {
    pub fn update(&mut self, t: &mut Telemetry) {
        self.best.update(t); // best needs last_lap; run before delta
        self.cur.update(t); // current-lap time before delta (delta uses it)
        self.fuel.update(t);
        self.delta.update(t);
    }
}

/// Best lap = the fastest completed lap seen (when the source doesn't send one).
#[derive(Default)]
struct BestLap {
    best_ms: i32,
    last_seen_lap: i32,
}

impl BestLap {
    fn update(&mut self, t: &mut Telemetry) {
        if t.laps_done < self.last_seen_lap {
            self.best_ms = 0; // new session → forget
        }
        self.last_seen_lap = t.laps_done;
        for v in [t.best_lap_ms, t.last_lap_ms] {
            if v > 0 && (self.best_ms == 0 || v < self.best_ms) {
                self.best_ms = v;
            }
        }
        if t.best_lap_ms == 0 && self.best_ms > 0 {
            t.best_lap_ms = self.best_ms;
        }
    }
}

/// Current lap time by wall clock (for sources with a lap counter but no live
/// lap time, e.g. GT7). Approximate — keeps running if the game is paused.
#[derive(Default)]
struct CurLap {
    started: bool,
    last_lap: i32,
    lap_start_ms: u64,
}

impl CurLap {
    fn update(&mut self, t: &mut Telemetry) {
        if t.cur_lap_ms > 0 {
            return; // source provides it
        }
        let now = now_ms();
        if !self.started || t.laps_done != self.last_lap {
            self.started = true;
            self.last_lap = t.laps_done;
            self.lap_start_ms = now;
        }
        t.cur_lap_ms = now.saturating_sub(self.lap_start_ms) as i32;
    }
}

/// Fuel burned per completed lap → `fuel_per_lap_ml` + `fuel_laps_x10`.
#[derive(Default)]
struct Fuel {
    started: bool,
    last_lap: i32,
    lap_start_fuel_dl: i32,
    per_lap_ml: i32,
}

impl Fuel {
    fn update(&mut self, t: &mut Telemetry) {
        if t.fuel_per_lap_ml > 0 {
            self.per_lap_ml = t.fuel_per_lap_ml; // source provides it — adopt
        } else {
            if !self.started || t.laps_done < self.last_lap {
                self.started = true;
                self.last_lap = t.laps_done;
                self.lap_start_fuel_dl = t.fuel_dl;
            } else if t.laps_done > self.last_lap {
                let used_ml = (self.lap_start_fuel_dl - t.fuel_dl) * 100; // 1 dl = 100 ml
                self.last_lap = t.laps_done;
                self.lap_start_fuel_dl = t.fuel_dl;
                if used_ml > 50 && used_ml < 30_000 {
                    self.per_lap_ml = if self.per_lap_ml == 0 {
                        used_ml
                    } else {
                        (self.per_lap_ml * 3 + used_ml) / 4 // EMA
                    };
                }
            }
            if self.per_lap_ml > 0 {
                t.fuel_per_lap_ml = self.per_lap_ml;
            }
        }
        if t.fuel_laps_x10 == 0 && self.per_lap_ml > 0 && t.fuel_dl > 0 {
            t.fuel_laps_x10 = t.fuel_dl * 1000 / self.per_lap_ml;
        }
    }
}

const NB: usize = 100; // track-position buckets

/// Delta = current-lap pace vs the best lap, sampled by track position (0.1 ms
/// units; negative = ahead). Approximate (bucketed) — matches how HUDs build it.
struct Delta {
    started: bool,
    have_ref: bool,
    last_lap: i32,
    best_lap_ms: i32,
    best_ref: [i32; NB],
    cur: [i32; NB],
}

impl Default for Delta {
    fn default() -> Self {
        Self { started: false, have_ref: false, last_lap: 0, best_lap_ms: 0, best_ref: [0; NB], cur: [0; NB] }
    }
}

impl Delta {
    fn update(&mut self, t: &mut Telemetry) {
        if t.delta_ms != 0 {
            return; // source provides a real delta
        }
        let bucket = ((t.track_pct.clamp(0, 1000) as usize) * NB / 1001).min(NB - 1);
        let cur_ms = t.cur_lap_ms;
        if !self.started || t.laps_done < self.last_lap {
            self.started = true;
            self.last_lap = t.laps_done;
            self.cur = [0; NB];
        } else if t.laps_done > self.last_lap {
            if t.last_lap_ms > 0 && (!self.have_ref || t.last_lap_ms < self.best_lap_ms) {
                self.best_ref = self.cur;
                self.best_lap_ms = t.last_lap_ms;
                self.have_ref = true;
            }
            self.last_lap = t.laps_done;
            self.cur = [0; NB];
        }
        if cur_ms > 0 {
            self.cur[bucket] = cur_ms;
            if self.have_ref {
                let r = self.best_ref[bucket];
                if r > 0 {
                    t.delta_ms = (cur_ms - r) * 10; // ms → 0.1 ms units
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn best_lap_from_completed() {
        let mut d = Derived::default();
        // Each frame is a fresh parse_line where the source leaves best_lap_ms = 0.
        let frame = |d: &mut Derived, lap, last| {
            let mut t = Telemetry::idle();
            t.laps_done = lap;
            t.last_lap_ms = last;
            d.update(&mut t);
            t.best_lap_ms
        };
        assert_eq!(frame(&mut d, 2, 84_000), 84_000);
        assert_eq!(frame(&mut d, 3, 82_500), 82_500); // faster
        assert_eq!(frame(&mut d, 4, 83_000), 82_500); // slower → best stays
    }

    #[test]
    fn fuel_per_lap_from_burn() {
        let mut d = Derived::default();
        let mut t = Telemetry::idle();
        t.laps_done = 1;
        t.fuel_dl = 500;
        d.update(&mut t);
        t.laps_done = 2;
        t.fuel_dl = 476; // burned 2.4 L
        d.update(&mut t);
        assert_eq!(t.fuel_per_lap_ml, 2400);
        assert_eq!(t.fuel_laps_x10, 476 * 1000 / 2400);
    }
}
