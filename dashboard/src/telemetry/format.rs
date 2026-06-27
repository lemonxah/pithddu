#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fmt {
    Int = 0,
    Fixed1,
    Fixed2,
    Time,
    Sector,
    Delta,
    Str,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Lt = 0,
    Le,
    Eq,
    Ge,
    Gt,
}

pub const FMT_NAMES: [&str; 7] = [
    "int", "fixed1", "fixed2", "time", "sector", "delta", "string",
];
pub const OP_NAMES: [&str; 5] = ["<", "<=", "==", ">=", ">"];
pub const PALETTE_TOKENS: [&str; 8] = [
    "white", "dim", "green", "amber", "red", "cyan", "blue", "purple",
];
pub const KIND_OPTIONS: [&str; 12] = [
    "stat",
    "bar",
    "gear",
    "gearSpeed",
    "rpmStrip",
    "tyreGrid",
    "tcDual",
    "sectors",
    "lapPair",
    "map",
    "flag",
    "position",
];

pub fn fmt_from_str(s: &str) -> Fmt {
    match FMT_NAMES.iter().position(|&n| n == s) {
        Some(0) => Fmt::Int,
        Some(1) => Fmt::Fixed1,
        Some(2) => Fmt::Fixed2,
        Some(3) => Fmt::Time,
        Some(4) => Fmt::Sector,
        Some(5) => Fmt::Delta,
        Some(6) => Fmt::Str,
        _ => Fmt::Int,
    }
}

pub fn op_from_str(s: &str) -> Op {
    match OP_NAMES.iter().position(|&n| n == s) {
        Some(0) => Op::Lt,
        Some(1) => Op::Le,
        Some(2) => Op::Eq,
        Some(3) => Op::Ge,
        Some(4) => Op::Gt,
        _ => Op::Gt,
    }
}

pub fn idx_of(list: &[&str], v: &str) -> i32 {
    list.iter()
        .position(|&x| x == v)
        .map(|i| i as i32)
        .unwrap_or(-1)
}

pub fn fmtc_format(v: i32, fmt: Fmt, scale: i32, unit: &str) -> String {
    let scale = if scale <= 0 { 1 } else { scale };
    match fmt {
        Fmt::Time => {
            if v <= 0 {
                return "--:--.---".to_string();
            }
            let mut o = format!("{}:{:02}.{:03}", v / 60000, (v / 1000) % 60, v % 1000);
            if !unit.is_empty() {
                o.push_str(unit);
            }
            o
        }
        Fmt::Sector => {
            if v <= 0 {
                return "--.---".to_string();
            }
            let mut o = format!("{}.{:03}", v / 1000, v % 1000);
            if !unit.is_empty() {
                o.push_str(unit);
            }
            o
        }
        Fmt::Delta => {
            let v = v.clamp(-99999, 99999);
            let sign = if v >= 0 { '+' } else { '-' };
            let a = v.abs();
            format!("{}{}.{:04}", sign, a / 10000, a % 10000)
        }
        Fmt::Fixed1 => {
            let whole = v / scale;
            let frac = (v.wrapping_abs() * 10 / scale) % 10;
            format!("{}.{}{}", whole, frac, unit)
        }
        Fmt::Fixed2 => {
            let whole = v / scale;
            let frac = (v.wrapping_abs() * 100 / scale) % 100;
            format!("{}.{:02}{}", whole, frac, unit)
        }
        Fmt::Int | Fmt::Str => format!("{}{}", v / scale, unit),
    }
}

pub fn rule_match(v: i32, op: Op, rule_v: i32) -> bool {
    match op {
        Op::Lt => v < rule_v,
        Op::Le => v <= rule_v,
        Op::Eq => v == rule_v,
        Op::Ge => v >= rule_v,
        Op::Gt => v > rule_v,
    }
}

pub fn palette_color(tok: &str) -> u32 {
    match tok {
        "bg" => 0x060708,
        "panel" => 0x131519,
        "dim" => 0x636A74,
        "green" => 0x00E676,
        "amber" => 0xFFB300,
        "red" => 0xFF3B30,
        "cyan" => 0x7FC9B1,
        "blue" => 0x2E9DFF,
        "purple" => 0xD500F9,
        _ => 0xE8EAED,
    }
}
