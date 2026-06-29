//! Minimal Win32 shared-memory reader (runs under Wine in the game's prefix).

use core::ffi::c_void;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::Memory::{
    MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
    MEMORY_BASIC_INFORMATION,
};

/// Open a named shared-memory mapping read-only and return a snapshot of its
/// bytes. `None` if the mapping doesn't exist (game not running) or can't be read.
pub fn read_mapping(name: &str) -> Option<Vec<u8>> {
    let wide: Vec<u16> = name.encode_utf16().chain(core::iter::once(0)).collect();
    unsafe {
        let h = OpenFileMappingW(FILE_MAP_READ, 0, wide.as_ptr());
        if h.is_null() {
            return None;
        }
        let view = MapViewOfFile(h, FILE_MAP_READ, 0, 0, 0); // 0 = map whole mapping
        if view.Value.is_null() {
            CloseHandle(h);
            return None;
        }
        let mut mbi: MEMORY_BASIC_INFORMATION = core::mem::zeroed();
        let got = VirtualQuery(
            view.Value as *const c_void,
            &mut mbi,
            core::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
        );
        let out = if got != 0 && mbi.RegionSize > 0 {
            Some(std::slice::from_raw_parts(view.Value as *const u8, mbi.RegionSize).to_vec())
        } else {
            None
        };
        UnmapViewOfFile(view);
        CloseHandle(h);
        out
    }
}

/// Try several candidate names; return the first that reads.
pub fn read_any(names: &[&str]) -> Option<Vec<u8>> {
    names.iter().find_map(|n| read_mapping(n))
}
