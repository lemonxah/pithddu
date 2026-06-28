//! Shared device transport for the Pith DDU: the HID command/log channel, the
//! serial fallback, and the `Dash` abstraction that drives both (including the
//! `@OTA` firmware-upload protocol). Used by the dashboard GUI and the
//! `pith-flash` CLI so they speak to the device through exactly one code path.

pub mod dash;
pub mod hid;
pub mod serial;

pub use dash::Dash;
pub use serial::{PortInfo, Serial};

/// USB IDs the Pith DDU enumerates as (Espressif VID, Pith PID).
pub const PITH_VID: u16 = 0x303A;
pub const PITH_PID: u16 = 0x4002;
