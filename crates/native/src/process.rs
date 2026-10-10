//! Starting programs: the Craft Apps themselves, the file manager, and helper tools.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A `Command` that won't flash a console window on Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Runs a helper and returns its standard output when it succeeds within `timeout`.
pub fn capture(program: &str, args: &[&str], timeout: Duration) -> Option<String> {
    let mut child = command(program).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = std::io::Read::read_to_end(&mut std::io::Read::take(&mut stdout, 8 << 20), &mut out);
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let out = reader.join().ok()?;
    status.success().then(|| String::from_utf8_lossy(&out).into_owned())
}

/// Starts `cmd` without waiting for it, and reaps it when it exits.
pub fn spawn_detached(mut cmd: Command) -> Result<(), String> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("couldn't start it ({e})"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Starts an app binary from its own folder.
pub fn launch_binary(exe: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = command(exe);
    cmd.args(args);
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    // An AppImage'd launcher sets these for itself; they'd break the app we start.
    for var in [
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        "APPDIR",
        "APPIMAGE",
        "ARGV0",
        "OWD",
        "GDK_PIXBUF_MODULE_FILE",
        "GIO_MODULE_DIR",
        "GTK_PATH",
        "PYTHONHOME",
        "QT_PLUGIN_PATH",
    ] {
        if std::env::var_os("APPIMAGE").is_some() {
            cmd.env_remove(var);
        }
    }
    spawn_detached(cmd)
}

/// Opens a file, folder or URL with the system's default handler.
pub fn open(target: &std::ffi::OsStr) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        command("open")
    } else if cfg!(windows) {
        let mut c = command("explorer");
        c.arg(target);
        return spawn_detached(c);
    } else {
        command("xdg-open")
    };
    cmd.arg(target);
    spawn_detached(cmd)
}

/// Shows `path` selected in the file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    if cfg!(target_os = "macos") {
        let mut cmd = command("open");
        cmd.arg("-R").arg(path);
        spawn_detached(cmd)
    } else if cfg!(windows) {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path);
        let mut cmd = command("explorer");
        cmd.arg(arg);
        spawn_detached(cmd)
    } else {
        open(path.parent().unwrap_or(path).as_os_str())
    }
}

/// Whether a process started from inside `path` (an exe, an AppImage, or a `.app`) is running.
pub fn is_running(path: &Path) -> bool {
    if cfg!(windows) {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false };
        let filter = format!("IMAGENAME eq {name}");
        capture("tasklist", &["/FI", &filter, "/NH", "/FO", "CSV"], Duration::from_secs(5))
            .is_some_and(|out| out.to_ascii_lowercase().contains(&name.to_ascii_lowercase()))
    } else {
        let Some(p) = path.to_str() else { return false };
        // pgrep exits 0 when something matches; it never matches itself.
        capture("pgrep", &["-f", "--", p], Duration::from_secs(5)).is_some()
    }
}
