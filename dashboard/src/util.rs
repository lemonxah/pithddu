pub fn atoi(s: &str) -> i32 {
    let s = s.trim_start();
    let b = s.as_bytes();
    let mut i = 0;
    let mut sign: i64 = 1;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        if b[i] == b'-' {
            sign = -1;
        }
        i += 1;
    }
    let mut n: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        n = n * 10 + (b[i] - b'0') as i64;
        if n > i32::MAX as i64 {
            n = i32::MAX as i64;
        }
        i += 1;
    }
    (sign * n) as i32
}

pub fn trim(s: &str) -> String {
    s.trim_matches(|c| c == ' ' || c == '\t').to_string()
}

pub fn hex_prefix(s: &str) -> u32 {
    let hex: String = s
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_hexdigit())
        .collect();
    u64::from_str_radix(&hex, 16).map(|v| v as u32).unwrap_or(0)
}

pub fn norm_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub fn car_variant(id: &str, name: &str) -> String {
    let norm_sep = |s: &str| -> String {
        s.chars()
            .map(|c| if c == '_' || c == '-' { ' ' } else { c })
            .collect()
    };
    let out = norm_sep(id);
    let nm = norm_sep(name);
    let lout = out.to_ascii_lowercase();
    let lnm = nm.to_ascii_lowercase();
    if let Some(p) = lout.find(&lnm) {
        let mut o = out.clone();
        let end = (p + nm.len()).min(o.len());
        o.replace_range(p..end, "");
        trim(&o)
    } else {
        trim(&out)
    }
}
