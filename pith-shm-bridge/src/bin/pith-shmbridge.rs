//! pith-shmbridge — runs inside a sim's Proton/Wine prefix and copies the sim's
//! shared memory to a real `/dev/shm` file (via Wine's `Z:\dev\shm`), so the
//! dashboard's native shared-memory reader can pick it up. Use this if you'd
//! rather the dashboard read `/dev/shm` directly than receive UDP from the shim.

#[cfg(windows)]
fn main() {
    use std::time::Duration;

    println!("pith-shmbridge: mirroring sim shared memory → /dev/shm (~50 Hz)");
    let mut announced = false;
    loop {
        let mut any = false;
        for (names, dest) in pith_shm_bridge::COPY_BLOCKS {
            if let Some(bytes) = pith_shm_bridge::win::read_any(names) {
                if std::fs::write(dest, &bytes).is_ok() {
                    any = true;
                }
            }
        }
        if any && !announced {
            announced = true;
            println!("pith-shmbridge: a sim is running — blocks are live in /dev/shm");
        } else if !any {
            announced = false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("pith-shmbridge only runs on Windows / Proton-Wine.");
}
