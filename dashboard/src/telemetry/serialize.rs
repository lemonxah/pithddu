//! `Telemetry` → Pith `$`-frame serializer. The implementation now lives in
//! `pith_core::simhub::Telemetry::to_frame` (shared with the in-prefix tools);
//! this thin wrapper keeps the existing `frame_from_telem` call sites unchanged.

use pith_core::simhub::Telemetry;

/// Render `t` as one `$`-frame line (no trailing newline).
pub fn frame_from_telem(t: &Telemetry) -> String {
    t.to_frame()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pith_core::simhub::parse_line;

    #[test]
    fn roundtrips_through_parser() {
        let mut t = Telemetry::idle();
        t.gear = b'3';
        t.speed_kmh = 212;
        t.rpm = 6800;
        t.max_rpm = 7500;
        t.shift_rpm = 7400;
        t.position = 4;
        t.cur_lap_ms = 84012;
        t.delta_ms = -3000;
        t.throttle = 100;
        t.pos_x = -1234;
        t.pos_z = 5678;
        let frame = frame_from_telem(&t);
        let back = parse_line(&frame).expect("serialized frame must parse");
        assert_eq!(back, t);
    }
}
