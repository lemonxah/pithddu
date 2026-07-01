use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::device::Dash;
use crate::state::State;
use crate::AppWindow;

pub struct Ctx {
    pub ui: slint::Weak<AppWindow>,
    pub state: Arc<Mutex<State>>,
    pub dash: Arc<Mutex<Dash>>,
    pub rt: tokio::runtime::Handle,
    pub running: Arc<AtomicBool>,
    pub ota_active: Arc<AtomicBool>,
    /// GUI "Simulate" toggle — when set, sim_loop streams a full animated test
    /// telemetry feed (every field) + cycles car shift-light profiles to the device.
    pub sim_active: Arc<AtomicBool>,
    pub busy: Arc<AtomicBool>,
    pub car_gen: Arc<std::sync::atomic::AtomicUsize>,
    pub build_cancel: Arc<AtomicBool>,
    pub build_pgid: Arc<std::sync::atomic::AtomicI32>,
    pub tray_active: Arc<AtomicBool>,
}

impl Ctx {
    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }
    pub fn dash(&self) -> MutexGuard<'_, Dash> {
        self.dash.lock().unwrap()
    }

    pub fn ui_run<F: FnOnce(AppWindow) + Send + 'static>(&self, f: F) {
        let w = self.ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(u) = w.upgrade() {
                f(u);
            }
        });
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        self.rt.spawn(fut);
    }
}
