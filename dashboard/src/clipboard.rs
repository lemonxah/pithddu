use std::io::Write;
use std::process::{Command, Stdio};

fn run_pipe(shell_cmd: &str, text: &str) -> bool {
    #[cfg(unix)]
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(shell_cmd)
        .stdin(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    #[cfg(windows)]
    let mut child = match Command::new(shell_cmd).stdin(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    child.wait().map(|s| s.success()).unwrap_or(false)
}

pub fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "linux")]
    {
        let cmd = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            "wl-copy 2>/dev/null || xclip -selection clipboard 2>/dev/null || xsel -ib 2>/dev/null"
        } else {
            "xclip -selection clipboard 2>/dev/null || xsel -ib 2>/dev/null || wl-copy 2>/dev/null"
        };
        run_pipe(cmd, text)
    }
    #[cfg(target_os = "macos")]
    {
        run_pipe("pbcopy", text)
    }
    #[cfg(windows)]
    {
        run_pipe("clip", text)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = text;
        false
    }
}
