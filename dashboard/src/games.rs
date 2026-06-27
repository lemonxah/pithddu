use crate::catalog::GAME_PROCS;

#[cfg(target_os = "linux")]
fn running_procs() -> Vec<String> {
    let mut v = Vec::new();
    let rd = match std::fs::read_dir("/proc") {
        Ok(r) => r,
        Err(_) => return v,
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.is_empty() || !name.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        if let Ok(bytes) = std::fs::read(e.path().join("cmdline")) {
            let cmd: String = bytes
                .iter()
                .map(|&b| if b == 0 { ' ' } else { b as char })
                .collect();
            if !cmd.trim().is_empty() {
                v.push(cmd);
            }
        }
    }
    v
}

#[cfg(not(target_os = "linux"))]
fn running_procs() -> Vec<String> {
    Vec::new()
}

pub fn detect_game(sims: &[(String, String)]) -> i32 {
    for p in running_procs() {
        let lp = p.to_lowercase();
        for (needle, sim_id) in GAME_PROCS {
            if lp.contains(needle) {
                if let Some(i) = sims.iter().position(|s| s.1 == *sim_id) {
                    return i as i32;
                }
            }
        }
    }
    -1
}
