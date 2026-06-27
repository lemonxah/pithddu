use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub struct Hid {
    api: Option<HidApi>,
    dev: Option<HidDevice>,
    rx: Vec<u8>,
}

impl Default for Hid {
    fn default() -> Self {
        Hid {
            api: None,
            dev: None,
            rx: Vec::new(),
        }
    }
}

impl Hid {
    pub fn is_open(&self) -> bool {
        self.dev.is_some()
    }

    pub fn open(&mut self, vid: u16, pid: u16) -> bool {
        self.close();
        if self.api.is_none() {
            self.api = HidApi::new().ok();
        }
        let api = match self.api.as_ref() {
            Some(a) => a,
            None => return false,
        };
        match api.open(vid, pid) {
            Ok(d) => {
                let _ = d.set_blocking_mode(false);
                self.dev = Some(d);
                self.rx.clear();
                true
            }
            Err(_) => false,
        }
    }

    pub fn close(&mut self) {
        self.dev = None;
    }

    pub fn write(&mut self, data: &[u8]) -> bool {
        let dev = match self.dev.as_ref() {
            Some(d) => d,
            None => return false,
        };
        let mut off = 0;
        loop {
            let n = std::cmp::min(61, data.len() - off);
            let mut rep = [0u8; 64];
            rep[0] = 0x02;
            rep[1] = n as u8;
            if n > 0 {
                rep[2..2 + n].copy_from_slice(&data[off..off + n]);
            }
            if dev.write(&rep).is_err() {
                return false;
            }
            off += n;
            if off >= data.len() {
                break;
            }
        }
        true
    }

    pub fn write_str(&mut self, s: &str) -> bool {
        self.write(s.as_bytes())
    }

    pub fn drain(&mut self) {
        if let Some(dev) = self.dev.as_ref() {
            let mut buf = [0u8; 64];
            while dev.read_timeout(&mut buf, 0).unwrap_or(0) > 0 {}
        }
        self.rx.clear();
    }

    pub fn read_line(&mut self, ms: u64) -> String {
        let dev = match self.dev.as_ref() {
            Some(d) => d,
            None => return String::new(),
        };
        let deadline = Instant::now() + Duration::from_millis(ms);
        loop {
            if let Some(nl) = self.rx.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.rx.drain(..=nl).collect();
                let mut l = String::from_utf8_lossy(&line[..line.len() - 1]).to_string();
                while l.ends_with('\r') {
                    l.pop();
                }
                return l;
            }
            let mut buf = [0u8; 64];
            let r = dev.read_timeout(&mut buf, 40).unwrap_or(0);
            if r > 1 && buf[0] == 0x02 {
                let mut n = buf[1] as usize;
                if n > r - 2 {
                    n = r - 2;
                }
                if n > 0 {
                    self.rx.extend_from_slice(&buf[2..2 + n]);
                }
            }
            if Instant::now() >= deadline && !self.rx.contains(&b'\n') {
                return String::new();
            }
        }
    }
}
