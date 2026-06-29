use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde_json::Value;

use crate::ctx::Ctx;
use crate::net::http::{http_download_file, http_get};
use crate::paths::*;
use crate::state::{CarItem, State};
use crate::ui_bridge::cars::{push_car_results, push_classes, rebuild_filtered};
use crate::ui_bridge::shift::{push_led_model, push_shift_scalars};
use crate::ui_bridge::sstr;
use crate::util::{norm_name, trim};
use crate::CarLib;

const MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/Lovely-Sim-Racing/lovely-car-data/main/data/manifest.json";
const CAR_BASE_URL: &str =
    "https://raw.githubusercontent.com/Lovely-Sim-Racing/lovely-car-data/main/data/";
const COMMITS_URL: &str =
    "https://api.github.com/repos/Lovely-Sim-Racing/lovely-car-data/commits/main";
const TARBALL_URL: &str =
    "https://codeload.github.com/Lovely-Sim-Racing/lovely-car-data/tar.gz/refs/heads/main";

fn json_int(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
}

pub fn derive_redline(j: &Value) -> i32 {
    let mut rl = 0;
    if let Some(arr) = j.get("ledRpm").and_then(|x| x.as_array()) {
        if let Some(obj) = arr.first().and_then(|x| x.as_object()) {
            for (_gear, a) in obj {
                if let Some(items) = a.as_array() {
                    for v in items {
                        if let Some(n) = json_int(v) {
                            if n as i32 > rl {
                                rl = n as i32;
                            }
                        }
                    }
                }
            }
        }
    }
    rl
}

pub fn parse_car_led_colors(j: &Value) -> Vec<u32> {
    let arr = match j.get("ledColor").and_then(|x| x.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let raw = j.get("ledNumber").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    let n = if raw > 12 { 12 } else { raw };
    let skip = if raw > 12 { raw - 12 } else { 0 };
    let mut out = Vec::new();
    for i in 0..n {
        let idx = (skip + i + 1) as usize;
        let mut s = arr
            .get(idx)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        if s.starts_with('#') {
            s = s[1..].to_string();
        }
        out.push(crate::util::hex_prefix(&s) & 0xFFFFFF);
    }
    out
}

pub fn load_car_into_leds(s: &mut State, j: &Value) {
    let cols = parse_car_led_colors(j);
    let redline = derive_redline(j);
    if cols.is_empty() || redline <= 0 {
        return;
    }
    let raw = j.get("ledNumber").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
    let skip = if raw > 12 { raw - 12 } else { 0 };
    let gears = j
        .get("ledRpm")
        .and_then(|x| x.as_array())
        .and_then(|a| a.first())
        .filter(|o| o.is_object());
    for gear in 1..=6usize {
        let key = gear.to_string();
        let arr = gears.and_then(|g| g.get(&key)).and_then(|a| a.as_array());
        for i in 0..12 {
            let col = if i < cols.len() { cols[i] } else { 0 };
            let mut thr = 0;
            if let Some(a) = arr {
                let idx = (skip as usize) + i + 1;
                if let Some(v) = a.get(idx).and_then(json_int) {
                    thr = (v as i32) * 100 / redline;
                }
            }
            s.leds[gear][i].rgb = col;
            s.leds[gear][i].threshold = thr;
        }
    }
}

fn car_dedup_sig(sim: &str, it: &CarItem) -> String {
    let body = read_file(&data_root().join(&it.path));
    if !body.is_empty() {
        let mut b = body;
        if let Some(p) = b.find("\"carId\"") {
            let e = b[p..].find('\n').map(|o| p + o).unwrap_or(b.len());
            b.replace_range(p..e, "");
        }
        format!("{sim}\u{1f}{b}")
    } else {
        format!("{sim}\u{1f}#{}", it.id)
    }
}

pub fn parse_manifest(s: &mut State, body: &str) {
    s.all_cars.clear();
    let j: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(_) => return,
    };
    let cars = match j.get("cars").and_then(|c| c.as_object()) {
        Some(c) => c,
        None => return,
    };
    let mut seen = std::collections::HashSet::new();
    for (sim, arr) in cars {
        if let Some(list) = arr.as_array() {
            for c in list {
                let it = CarItem {
                    sim: sim.clone(),
                    name: trim(c.get("carName").and_then(|x| x.as_str()).unwrap_or("")),
                    id: c
                        .get("carId")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    path: c
                        .get("path")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    ..Default::default()
                };
                if it.name.is_empty() {
                    continue;
                }
                if !seen.insert(car_dedup_sig(sim, &it)) {
                    continue;
                }
                s.all_cars.push(it);
            }
        }
    }
}

pub async fn car_body_by_path(path: &str) -> String {
    let body = read_file(&data_root().join(path));
    if !body.is_empty() {
        return body;
    }
    let safe = path.replace('/', "_");
    let cf = cache_dir().join(format!("{safe}.car"));
    if cf.exists() {
        return read_file(&cf);
    }
    let (body, _code) = http_get(&format!("{CAR_BASE_URL}{path}")).await;
    if !body.is_empty() {
        let _ = std::fs::write(&cf, &body);
    }
    body
}

fn cached_commit() -> String {
    read_file(&commit_path())
}
fn store_commit(sha: &str) {
    let _ = std::fs::write(commit_path(), sha);
}
async fn latest_commit() -> String {
    let (body, _) = http_get(COMMITS_URL).await;
    if body.is_empty() {
        return String::new();
    }
    serde_json::from_str::<Value>(&body)
        .ok()
        .and_then(|j| j.get("sha").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

fn extract_tar_gz(archive: &std::path::Path, dest: &std::path::Path) -> bool {
    let _ = std::fs::create_dir_all(dest);
    let f = match std::fs::File::open(archive) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let gz = flate2::read::GzDecoder::new(f);
    let mut ar = tar::Archive::new(gz);
    ar.unpack(dest).is_ok()
}

pub async fn load_manifest_from_cache_or_net(s: &mut State) {
    let mut body = read_file(&data_root().join("manifest.json"));
    if body.is_empty() {
        let (b, _) = http_get(MANIFEST_URL).await;
        body = b;
        if !body.is_empty() {
            let _ = std::fs::write(manifest_cache_path(), &body);
        }
    }
    if body.is_empty() {
        body = read_file(&manifest_cache_path());
    }
    parse_manifest(s, &body);
}

async fn download_database(ctx: &Arc<Ctx>) -> bool {
    ctx.ui_run(|u| {
        let cl = u.global::<CarLib>();
        cl.set_downloading(true);
        cl.set_download_progress(0.0);
        cl.set_status(sstr("Downloading car database…"));
    });
    let archive = cache_dir().join("lovely-car-data.tar.gz");
    let pc = ctx.clone();
    let mut ok = http_download_file(TARBALL_URL, &archive, move |frac| {
        pc.ui_run(move |u| {
            u.global::<CarLib>()
                .set_download_progress((frac * 0.9) as f32)
        });
    })
    .await;
    if ok {
        let _ = std::fs::remove_dir_all(db_dir());
        ok = extract_tar_gz(&archive, &db_dir());
        let _ = std::fs::remove_file(&archive);
    }
    ctx.ui_run(move |u| {
        let cl = u.global::<CarLib>();
        cl.set_download_progress(1.0);
        cl.set_downloading(false);
        cl.set_status(sstr(if ok {
            "Database ready"
        } else {
            "Download failed (offline?)"
        }));
    });
    ok && data_root().join("manifest.json").exists()
}

pub fn refresh_database(ctx: &Arc<Ctx>) {
    if ctx.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let ok = download_database(&ctx).await;
        let sha = latest_commit().await;
        if ok && !sha.is_empty() {
            store_commit(&sha);
        }
        let body = if ok {
            read_file(&data_root().join("manifest.json"))
        } else {
            String::new()
        };
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            if !body.is_empty() {
                parse_manifest(&mut s, &body);
                push_classes(&u, &mut s);
                rebuild_filtered(&mut s);
            }
            push_car_results(&u, &s);
            drop(s);
            if !body.is_empty() {
                prefetch_game_data(&ctx2);
            }
        });
        ctx.busy.store(false, Ordering::SeqCst);
    });
}

pub fn sync_database_if_stale(ctx: &Arc<Ctx>) {
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let have_local = data_root().join("manifest.json").exists();
        let latest = latest_commit().await;
        if have_local && (latest.is_empty() || latest == cached_commit()) {
            return;
        }
        if !download_database(&ctx).await {
            return;
        }
        if !latest.is_empty() {
            store_commit(&latest);
        }
        let body = read_file(&data_root().join("manifest.json"));
        if body.is_empty() {
            return;
        }
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            parse_manifest(&mut s, &body);
            push_classes(&u, &mut s);
            rebuild_filtered(&mut s);
            push_car_results(&u, &s);
            u.global::<CarLib>().set_status(sstr("database updated"));
            drop(s);
            prefetch_game_data(&ctx2);
        });
    });
}

pub fn prefetch_game_data(ctx: &Arc<Ctx>) {
    let gen_id = ctx.car_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let (work, _sim): (Vec<(usize, String)>, String) = {
        let s = ctx.lock();
        let sim = s.sim_of(s.game);
        let work = s
            .all_cars
            .iter()
            .enumerate()
            .filter(|(_, c)| c.sim == sim && c.redline == 0 && c.klass.is_empty())
            .map(|(i, c)| (i, c.path.clone()))
            .collect();
        (work, sim)
    };
    if work.is_empty() {
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            push_classes(&u, &mut s);
        });
        return;
    }
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let mut done = 0;
        for (idx, path) in work {
            if gen_id != ctx.car_gen.load(Ordering::SeqCst) {
                return;
            }
            let body = car_body_by_path(&path).await;
            let (mut rl, mut led_n, mut klass, mut cols) = (0, 0, String::new(), Vec::new());
            if let Ok(j) = serde_json::from_str::<Value>(&body) {
                klass = j
                    .get("carClass")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                led_n = j.get("ledNumber").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
                rl = derive_redline(&j);
                cols = parse_car_led_colors(&j);
            }
            if gen_id != ctx.car_gen.load(Ordering::SeqCst) {
                return;
            }
            {
                let mut s = ctx.lock();
                if idx < s.all_cars.len() {
                    s.all_cars[idx].redline = rl;
                    s.all_cars[idx].led_n = led_n;
                    s.all_cars[idx].klass = klass;
                    s.all_cars[idx].led_cols = cols;
                }
            }
            done += 1;
            if done % 8 == 0 {
                let ctx2 = ctx.clone();
                ctx.ui_run(move |u| {
                    if gen_id == ctx2.car_gen.load(Ordering::SeqCst) {
                        let mut s = ctx2.lock();
                        push_classes(&u, &mut s);
                        push_car_results(&u, &s);
                    }
                });
            }
        }
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            if gen_id == ctx2.car_gen.load(Ordering::SeqCst) {
                let mut s = ctx2.lock();
                push_classes(&u, &mut s);
                rebuild_filtered(&mut s);
                push_car_results(&u, &s);
            }
        });
    });
}

pub fn select_car(ctx: &Arc<Ctx>, filtered_idx: i32) {
    let idx = {
        let s = ctx.lock();
        if filtered_idx < 0 || filtered_idx as usize >= s.filtered.len() {
            return;
        }
        s.filtered[filtered_idx as usize]
    };
    ctx.ui_run(move |u| u.global::<CarLib>().set_sel(filtered_idx));
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let path = {
            let s = ctx.lock();
            s.all_cars[idx].path.clone()
        };
        let body = car_body_by_path(&path).await;
        let (mut redline, mut led_n, mut klass, mut cols) = (0, 0, String::new(), Vec::new());
        if let Ok(j) = serde_json::from_str::<Value>(&body) {
            klass = j
                .get("carClass")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            led_n = j.get("ledNumber").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
            redline = derive_redline(&j);
            cols = parse_car_led_colors(&j);
        }
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            if idx < s.all_cars.len() {
                s.all_cars[idx].redline = redline;
                s.all_cars[idx].led_n = led_n;
                s.all_cars[idx].klass = klass;
                s.all_cars[idx].led_cols = cols;
            }
            push_car_results(&u, &s);
            u.global::<CarLib>().set_sel(filtered_idx);
        });
    });
}

pub fn set_active_car(ctx: &Arc<Ctx>, filtered_idx: i32) {
    let car = {
        let s = ctx.lock();
        if filtered_idx < 0 || filtered_idx as usize >= s.filtered.len() {
            return;
        }
        s.all_cars[s.filtered[filtered_idx as usize]].clone()
    };
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let body = car_body_by_path(&car.path).await;
        let mut minified = String::new();
        let mut redline = car.redline;
        let mut status = String::new();
        match serde_json::from_str::<Value>(&body) {
            Ok(j) => {
                minified = serde_json::to_string(&j).unwrap_or_default();
                if redline == 0 {
                    redline = derive_redline(&j);
                }
            }
            Err(_) => status = "car parse error".to_string(),
        }
        let connected = ctx.dash().connected();
        if !minified.is_empty() && connected {
            let ok = ctx.dash().push_car(&minified);
            status = if ok {
                format!("Sent {}", car.name)
            } else {
                "device rejected".to_string()
            };
        } else if !connected {
            status = "sent (offline cache) · connect to push".to_string();
        }
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            s.car_name = car.name.clone();
            s.car_game = car.sim.clone();
            s.car_id = car.id.clone();
            if redline > 0 {
                s.redline_rpm = redline;
                s.shift_custom = false;
            }
            if let Ok(j) = serde_json::from_str::<Value>(&body) {
                load_car_into_leds(&mut s, &j);
            }
            push_shift_scalars(&u, &s);
            push_led_model(&u, &s);
            push_car_results(&u, &s);
            crate::persist::save_active_car(&s);
            u.global::<CarLib>().set_status(sstr(&status));
        });
    });
}

pub fn auto_apply_car_model(ctx: &Arc<Ctx>, s: &mut State, model: &str) {
    if model.is_empty() || model == s.last_auto_model {
        return;
    }
    s.last_auto_model = model.to_string();
    let sim = s.sim_of(s.game);
    // Match on the bare model — drop entry/livery suffixes like "#23:WEC" (rF2/LMU).
    let nm = norm_name(crate::util::clean_car_name(model));
    if nm.is_empty() {
        return;
    }
    let mut best: i32 = -1;
    let mut best_score: usize = 0;
    for (i, c) in s.all_cars.iter().enumerate() {
        if c.sim != sim {
            continue;
        }
        let mut score = 0usize;
        for cand in [norm_name(&c.name), norm_name(&c.id)] {
            if cand.is_empty() {
                continue;
            }
            if cand == nm {
                score = score.max(1000);
            } else if cand.contains(&nm) {
                score = score.max(nm.len());
            } else if nm.contains(&cand) {
                score = score.max(cand.len());
            }
        }
        if score > best_score {
            best_score = score;
            best = i as i32;
        }
    }
    let model_owned = model.to_string();
    if best < 0 {
        ctx.ui_run(move |u| {
            u.global::<CarLib>()
                .set_status(sstr(&format!("No library match for '{model_owned}'")));
        });
        return;
    }
    let best = best as usize;
    let car = s.all_cars[best].clone();
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let body = car_body_by_path(&car.path).await;
        let mut minified = String::new();
        let mut redline = car.redline;
        if let Ok(j) = serde_json::from_str::<Value>(&body) {
            minified = serde_json::to_string(&j).unwrap_or_default();
            if redline == 0 {
                redline = derive_redline(&j);
            }
        }
        let ok = !minified.is_empty() && ctx.dash().connected() && ctx.dash().push_car(&minified);
        let ctx2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = ctx2.lock();
            if redline > 0 {
                s.redline_rpm = redline;
                s.shift_custom = false;
            }
            s.car_name = car.name.clone();
            s.car_game = car.sim.clone();
            s.car_id = car.id.clone();
            if let Ok(j) = serde_json::from_str::<Value>(&body) {
                load_car_into_leds(&mut s, &j);
            }
            if let Some(fi) = s.filtered.iter().position(|&x| x == best) {
                u.global::<CarLib>().set_sel(fi as i32);
            }
            push_shift_scalars(&u, &s);
            push_led_model(&u, &s);
            push_car_results(&u, &s);
            crate::persist::save_active_car(&s);
            u.global::<CarLib>().set_status(sstr(&if ok {
                format!("Auto: {}", car.name)
            } else {
                format!("Matched {} (offline)", car.name)
            }));
        });
    });
}
