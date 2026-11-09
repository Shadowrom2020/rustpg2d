//! Start-up of the desktop programs (game runner and editor) on Windows: they are GUI programs
//! (`windows_subsystem = "windows"`: no console window of their own), so
//! - started from a terminal, their output (logs, `--help`, automated runs' `status`) goes there;
//!   output that is already redirected (the editor reading `--engine-build`) stays where it is;
//! - a panic shows a message box instead of vanishing silently: an OpenGL text when the GL loader
//!   failed (Windows without a graphics driver, e.g. a VM without 3D acceleration, has only
//!   OpenGL 1.1), else the error.
//!
//! Elsewhere [`startup`] does nothing.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set while [`catch_panic`] runs: the Windows panic hook then only logs (no message box, no exit).
static CATCHING: AtomicBool = AtomicBool::new(false);

/// Runs `f` and turns a panic into its message (the editor's agent server answers a tool's
/// panic with an error and keeps running). The default hook still prints the panic.
pub fn catch_panic<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    CATCHING.store(true, Ordering::Relaxed);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CATCHING.store(false, Ordering::Relaxed);
    r.map_err(|p| p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "unknown panic".into()))
}

/// Call first thing in `window_conf`; `app` names the program in the message box.
pub fn startup(app: &'static str) {
    #[cfg(windows)]
    windows::startup(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// Reports an error the program can't start with (a project that doesn't load) and exits: on the
/// terminal, and on Windows in a message box too (the GUI program has no console a player sees).
pub fn fatal(app: &str, msg: &str) -> ! {
    eprintln!("[{}] error: {msg}", app.to_lowercase().split_whitespace().last().unwrap_or("game"));
    #[cfg(windows)]
    windows::message_box(&format!("{app} can't start:\n\n{msg}"));
    std::process::exit(1)
}

#[cfg(windows)]
mod windows {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process: u32) -> i32;
        fn GetStdHandle(which: u32) -> isize;
    }
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const MB_ICONERROR: u32 = 0x10;

    pub fn startup(app: &'static str) {
        // SAFETY: plain Win32 calls without pointers.
        unsafe {
            let out = GetStdHandle(STD_OUTPUT_HANDLE);
            if out == 0 || out == -1 {
                AttachConsole(ATTACH_PARENT_PROCESS);
            }
        }
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            if super::CATCHING.load(super::Ordering::Relaxed) {
                return; // caught by catch_panic: the program goes on
            }
            let msg = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
            let gl = msg.contains("WGL") || info.location().is_some_and(|l| l.file().ends_with("gl.rs"));
            let text = if gl {
                format!("{app} needs a graphics driver with OpenGL 3.2 or newer, and this PC (or virtual machine) has none: Windows' own OpenGL 1.1 isn't enough. Installing the graphics card's driver (or turning on 3D acceleration in the virtual machine) helps.\n\n({msg})")
            } else {
                format!("{app} stopped because of an error:\n\n{msg}")
            };
            message_box(&text);
            std::process::exit(1);
        }));
    }

    /// An error box titled R(ust)PG2D.
    pub fn message_box(text: &str) {
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        // SAFETY: null-terminated UTF-16 strings that outlive the call.
        unsafe {
            MessageBoxW(0, wide(text).as_ptr(), wide("R(ust)PG2D").as_ptr(), MB_ICONERROR);
        }
    }
}
