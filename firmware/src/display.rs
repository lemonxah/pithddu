//! Dual ST7796 SPI displays + XPT2046 touch on the shared SPI2 bus, and the
//! render/interaction loop. One panel shows the @RS race screen (or the config
//! screen), the other the @BS button box. Drawing is direct to the mipidsi
//! displays (double-buffering is a later refinement). Touch drives HID buttons,
//! page changes, brightness, sim toggle and reboot.
//!
//! Hardware constraints mirrored from the legacy firmware: a single shared DC
//! pin across both panels (wrapped in Rc<RefCell> since the task is single-
//! threaded), and pushing display 2 before display 1 on the shared bus.

use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{AnyIOPin, Output, PinDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::spi::{
    config::Config as SpiConfig, Dma, SpiDeviceDriver, SpiDriver, SpiDriverConfig,
};
use esp_idf_svc::hal::units::FromValueType;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7796;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use mipidsi::Builder;

use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, Pixel, RgbColor, Size};
use embedded_graphics::primitives::Rectangle;

use crate::{hid, ota, state, ui, usb};

/// An in-RAM RGB565 framebuffer (lives in PSRAM). The whole UI is drawn here —
/// fonts and fills hit RAM instead of issuing a windowed SPI write per glyph
/// pixel — then the full buffer is streamed to the panel in one DMA blit. This
/// is what makes redraws fast (direct-to-SPI font drawing was ~1 s/frame).
struct FrameBuf {
    data: Vec<Rgb565>,
    w: i32,
    h: i32,
}

impl FrameBuf {
    fn new(w: i32, h: i32) -> Self {
        Self { data: vec![Rgb565::BLACK; (w * h) as usize], w, h }
    }
    #[inline]
    fn put(&mut self, x: i32, y: i32, c: Rgb565) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.data[(y * self.w + x) as usize] = c;
        }
    }

    /// Diagnostic touch indicator: a translucent dot + white crosshair at (cx,cy)
    /// plus a ring that expands and fades as `fade` drops from `fade_max` to 0
    /// (i.e. after release). Lets you eyeball whether the touch X/Y matches your
    /// finger. RGB565 has no alpha, so colours are blended by hand.
    fn draw_touch_marker(&mut self, cx: i32, cy: i32, fade: u8, fade_max: u8) {
        #[inline]
        fn blend(bg: Rgb565, fg: Rgb565, a: u16) -> Rgb565 {
            let inv = 256 - a;
            let r = ((bg.r() as u16 * inv + fg.r() as u16 * a) >> 8) as u8;
            let g = ((bg.g() as u16 * inv + fg.g() as u16 * a) >> 8) as u8;
            let b = ((bg.b() as u16 * inv + fg.b() as u16 * a) >> 8) as u8;
            Rgb565::new(r, g, b)
        }
        let fade_max = fade_max.max(1) as i32;
        let f = fade as i32; // fade_max while held -> 0 when gone
        let frac = (f * 256 / fade_max).clamp(0, 256) as u16; // overall opacity
        let accent = Rgb565::new(0, 63, 31); // bright cyan
        let white = Rgb565::WHITE;

        // translucent filled dot
        let r_dot: i32 = 9;
        let dot_a = (frac * 120 / 256).min(255);
        for dy in -r_dot..=r_dot {
            for dx in -r_dot..=r_dot {
                if dx * dx + dy * dy <= r_dot * r_dot {
                    let (x, y) = (cx + dx, cy + dy);
                    if x >= 0 && y >= 0 && x < self.w && y < self.h {
                        let idx = (y * self.w + x) as usize;
                        self.data[idx] = blend(self.data[idx], accent, dot_a);
                    }
                }
            }
        }
        // white crosshair marking the exact pixel
        for k in -(r_dot + 3)..=(r_dot + 3) {
            self.put(cx + k, cy, white);
            self.put(cx, cy + k, white);
        }
        // ring that grows + fades after release
        let grow = (fade_max - f) * 3;
        let rr = r_dot + 4 + grow;
        let ring_a = frac.min(255);
        let (r0, r1) = ((rr - 1) * (rr - 1), (rr + 1) * (rr + 1));
        for dy in -(rr + 1)..=(rr + 1) {
            for dx in -(rr + 1)..=(rr + 1) {
                let d2 = dx * dx + dy * dy;
                if d2 >= r0 && d2 <= r1 {
                    let (x, y) = (cx + dx, cy + dy);
                    if x >= 0 && y >= 0 && x < self.w && y < self.h {
                        let idx = (y * self.w + x) as usize;
                        self.data[idx] = blend(self.data[idx], accent, ring_a);
                    }
                }
            }
        }
    }

    /// Dedicated TOUCH_DIAG screen: a cheap full-clear + reference grid + orange
    /// corner/centre crosses + the live touch marker. No widget rendering, so it's
    /// fast (the overlay-on-live-UI approach forced a full repaint each frame).
    /// Touch a known reference and see whether the marker lands on it.
    fn render_touch_test(&mut self, mark: Option<(i32, i32)>, fade: u8, fade_max: u8) {
        let bg = Rgb565::new(2, 4, 6);
        for p in self.data.iter_mut() {
            *p = bg;
        }
        let grid = Rgb565::new(5, 10, 16);
        let mut gx = 0;
        while gx < self.w {
            for y in 0..self.h {
                self.put(gx, y, grid);
            }
            gx += 60;
        }
        let mut gy = 0;
        while gy < self.h {
            for x in 0..self.w {
                self.put(x, gy, grid);
            }
            gy += 60;
        }
        // orange reference crosses at the four corners + centre
        let oc = Rgb565::new(31, 24, 0);
        let refs = [
            (8, 8),
            (self.w - 8, 8),
            (8, self.h - 8),
            (self.w - 8, self.h - 8),
            (self.w / 2, self.h / 2),
        ];
        for (rx, ry) in refs {
            for k in -7..=7 {
                self.put(rx + k, ry, oc);
                self.put(rx, ry + k, oc);
            }
        }
        if let Some((mx, my)) = mark {
            if fade > 0 {
                self.draw_touch_marker(mx, my, fade, fade_max);
            }
        }
    }
}

/// Stream a whole framebuffer to a panel in one windowed write (DMA). A macro
/// (not a generic fn) sidesteps naming the display's concrete reset-pin type.
macro_rules! blit {
    ($disp:expr, $fb:expr) => {{
        let _ = $disp.set_pixels(
            0,
            0,
            (ui::W - 1) as u16,
            (ui::H - 1) as u16,
            $fb.data.iter().copied(),
        );
    }};
}

impl OriginDimensions for FrameBuf {
    fn size(&self) -> Size {
        Size::new(self.w as u32, self.h as u32)
    }
}

impl DrawTarget for FrameBuf {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Rgb565>>,
    {
        for Pixel(p, c) in pixels {
            self.put(p.x, p.y, c);
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Rgb565>,
    {
        let mut it = colors.into_iter();
        for y in area.top_left.y..area.top_left.y + area.size.height as i32 {
            for x in area.top_left.x..area.top_left.x + area.size.width as i32 {
                match it.next() {
                    Some(c) => self.put(x, y, c),
                    None => return Ok(()),
                }
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Rgb565) -> Result<(), Self::Error> {
        let x0 = area.top_left.x.max(0);
        let y0 = area.top_left.y.max(0);
        let x1 = (area.top_left.x + area.size.width as i32).min(self.w);
        let y1 = (area.top_left.y + area.size.height as i32).min(self.h);
        for y in y0..y1 {
            let row = (y * self.w) as usize;
            for x in x0..x1 {
                self.data[row + x as usize] = color;
            }
        }
        Ok(())
    }

    fn clear(&mut self, color: Rgb565) -> Result<(), Self::Error> {
        self.data.iter_mut().for_each(|p| *p = color);
        Ok(())
    }
}

// XPT2046 calibration (from the legacy lgfx setup): Y is inverted.
const X_MIN: i32 = 300;
const X_MAX: i32 = 3900;
const Y_MIN: i32 = 3900;
const Y_MAX: i32 = 300;
const Z_THRESH: u16 = 400;

/// Shared DC pin (one GPIO drives both panels). The display task is single-
/// threaded, so Rc<RefCell> is sufficient.
#[derive(Clone)]
struct SharedDc(Rc<RefCell<PinDriver<'static, AnyIOPin, Output>>>);
impl embedded_hal::digital::ErrorType for SharedDc {
    type Error = core::convert::Infallible; // DC toggles can't meaningfully fail
}
impl embedded_hal::digital::OutputPin for SharedDc {
    fn set_low(&mut self) -> Result<(), core::convert::Infallible> {
        let _ = self.0.borrow_mut().set_low();
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), core::convert::Infallible> {
        let _ = self.0.borrow_mut().set_high();
        Ok(())
    }
}

fn xpt_read<S: embedded_hal::spi::SpiDevice>(dev: &mut S, cmd: u8) -> u16 {
    let mut buf = [cmd, 0, 0];
    let _ = dev.transfer_in_place(&mut buf);
    // 12-bit result sits in bits [14:3] after a leading busy bit. The old code
    // forgot the 0x0FFF mask, so the busy bit leaked in as +4096 — that alone
    // pushed every z1 read over Z_THRESH (or corrupted X/Y), so touch could read
    // as permanently pressed or never settle.
    ((((buf[1] as u16) << 8) | buf[2] as u16) >> 3) & 0x0FFF
}

/// When DIAG is on, periodically log raw touch reads so a dead touch panel can be
/// diagnosed over serial (idle baseline + any press). Set false once it works.
const TOUCH_DIAG: bool = true;
static TOUCH_DIAG_TICK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Read the touch panel; returns screen coords if pressed.
fn read_touch<S: embedded_hal::spi::SpiDevice>(dev: &mut S) -> Option<(i32, i32)> {
    let z1 = xpt_read(dev, 0xB0);
    let rx = xpt_read(dev, 0xD0) as i32; // X
    let ry = xpt_read(dev, 0x90) as i32; // Y
    if TOUCH_DIAG {
        // Log every ~64th idle poll (baseline), and every read that crosses the
        // pressure threshold (a real touch) — so we can see if z1/rx/ry move.
        let n = TOUCH_DIAG_TICK.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if z1 >= Z_THRESH || n % 64 == 0 {
            log::warn!("touch raw: z1={z1} rx={rx} ry={ry} (thresh={Z_THRESH})");
        }
    }
    if z1 < Z_THRESH {
        return None;
    }
    // Normalise to 0..1000 in the panel-native touch axes…
    let nx = ((rx - X_MIN) * 1000 / (X_MAX - X_MIN)).clamp(0, 1000);
    let ny = ((ry - Y_MIN) * 1000 / (Y_MAX - Y_MIN)).clamp(0, 1000);
    // …then rotate into the displayed 480×320 space. This panel renders at 270°
    // + horizontal flip, so the native X axis runs DOWN the screen and the native
    // Y axis runs ACROSS it: swap, and invert the vertical. (These constants +
    // rotation will become per-screen, set by the calibration wizard.)
    let sx = (ny * (ui::W - 1) / 1000).clamp(0, ui::W - 1);
    let sy = ((1000 - nx) * (ui::H - 1) / 1000).clamp(0, ui::H - 1);
    Some((sx, sy))
}

#[derive(PartialEq, Clone, Copy)]
enum RaceMode {
    Race,
    Config,
}

fn now_ms() -> i64 {
    unsafe { esp_idf_svc::sys::esp_timer_get_time() / 1000 }
}

/// Build a mipidsi orientation from the runtime config (rotation 0..3 = 0/90/
/// 180/270°, plus optional mirroring), so a differently-mounted panel is a
/// config change rather than a recompile.
fn make_orientation(rot: u8, flip_h: bool, flip_v: bool) -> Orientation {
    let mut o = Orientation::new().rotate(match rot & 3 {
        0 => Rotation::Deg0,
        1 => Rotation::Deg90,
        2 => Rotation::Deg180,
        _ => Rotation::Deg270,
    });
    if flip_h {
        o = o.flip_horizontal();
    }
    if flip_v {
        o = o.flip_vertical();
    }
    o
}

fn display_task() {
    let peripherals = match Peripherals::take() {
        Ok(p) => p,
        Err(e) => {
            log::error!("peripherals take failed: {e:?}");
            return;
        }
    };
    let pins = state::with(|s| s.pins);

    let driver = match SpiDriver::new(
        peripherals.spi2,
        unsafe { AnyIOPin::new(pins.sclk) },
        unsafe { AnyIOPin::new(pins.mosi) },
        Some(unsafe { AnyIOPin::new(pins.miso) }),
        // DMA so big display flushes don't crawl on the CPU.
        &SpiDriverConfig::new().dma(Dma::Auto(8192)),
    ) {
        Ok(d) => d,
        Err(e) => {
            log::error!("spi bus: {e:?}");
            return;
        }
    };

    let lcd_cfg = SpiConfig::new().baudrate(40.MHz().into());
    let touch_cfg = SpiConfig::new().baudrate(2.MHz().into());

    let dc = SharedDc(Rc::new(RefCell::new(
        PinDriver::output(unsafe { AnyIOPin::new(pins.dc) }).expect("dc"),
    )));

    let dev1 = SpiDeviceDriver::new(&driver, Some(unsafe { AnyIOPin::new(pins.disp1_cs) }), &lcd_cfg).expect("dev1");
    let dev2 = SpiDeviceDriver::new(&driver, Some(unsafe { AnyIOPin::new(pins.disp2_cs) }), &lcd_cfg).expect("dev2");
    let mut t1 = SpiDeviceDriver::new(&driver, Some(unsafe { AnyIOPin::new(pins.touch1_cs) }), &touch_cfg).expect("t1");
    let mut t2 = SpiDeviceDriver::new(&driver, Some(unsafe { AnyIOPin::new(pins.touch2_cs) }), &touch_cfg).expect("t2");

    // Large per-display SPI scratch so mipidsi streams big chunks: 512 bytes was
    // only 256 px/flush, so a full redraw took ~600 transactions and you could
    // watch it paint. Heap-allocated (the task stack is just 12 KB); DMA above
    // makes each flush fast.
    let buf1: &'static mut [u8] = vec![0u8; 16384].leak();
    let buf2: &'static mut [u8] = vec![0u8; 16384].leak();
    let mut delay = Ets;
    let (drot, dfh, dfv, dbgr, dinv) =
        state::with(|s| (s.disp_rot, s.disp_flip_h, s.disp_flip_v, s.disp_bgr, s.disp_inv));
    let orient = make_orientation(drot, dfh, dfv);
    let color_order = if dbgr { ColorOrder::Bgr } else { ColorOrder::Rgb };
    let inversion = if dinv { ColorInversion::Inverted } else { ColorInversion::Normal };
    let mut disp1 = Builder::new(ST7796, SpiInterface::new(dev1, dc.clone(), buf1))
        .display_size(320, 480)
        .orientation(orient)
        .color_order(color_order)
        .invert_colors(inversion)
        .init(&mut delay)
        .expect("disp1");
    let mut disp2 = Builder::new(ST7796, SpiInterface::new(dev2, dc.clone(), buf2))
        .display_size(320, 480)
        .orientation(orient)
        .color_order(color_order)
        .invert_colors(inversion)
        .init(&mut delay)
        .expect("disp2");

    // race_screen pin = which physical panel index (0/1) shows the race screen.
    let race_is_1 = pins.race_screen == 0;

    let mut mode = RaceMode::Race;
    let mut last_touch_ms = now_ms();
    let mut page: usize = 0;
    let mut toggle_on = [false; 32];
    let mut prev_btn_down = false;
    let mut prev_d1_down = false;
    // Push (momentary) UiDoc buttons currently held down per panel, so we can send
    // the HID button-up when the finger lifts (true hold, not a fixed pulse).
    let mut held_race: Option<usize> = None;
    let mut held_side: Option<usize> = None;
    // active tab page per panel (for tabbed screens)
    let mut race_tab: u8 = 0;
    let mut side_tab: u8 = 0;
    // TOUCH_DIAG: last touch point + fade countdown per panel, so we can draw a
    // marker where the firmware thinks you touched (verifies the X/Y mapping).
    let mut race_mark: Option<(i32, i32)> = None;
    let mut race_fade: u8 = 0;
    let mut side_mark: Option<(i32, i32)> = None;
    let mut side_fade: u8 = 0;
    const MARK_FADE: u8 = 12; // ~12 frames * 33 ms ≈ 0.4 s ripple-out on release

    // pith-ui dirty-rect state (one cache per physical panel). The active layout is
    // cloned locally and only refreshed when the dashboard pushes a new UiDoc
    // (tracked via ui_ver), so the hot loop never re-parses or re-clones it.
    let mut race_cache = pith_ui::RenderCache::new();
    let mut side_cache = pith_ui::RenderCache::new();
    let mut local_doc: Option<pith_ui::UiDoc> = state::with(|s| s.ui_doc.clone());
    let mut last_ui_ver = state::with(|s| s.ui_ver);
    let mut last_mode = mode;
    let mut last_disp_ver = state::with(|s| s.disp_ver);

    // Two PSRAM framebuffers, one per physical panel. The UI draws into RAM
    // (fonts + fills never touch SPI) and each panel is flushed in a single DMA
    // blit. fb1 -> disp1, fb2 -> disp2.
    let mut fb1 = FrameBuf::new(ui::W, ui::H);
    let mut fb2 = FrameBuf::new(ui::W, ui::H);

    loop {
        let t = *usb::TELEM.lock().unwrap();
        let now = now_ms();

        if ota::ACTIVE.load(std::sync::atomic::Ordering::Relaxed) {
            let pct = ota_pct();
            ui::render_ota(&mut fb1, pct);
            blit!(disp1, fb1);
            blit!(disp2, fb1);
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        // Re-apply display orientation live when the dashboard changes it (@DO).
        let cur_disp_ver = state::with(|s| s.disp_ver);
        if cur_disp_ver != last_disp_ver {
            last_disp_ver = cur_disp_ver;
            let (r, fh, fv) = state::with(|s| (s.disp_rot, s.disp_flip_h, s.disp_flip_v));
            let o = make_orientation(r, fh, fv);
            let _ = disp1.set_orientation(o);
            let _ = disp2.set_orientation(o);
        }

        // Refresh the cached pith-ui layout only when the dashboard pushes a new one,
        // and force a full repaint of both panels on the next frame.
        let cur_ver = state::with(|s| s.ui_ver);
        if cur_ver != last_ui_ver {
            local_doc = state::with(|s| s.ui_doc.clone());
            last_ui_ver = cur_ver;
            race_cache.invalidate();
            side_cache.invalidate();
        }

        // Parse the pushed layouts each frame (cheap; could cache on change).
        let race_json = state::with(|s| s.race_json.clone());
        let btn_json = state::with(|s| s.buttons_json.clone());
        let buttons = ui::parse_buttons(&btn_json).unwrap_or_default();

        // --- touch: race panel (UiDoc buttons / config nav / slider / sim / reboot) ---
        let race_touch = if race_is_1 { read_touch(&mut t1) } else { read_touch(&mut t2) };
        if TOUCH_DIAG {
            match race_touch {
                Some((tx, ty)) => {
                    race_mark = Some((tx, ty));
                    race_fade = MARK_FADE;
                }
                None => race_fade = race_fade.saturating_sub(1),
            }
        }
        match race_touch {
            Some((tx, ty)) => {
                last_touch_ms = now;
                if !prev_d1_down {
                    prev_d1_down = true;
                    if mode == RaceMode::Config {
                        handle_config_touch(&mut mode, tx, ty);
                    } else if ui::hit(ui::CONFIG_HOTSPOT, tx, ty) {
                        // top-left hotspot opens the on-device config screen (checked
                        // first so config stays reachable even over a tab/button)
                        mode = RaceMode::Config;
                    } else if let Some(scr) = local_doc
                        .as_ref()
                        .and_then(|d| d.screens.iter().find(|s| s.display == 0))
                    {
                        let tabbed = !scr.tabs.is_empty();
                        if tabbed {
                            if let Some(tb) = pith_ui::tab_at(scr.w, scr.tabs.len(), tx, ty) {
                                if tb != race_tab {
                                    race_tab = tb;
                                    race_cache.invalidate();
                                }
                            } else {
                                ui_button_down(scr, race_tab as i32, tx, ty, &mut held_race, &mut toggle_on);
                            }
                        } else {
                            ui_button_down(scr, -1, tx, ty, &mut held_race, &mut toggle_on);
                        }
                    }
                }
            }
            None => {
                prev_d1_down = false;
                if let Some(b) = held_race.take() {
                    hid::set(b, false); // release a held push button
                }
            }
        }
        if mode == RaceMode::Config && now - last_touch_ms > 8000 {
            mode = RaceMode::Race; // auto-return
        }
        // A mode switch changes the whole race panel -> force a full repaint.
        if mode != last_mode {
            race_cache.invalidate();
            last_mode = mode;
        }

        // --- touch: side/button panel ---
        let btn_touch = if race_is_1 { read_touch(&mut t2) } else { read_touch(&mut t1) };
        if TOUCH_DIAG {
            match btn_touch {
                Some((tx, ty)) => {
                    side_mark = Some((tx, ty));
                    side_fade = MARK_FADE;
                }
                None => side_fade = side_fade.saturating_sub(1),
            }
        }
        // Prefer UiDoc buttons placed on display 1; fall back to the legacy button box.
        let has_side_ui = local_doc
            .as_ref()
            .map(|d| d.screens.iter().any(|s| s.display == 1))
            .unwrap_or(false);
        if has_side_ui {
            if let Some(scr) = local_doc
                .as_ref()
                .and_then(|d| d.screens.iter().find(|s| s.display == 1))
            {
                ui_button_touch(scr, btn_touch, &mut prev_btn_down, &mut held_side, &mut toggle_on, &mut side_tab);
            }
        } else {
            handle_button_touch(&buttons, &mut page, &mut toggle_on, &mut prev_btn_down, btn_touch);
        }
        // Safety net: a held push button must release on finger-up even if the side
        // UiDoc was swapped out mid-press (otherwise the HID bit would stick on).
        if btn_touch.is_none() {
            if let Some(b) = held_side.take() {
                hid::set(b, false);
            }
        }

        // A screen from the active UiDoc is selected by display index (0 = race
        // panel, 1 = side panel). Absent -> fall back to the legacy renderers.
        // Each panel is drawn into its framebuffer, then blitted in one DMA write.
        // --- render: side/button panel first (shared-bus ordering) ---
        {
            let fb = if race_is_1 { &mut fb2 } else { &mut fb1 };
            if TOUCH_DIAG {
                fb.render_touch_test(side_mark, side_fade, MARK_FADE);
                if side_fade == 0 {
                    side_mark = None;
                }
            } else {
                let side_scr = local_doc
                    .as_ref()
                    .and_then(|d| d.screens.iter().find(|s| s.display == 1));
                if let Some(scr) = side_scr {
                    if scr.tabs.is_empty() {
                        pith_ui::render_screen_diff(scr, &t, now, &mut side_cache, fb);
                    } else {
                        // full repaint of the active tab page (simple button screen)
                        pith_ui::render_tabbed(scr, side_tab, &t, now, fb);
                    }
                } else {
                    ui::render_buttons(fb, &buttons, page, &t, &toggle_on);
                }
            }
            if race_is_1 {
                blit!(disp2, fb2);
            } else {
                blit!(disp1, fb1);
            }
        }
        // --- render: race panel ---
        {
            let fb = if race_is_1 { &mut fb1 } else { &mut fb2 };
            if TOUCH_DIAG {
                fb.render_touch_test(race_mark, race_fade, MARK_FADE);
                if race_fade == 0 {
                    race_mark = None;
                }
            } else {
            match mode {
                RaceMode::Config => {
                    let (b, sim, car) =
                        state::with(|s| (s.brightness, s.sim_on, s.car_model.clone()));
                    let heap_kb =
                        (unsafe { esp_idf_svc::sys::esp_get_free_heap_size() } / 1024) as i32;
                    let uptime_s = unsafe { esp_idf_svc::sys::esp_timer_get_time() } / 1_000_000;
                    let info = ui::ConfigInfo {
                        fw: env!("CARGO_PKG_VERSION"),
                        board: option_env!("PITHDDU_BOARD").unwrap_or("xiao_s3"),
                        serial: crate::device::serial(),
                        car: &car,
                        heap_kb,
                        uptime_s,
                        brightness: b,
                        sim,
                    };
                    ui::render_config(fb, &info);
                }
                RaceMode::Race => {
                    let race_scr = local_doc
                        .as_ref()
                        .and_then(|d| d.screens.iter().find(|s| s.display == 0));
                    if let Some(scr) = race_scr {
                        if scr.tabs.is_empty() {
                            pith_ui::render_screen_diff(scr, &t, now, &mut race_cache, fb);
                        } else {
                            pith_ui::render_tabbed(scr, race_tab, &t, now, fb);
                        }
                    } else {
                        let layout = ui::parse_race(&race_json).unwrap_or_default();
                        ui::render_race(fb, &layout, &t, now);
                    }
                    // discoverable tap-to-config affordance (drawn over the race UI)
                    ui::render_config_hint(fb);
                }
            }
            }
            if race_is_1 {
                blit!(disp1, fb1);
            } else {
                blit!(disp2, fb2);
            }
        }

        thread::sleep(Duration::from_millis(33));
    }
}

fn handle_config_touch(mode: &mut RaceMode, tx: i32, ty: i32) {
    if ui::hit(ui::BACK_BTN, tx, ty) {
        *mode = RaceMode::Race;
    } else if ui::hit(ui::SLD, tx, ty) {
        let pct = ((tx - ui::SLD.0) * 100 / ui::SLD.2).clamp(0, 100);
        state::with(|s| s.set_brightness(pct));
    } else if ui::hit(ui::SIM_BTN, tx, ty) {
        state::with(|s| s.sim_on = !s.sim_on);
    } else if ui::hit(ui::RBT_BTN, tx, ty) {
        thread::sleep(Duration::from_millis(150));
        unsafe { esp_idf_svc::sys::esp_restart() };
    }
}

/// Hit-test a down-edge tap against the `Button` nodes of a UiDoc screen and drive
/// HID. Toggles latch (`toggle_on`); push buttons press now and record `held` so the
/// caller can release them on finger-up. Returns true if a button consumed the tap.
fn ui_button_down(
    scr: &pith_ui::Screen,
    active_page: i32, // -1 = all pages (untabbed); else only this tab page
    tx: i32,
    ty: i32,
    held: &mut Option<usize>,
    toggle_on: &mut [bool; 32],
) -> bool {
    for node in &scr.nodes {
        if active_page >= 0 && node.page as i32 != active_page {
            continue;
        }
        if let pith_ui::Kind::Button { hid, toggle, .. } = &node.kind {
            let (hid, toggle) = (*hid, *toggle);
            if hid == 0 {
                continue;
            }
            let bit = (hid as usize - 1).min(31);
            let r = &node.rect;
            if tx >= r.x && tx < r.x + r.w as i32 && ty >= r.y && ty < r.y + r.h as i32 {
                if toggle {
                    let on = !toggle_on[bit];
                    toggle_on[bit] = on;
                    hid::set(bit, on);
                } else {
                    hid::set(bit, true);
                    *held = Some(bit);
                }
                return true;
            }
        }
    }
    false
}

/// Full press/release dispatch for UiDoc buttons on a panel (edge-tracked): a tap
/// presses (toggle latches / push holds); lifting releases a held push button.
fn ui_button_touch(
    scr: &pith_ui::Screen,
    touch: Option<(i32, i32)>,
    prev_down: &mut bool,
    held: &mut Option<usize>,
    toggle_on: &mut [bool; 32],
    tab: &mut u8,
) {
    let tabbed = !scr.tabs.is_empty();
    match touch {
        Some((tx, ty)) => {
            if !*prev_down {
                *prev_down = true;
                // a tap in the tab strip switches page; otherwise dispatch buttons
                if tabbed {
                    if let Some(t) = pith_ui::tab_at(scr.w, scr.tabs.len(), tx, ty) {
                        *tab = t;
                        return;
                    }
                }
                let active = if tabbed { *tab as i32 } else { -1 };
                ui_button_down(scr, active, tx, ty, held, toggle_on);
            }
        }
        None => {
            *prev_down = false;
            if let Some(b) = held.take() {
                hid::set(b, false);
            }
        }
    }
}

fn handle_button_touch(
    buttons: &ui::Buttons,
    page: &mut usize,
    toggle_on: &mut [bool; 32],
    prev_down: &mut bool,
    touch: Option<(i32, i32)>,
) {
    match touch {
        Some((tx, ty)) => {
            if *prev_down {
                return; // edge-triggered
            }
            *prev_down = true;
            // tab bar?
            if ty < ui::TABH {
                let np = buttons.pages.len().max(1) as i32;
                let tw = ui::W / np;
                let p = (tx / tw).clamp(0, np - 1) as usize;
                *page = p;
                return;
            }
            if let Some(pg) = buttons.pages.get(*page) {
                for b in pg {
                    let r = ui::button_rect(b.hid % 8);
                    if ui::hit(r, tx, ty) {
                        if b.toggle {
                            let on = !toggle_on[b.hid.min(31)];
                            toggle_on[b.hid.min(31)] = on;
                            hid::set(b.hid, on);
                        } else {
                            hid::pulse(b.hid);
                        }
                    }
                }
            }
        }
        None => {
            *prev_down = false;
        }
    }
}

fn ota_pct() -> i32 {
    // ota module tracks progress internally; expose a coarse value.
    ota::progress_pct()
}

/// Spawn the display + touch + UI task.
pub fn spawn() {
    thread::Builder::new()
        .stack_size(12288)
        .name("display".into())
        .spawn(display_task)
        .expect("spawn display task");
}
