use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::ctx::Ctx;
use crate::games::detect_game;
use crate::net::cardata::{auto_apply_car_model, prefetch_game_data};
use crate::persist::{race_layout_from_json, save_race_layout};
use crate::ui_bridge::cars::{push_car_results, push_classes, rebuild_filtered};
use crate::ui_bridge::firmware::{recompute_update_available, refresh_serial_ports};
use crate::ui_bridge::telemetry::{apply_caps, apply_status, apply_telemetry};
use crate::ui_bridge::{model, refresh_race, sstr};
use crate::{AppState, CarLib, Firmware, FwComponent, RaceLayout, Telemetry};

pub fn try_connect(ctx: &Arc<Ctx>) -> bool {
    let mut d = ctx.dash();
    if d.hid.open(0x303A, 0x4002) {
        d.use_hid = true;
        if d.capabilities().contains("name") {
            return true;
        }
        d.hid.close();
    }
    d.use_hid = true;
    false
}

pub fn dash_close(ctx: &Arc<Ctx>) {
    let mut d = ctx.dash();
    d.hid.close();
    d.ser.close();
    d.use_hid = false;
}

/// TCP receiver for the SimHub plugin (Phase 2): accept `$`-frames on
/// 127.0.0.1:28909 and forward each to the device over HID. The device parses
/// them into TELEM exactly like the old Custom Serial feed; the existing @T poll
/// then mirrors that back into the dashboard preview, so the screen stays the
/// single source of truth. Idle-cheap: non-blocking accept, blocking line reads.
pub fn sim_listener_loop(ctx: Arc<Ctx>) {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    let listener = loop {
        if !ctx.running.load(Ordering::SeqCst) {
            return;
        }
        match TcpListener::bind(("127.0.0.1", 28909)) {
            Ok(l) => break l,
            Err(_) => std::thread::sleep(Duration::from_secs(2)),
        }
    };
    let _ = listener.set_nonblocking(true);
    let mut last_push = std::time::Instant::now();
    let mut last_preview = std::time::Instant::now();

    while ctx.running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let reader = BufReader::new(stream);
                for line in reader.lines() {
                    if !ctx.running.load(Ordering::SeqCst) {
                        return;
                    }
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => break, // read timeout / disconnect: drop the client
                    };
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Some(model) = line.strip_prefix("@CM") {
                        // Car model → show it as the detected car (unconditionally — the
                        // @CM frame only arrives when SimHub has a car), match the
                        // library, and push the per-car LED profile + shift scalars.
                        let model = model.trim().to_string();
                        if !model.is_empty() {
                            {
                                let mut s = ctx.lock();
                                s.detected_model = model.clone();
                                crate::net::cardata::auto_apply_car_model(&ctx, &mut s, &model);
                            }
                            let label = model.clone();
                            ctx.ui_run(move |u| {
                                u.global::<CarLib>().set_detected_car(sstr(&label));
                            });
                        }
                    } else if let Some(track) = line.strip_prefix("@MAP") {
                        // New track → forget the old learned outline and relearn.
                        let track = track.trim().to_string();
                        let mut s = ctx.lock();
                        if s.map_learner.track != track {
                            s.map_learner.reset(&track);
                            s.learned_map.clear();
                            s.map_pushed = false;
                            s.map_push_pending = false;
                        }
                    } else if line.starts_with('$') && !ctx.ota_active.load(Ordering::SeqCst) {
                        ctx.lock().last_sim_frame = Some(std::time::Instant::now());
                        let now = std::time::Instant::now();
                        // Push to the device (~30 Hz — smooth on the LCD, half the HID
                        // traffic of 60, and no @T round-trip back from the device).
                        if now.duration_since(last_push) >= Duration::from_millis(33) {
                            last_push = now;
                            let mut d = ctx.dash();
                            if d.connected() {
                                d.push_telemetry(line);
                            }
                        }
                        // Feed the dashboard's OWN overview directly (~12 Hz) — much
                        // smoother than the 6 Hz device_loop, capped so the rendered
                        // preview image doesn't hog the UI thread.
                        if now.duration_since(last_preview) >= Duration::from_millis(80) {
                            last_preview = now;
                            let frame = line[1..].to_string();
                            let c2 = ctx.clone();
                            ctx.ui_run(move |u| {
                                let mut s = c2.lock();
                                u.global::<Telemetry>().set_connected(true);
                                apply_telemetry(&u, &mut s, &frame);
                            });
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(500)),
        }
    }
}

pub fn device_loop(ctx: Arc<Ctx>) {
    let mut fw_tick: u32 = 0;
    while ctx.running.load(Ordering::SeqCst) {
        // Re-scan for a locally built firmware image every ~2s so a bin built from
        // the terminal (just image) lights up the "FLASH LOCAL BUILD" button live.
        if fw_tick % 12 == 0 {
            let c2 = ctx.clone();
            ctx.ui_run(move |u| {
                let s = c2.lock();
                crate::ui_bridge::firmware::refresh_firmware_local(&u, &s);
            });
        }
        fw_tick = fw_tick.wrapping_add(1);
        if !ctx.dash().connected() {
            if try_connect(&ctx) {
                // Freshly (re)connected device: its LED car profile is RAM-only and
                // lost on reboot. Forget the last auto-applied car so the plugin's
                // next @CM heartbeat re-pushes the live @C and the LEDs light again.
                ctx.lock().last_auto_model.clear();
                let caps = ctx.dash().capabilities();
                let st = ctx.dash().status();
                let c2 = ctx.clone();
                ctx.ui_run(move |u| {
                    let mut s = c2.lock();
                    apply_caps(&u, &mut s, &caps);
                    let app = u.global::<AppState>();
                    app.set_connected(true);
                    app.set_conn_detail(sstr("Connected · HID (SimHub-safe)"));
                    app.set_health_pct(82);
                    if !st.is_empty() {
                        apply_status(&u, &mut s, &c2, &st);
                    }
                });
                // On connect, pull the device's saved layout so the editor shows what's
                // actually on the device — unless there are unsaved local edits to keep.
                if !ctx.lock().race_dirty {
                    read_race_from_device(&ctx);
                }
            } else {
                std::thread::sleep(Duration::from_millis(1500));
                continue;
            }
        }
        if ctx.ota_active.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(120));
            continue;
        }
        let st = ctx.dash().status();
        if st.is_empty() {
            dash_close(&ctx);
            let c2 = ctx.clone();
            ctx.ui_run(move |u| {
                let mut s = c2.lock();
                let app = u.global::<AppState>();
                app.set_connected(false);
                app.set_conn_detail(sstr("Disconnected"));
                app.set_health_pct(0);
                u.global::<Telemetry>().set_connected(false);
                let fw = u.global::<Firmware>();
                fw.set_current(sstr("—"));
                fw.set_components(model(Vec::<FwComponent>::new()));
                s.device_fw.clear();
                recompute_update_available(&u, &s);
                refresh_serial_ports(&u, &mut s);
            });
        } else {
            // If the SimHub plugin is feeding us (recent frame), drive our OWN view
            // from that frame and SKIP the @T round-trip — the app already has the
            // data, so polling it back from the device is pure waste + contention.
            let feeding = ctx
                .lock()
                .last_sim_frame
                .map_or(false, |t| t.elapsed() < Duration::from_millis(1500));
            // When the plugin feeds us, the sim_listener already drives the overview
            // (~12 Hz) AND pushes to the device — so here we just skip the @T poll.
            // Only poll @T for telemetry when there's no plugin feeding the app.
            if !feeding {
                let tl = ctx.dash().telemetry();
                let c2 = ctx.clone();
                ctx.ui_run(move |u| {
                    let mut s = c2.lock();
                    apply_status(&u, &mut s, &c2, &st);
                    if !tl.is_empty() {
                        apply_telemetry(&u, &mut s, &tl);
                    }
                });
            }
        }
        // Push a freshly self-learned track map (this background thread can block
        // on the reply). Skipped while the user has unsaved edits so an auto-push
        // never clobbers in-progress layout work — it then rides out on Save.
        if ctx.dash().connected() {
            let json = {
                let mut s = ctx.lock();
                if s.map_push_pending && !s.race_dirty {
                    s.map_push_pending = false;
                    Some(crate::ui_bridge::uidoc::build_uidoc_json(&s))
                } else {
                    None
                }
            };
            if let Some(json) = json {
                ctx.dash().push_ui(&json);
            }
        }
        // Stream firmware logs (HID report id 3) into the GUI's device-log view.
        let new_logs = ctx.dash().take_device_logs();
        if !new_logs.is_empty() {
            let c2 = ctx.clone();
            ctx.ui_run(move |u| {
                let mut s = c2.lock();
                s.device_log.extend(new_logs.iter().cloned());
                let len = s.device_log.len();
                if len > 2000 {
                    s.device_log.drain(..len - 2000);
                }
                crate::ui_bridge::push_device_log(&u, &s);
            });
        }
        std::thread::sleep(Duration::from_millis(160));
    }
}

pub fn game_loop(ctx: Arc<Ctx>) {
    let sims = ctx.lock().sims.clone();
    let mut last = -2;
    while ctx.running.load(Ordering::SeqCst) {
        let gi = detect_game(&sims);
        if gi != last {
            last = gi;
            let c2 = ctx.clone();
            ctx.ui_run(move |u| {
                let detected_model;
                let do_prefetch;
                {
                    let mut s = c2.lock();
                    // When SimHub is actively feeding us it's authoritative for the
                    // game + car — don't let a flaky process scan (especially under
                    // Wine) wipe the detected game/car SimHub provided.
                    let feeding = s
                        .last_sim_frame
                        .map_or(false, |t| t.elapsed() < std::time::Duration::from_millis(3000));
                    if gi < 0 && feeding {
                        return;
                    }
                    s.detected_game_idx = gi;
                    let cl = u.global::<CarLib>();
                    let dg = if gi >= 0 {
                        s.sims[gi as usize].0.clone()
                    } else {
                        String::new()
                    };
                    cl.set_detected_game(sstr(&dg));
                    if gi < 0 {
                        cl.set_detected_car(sstr(""));
                        s.detected_model.clear();
                        s.last_auto_model.clear();
                        do_prefetch = false;
                        detected_model = String::new();
                    } else if gi != s.game {
                        s.game = gi;
                        s.klass = 0;
                        s.sel_car = -1;
                        cl.set_game(gi);
                        cl.set_klass(0);
                        cl.set_sel(-1);
                        push_classes(&u, &mut s);
                        rebuild_filtered(&mut s);
                        push_car_results(&u, &s);
                        s.last_auto_model.clear();
                        detected_model = s.detected_model.clone();
                        do_prefetch = true;
                    } else {
                        do_prefetch = false;
                        detected_model = String::new();
                    }
                }
                if do_prefetch {
                    prefetch_game_data(&c2);
                    if !detected_model.is_empty() {
                        let mut s = c2.lock();
                        auto_apply_car_model(&c2, &mut s, &detected_model);
                    }
                }
            });
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

pub fn sync_from_device(ctx: &Arc<Ctx>) {
    let connected = ctx.dash().connected();
    if !connected {
        ctx.ui_run(|u| {
            u.global::<AppState>()
                .set_sync_status(sstr("Not connected"))
        });
        return;
    }
    ctx.ui_run(|u| {
        u.global::<AppState>()
            .set_sync_status(sstr("Syncing from device…"))
    });
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let caps = ctx.dash().capabilities();
        let (ok, reply) = ctx.dash().command("@RG");
        let c2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = c2.lock();
            if !caps.is_empty() {
                apply_caps(&u, &mut s, &caps);
            }
            let mut got_layout = false;
            if ok {
                if let Some(b) = reply.find('{') {
                    if let Ok(j) = serde_json::from_str::<serde_json::Value>(&reply[b..]) {
                        if j.get("mods")
                            .map(|m| !m.as_array().map(|a| a.is_empty()).unwrap_or(true))
                            .unwrap_or(false)
                        {
                            race_layout_from_json(&mut s, &j);
                            s.race_dirty = false;
                            u.global::<RaceLayout>().set_dirty(false);
                            refresh_race(&u, &s);
                            save_race_layout(&s);
                            got_layout = true;
                        }
                    }
                }
            }
            u.global::<AppState>().set_sync_status(sstr(if got_layout {
                "Synced from device"
            } else {
                "Synced — device has no saved layout"
            }));
        });
    });
}

pub fn read_race_from_device(ctx: &Arc<Ctx>) {
    let connected = ctx.dash().connected();
    if !connected {
        ctx.ui_run(|u| {
            u.global::<RaceLayout>()
                .set_save_status(sstr("Not connected"))
        });
        return;
    }
    ctx.ui_run(|u| {
        u.global::<RaceLayout>()
            .set_save_status(sstr("Reading from device…"))
    });
    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        // Prefer the full editor blob (@EG) — a lossless round-trip of the freeform
        // layout. Fall back to the legacy zone layout (@RG) on older firmware.
        let ed = ctx.dash().read_editor();
        let rg = ctx.dash().command("@RG").1;
        let c2 = ctx.clone();
        ctx.ui_run(move |u| {
            let mut s = c2.lock();
            let rl = u.global::<RaceLayout>();
            if let Some(b) = ed.find('{') {
                if let Ok(j) = serde_json::from_str::<serde_json::Value>(&ed[b..]) {
                    if crate::persist::apply_editor_layout_json(&mut s, &j) {
                        s.race_dirty = false;
                        rl.set_dirty(false);
                        refresh_race(&u, &s);
                        save_race_layout(&s);
                        rl.set_save_status(sstr("Loaded from device"));
                        return;
                    }
                }
            }
            if let Some(b) = rg.find('{') {
                if let Ok(j) = serde_json::from_str::<serde_json::Value>(&rg[b..]) {
                    if j.get("mods")
                        .map(|m| !m.as_array().map(|a| a.is_empty()).unwrap_or(true))
                        .unwrap_or(false)
                    {
                        race_layout_from_json(&mut s, &j);
                        s.race_dirty = false;
                        rl.set_dirty(false);
                        refresh_race(&u, &s);
                        save_race_layout(&s);
                        rl.set_save_status(sstr("Loaded from device (legacy)"));
                        return;
                    }
                }
            }
            rl.set_save_status(sstr("Device has no saved layout"));
        });
    });
}
