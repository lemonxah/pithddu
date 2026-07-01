// Build script for the pith-recovery app (Rust / esp-idf-sys). Emits the
// esp-idf-sys link flags, same as the main firmware.
fn main() {
    embuild::espidf::sysenv::output();
}
