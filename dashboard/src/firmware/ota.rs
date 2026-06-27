use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::ctx::Ctx;
use crate::ui_bridge::sstr;
use crate::{Firmware, FwState};

pub fn start_ota_from_bin_path(ctx: &Arc<Ctx>) {
    let (path, connected) = {
        let p = ctx
            .ui
            .upgrade()
            .map(|u| u.global::<Firmware>().get_bin_path().to_string());
        (p.unwrap_or_default(), ctx.dash().connected())
    };
    if !connected {
        ctx.ui_run(|u| {
            u.global::<Firmware>()
                .set_status_line(sstr("Not connected"))
        });
        return;
    }
    if path.is_empty() {
        ctx.ui_run(|u| {
            u.global::<Firmware>()
                .set_status_line(sstr("No firmware .bin selected"))
        });
        return;
    }
    ctx.ui_run(|u| {
        let fw = u.global::<Firmware>();
        fw.set_state(FwState::Flashing);
        fw.set_progress(0.0);
        fw.set_status_line(sstr("Reading firmware..."));
    });

    let ctx = ctx.clone();
    ctx.clone().spawn(async move {
        let img = match std::fs::read(&path) {
            Ok(b) if !b.is_empty() => b,
            _ => {
                ctx.ui_run(|u| {
                    let fw = u.global::<Firmware>();
                    fw.set_state(FwState::Failure);
                    fw.set_status_line(sstr("Cannot read .bin file"));
                });
                return;
            }
        };
        ctx.ui_run(|u| {
            u.global::<Firmware>()
                .set_status_line(sstr("Uploading over USB..."))
        });
        ctx.ota_active.store(true, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let ok = {
            let cb_ctx = ctx.clone();
            let mut dash = ctx.dash();
            dash.ota_upload(&img, move |pct| {
                cb_ctx.ui_run(move |u| u.global::<Firmware>().set_progress(pct as f32 / 100.0));
            })
        };
        ctx.ota_active.store(false, Ordering::SeqCst);
        ctx.ui_run(move |u| {
            let fw = u.global::<Firmware>();
            fw.set_state(if ok {
                FwState::Success
            } else {
                FwState::Failure
            });
            fw.set_progress(if ok { 1.0 } else { 0.0 });
            fw.set_status_line(sstr(if ok {
                "Updated — device rebooting"
            } else {
                "Update failed (port busy?)"
            }));
        });
    });
}
