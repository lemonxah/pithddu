use slint::ComponentHandle;
use slint::SharedString;

use super::{model, sstr};
use crate::device::Serial;
use crate::firmware::{can_build_firmware, semver_cmp, APP_FW_VERSION};
use crate::paths::{default_firmware_path, file_size_str};
use crate::state::State;
use crate::{AppState, AppWindow, Firmware, FwState};

pub fn refresh_firmware_local(ui: &AppWindow, _s: &State) {
    let fw = ui.global::<Firmware>();
    let bin = default_firmware_path();
    fw.set_can_build(can_build_firmware());
    fw.set_have_bin(bin.is_some());
    if let Some(b) = bin {
        fw.set_bin_path(sstr(&b));
        fw.set_size(sstr(&file_size_str(std::path::Path::new(&b))));
    }
}

pub fn refresh_serial_ports(ui: &AppWindow, s: &mut State) {
    s.serial_ports = Serial::list();
    let fw = ui.global::<Firmware>();
    let mut labels: Vec<SharedString> = Vec::new();
    let mut esp_count = 0;
    let mut first_esp: i32 = -1;
    for (i, p) in s.serial_ports.iter().enumerate() {
        let mut desc = p.manufacturer.clone();
        if !p.product.is_empty() {
            if !desc.is_empty() {
                desc.push_str(" · ");
            }
            desc.push_str(&p.product);
        }
        if desc.is_empty() {
            desc = if p.is_dash {
                "Pith DDU".into()
            } else {
                "unknown device".into()
            };
        }
        let mut lab = format!("{}  ·  {}", p.device, desc);
        if p.is_esp {
            lab.push_str("  [ESP]");
            esp_count += 1;
            if first_esp < 0 {
                first_esp = i as i32;
            }
        }
        labels.push(sstr(&lab));
    }
    fw.set_serial_ports(model(labels));
    fw.set_esp_port_count(esp_count);
    let mut sel = fw.get_serial_port();
    if sel < 0 || sel as usize >= s.serial_ports.len() {
        sel = if first_esp >= 0 { first_esp } else { 0 };
    }
    fw.set_serial_port(sel);
}

pub fn recompute_update_available(ui: &AppWindow, s: &State) {
    let fw = ui.global::<Firmware>();
    let latest = if s.releases.is_empty() {
        APP_FW_VERSION.to_string()
    } else {
        s.releases[0].tag.clone()
    };
    fw.set_latest(sstr(&if latest.starts_with('v') {
        latest.clone()
    } else {
        format!("v{latest}")
    }));
    let conn = ui.global::<AppState>().get_connected();
    let upd = conn && !s.device_fw.is_empty() && semver_cmp(&latest, &s.device_fw) > 0;
    fw.set_update_available(upd);
    if upd && fw.get_state() == FwState::Success {
        fw.set_state(FwState::Idle);
    }
    fw.set_latest_has_board(!s.releases.is_empty() && s.release_has_image(&s.releases[0]));
}

pub fn update_release_board_match(ui: &AppWindow, s: &State) {
    let fw = ui.global::<Firmware>();
    let i = fw.get_sel_release();
    fw.set_release_has_board(
        i >= 0 && (i as usize) < s.releases.len() && s.release_has_image(&s.releases[i as usize]),
    );
    fw.set_latest_has_board(!s.releases.is_empty() && s.release_has_image(&s.releases[0]));
}
