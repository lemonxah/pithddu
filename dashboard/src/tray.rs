#[cfg(target_os = "linux")]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    use slint::ComponentHandle;

    use crate::AppWindow;

    static VISIBLE: AtomicBool = AtomicBool::new(true);
    static HANDLE: OnceLock<Mutex<Option<ksni::Handle<PithTray>>>> = OnceLock::new();

    pub fn show_window(ui: &slint::Weak<AppWindow>) {
        VISIBLE.store(true, Ordering::SeqCst);
        let w = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(u) = w.upgrade() {
                let _ = u.window().show();
            }
        });
        set_window_visible(true);
    }

    pub fn hide_window(ui: &slint::Weak<AppWindow>) {
        VISIBLE.store(false, Ordering::SeqCst);
        let w = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(u) = w.upgrade() {
                let _ = u.window().hide();
            }
        });
        set_window_visible(false);
    }

    pub fn set_window_visible(visible: bool) {
        VISIBLE.store(visible, Ordering::SeqCst);
    }

    struct PithTray {
        ui: slint::Weak<AppWindow>,
    }

    impl ksni::Tray for PithTray {
        fn id(&self) -> String {
            "pith-dashboard".into()
        }
        fn title(&self) -> String {
            "Pith Dashboard".into()
        }
        fn icon_name(&self) -> String {
            "pith-dashboard".into()
        }
        fn activate(&mut self, _x: i32, _y: i32) {
            show_window(&self.ui);
        }
        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::StandardItem;
            let visible = VISIBLE.load(Ordering::SeqCst);
            vec![
                StandardItem {
                    label: if visible {
                        "Hide window".into()
                    } else {
                        "Show window".into()
                    },
                    activate: Box::new(|t: &mut PithTray| {
                        if VISIBLE.load(Ordering::SeqCst) {
                            hide_window(&t.ui);
                        } else {
                            show_window(&t.ui);
                        }
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Quit".into(),
                    activate: Box::new(|_t: &mut PithTray| {
                        let _ = slint::invoke_from_event_loop(|| {
                            let _ = slint::quit_event_loop();
                        });
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }

    pub async fn start(ui: slint::Weak<AppWindow>) -> bool {
        use ksni::TrayMethods;
        let tray = PithTray { ui };
        match tray.spawn().await {
            Ok(handle) => {
                let _ = HANDLE.set(Mutex::new(Some(handle)));
                true
            }
            Err(_) => false,
        }
    }
}

#[cfg(target_os = "linux")]
#[allow(unused_imports)]
pub use imp::{hide_window, set_window_visible, show_window, start};

#[cfg(not(target_os = "linux"))]
mod imp {
    use slint::ComponentHandle;

    use crate::AppWindow;
    pub fn show_window(ui: &slint::Weak<AppWindow>) {
        let w = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(u) = w.upgrade() {
                let _ = u.window().show();
            }
        });
    }
    pub fn hide_window(ui: &slint::Weak<AppWindow>) {
        let w = ui.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(u) = w.upgrade() {
                let _ = u.window().hide();
            }
        });
    }
    pub fn set_window_visible(_visible: bool) {}
    pub async fn start(_ui: slint::Weak<AppWindow>) -> bool {
        false
    }
}

#[cfg(not(target_os = "linux"))]
#[allow(unused_imports)]
pub use imp::{hide_window, set_window_visible, show_window, start};
