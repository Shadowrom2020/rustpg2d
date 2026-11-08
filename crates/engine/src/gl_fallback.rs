//! Windows without an OpenGL 3 driver (virtual machines, the "Microsoft Basic Display Adapter",
//! some servers) only has Windows' built-in OpenGL 1.1, and miniquad can't make a context: it
//! panics ("WGL_ARB_pixel_format is required"). R(ust)PG2D ships Mesa's software OpenGL
//! (llvmpipe) in a `mesa` folder next to the Windows executables (the toolkit, and exported games
//! that include it): when the system's OpenGL fails, the program starts itself again with
//! `RUSTPG2D_SOFTWARE_GL=1`, which loads `mesa\opengl32.dll` before miniquad asks for
//! `opengl32.dll` (Windows then uses the module already loaded). Without the folder a message box
//! says what is missing instead of a panic. Nothing happens on other platforms.

/// Set (to anything) to use the software OpenGL from the start.
pub const SOFTWARE_GL_ENV: &str = "RUSTPG2D_SOFTWARE_GL";
/// Test hook: behave as if the system's OpenGL failed (only without `SOFTWARE_GL_ENV`).
pub const TEST_NO_GL_ENV: &str = "RUSTPG2D_TEST_NO_GL";
/// The folder with Mesa's `opengl32.dll` and `libgallium_wgl.dll`, next to the executable.
pub const MESA_DIR: &str = "mesa";

/// Call first (before the window is made). `program` names the program in the message box.
pub fn prepare(program: &str) {
    #[cfg(windows)]
    imp::prepare(program);
    #[cfg(not(windows))]
    let _ = program;
}

/// Whether a panic message comes from making the OpenGL context on Windows (miniquad's WGL code).
pub fn is_gl_failure(message: &str, file: Option<&str>) -> bool {
    message.contains("WGL") || file.is_some_and(|f| f.ends_with("wgl.rs"))
}

#[cfg(windows)]
mod imp {
    use std::path::PathBuf;

    use super::{is_gl_failure, MESA_DIR, SOFTWARE_GL_ENV, TEST_NO_GL_ENV};

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(name: *const u16, file: isize, flags: u32) -> isize;
    }
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    /// The DLL's own folder is searched for its dependencies (libgallium_wgl.dll).
    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x8;
    const MB_ICONERROR: u32 = 0x10;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn mesa_dir() -> Option<PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join(MESA_DIR))
    }

    fn message_box(caption: &str, text: &str) {
        // SAFETY: null-terminated UTF-16 strings that outlive the call.
        unsafe {
            MessageBoxW(0, wide(text).as_ptr(), wide(caption).as_ptr(), MB_ICONERROR);
        }
    }

    pub fn prepare(program: &str) {
        let software = std::env::var_os(SOFTWARE_GL_ENV).is_some();
        let dll = mesa_dir().map(|d| d.join("opengl32.dll"));
        if software {
            match &dll {
                Some(p) if p.is_file() => {
                    let path: Vec<u16> = p.as_os_str().encode_wide_nul();
                    // SAFETY: a null-terminated path; the module stays loaded for the process.
                    let h = unsafe { LoadLibraryExW(path.as_ptr(), 0, LOAD_WITH_ALTERED_SEARCH_PATH) };
                    if h == 0 {
                        eprintln!("[gl] could not load {}: {}", p.display(), std::io::Error::last_os_error());
                    } else {
                        eprintln!("[gl] using Mesa's software OpenGL ({})", p.display());
                    }
                }
                _ => eprintln!("[gl] {SOFTWARE_GL_ENV} is set but there is no {MESA_DIR}\\opengl32.dll next to the program"),
            }
        }
        let program = program.to_string();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info.payload().downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| info.payload().downcast_ref::<String>().cloned()).unwrap_or_default();
            if !is_gl_failure(&message, info.location().map(|l| l.file())) {
                previous(info);
                return;
            }
            let have_mesa = dll.as_ref().is_some_and(|p| p.is_file());
            if !software && have_mesa {
                eprintln!("[gl] the system's OpenGL can't be used ({message}); starting again with Mesa's software OpenGL");
                let status = std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(std::env::args_os().skip(1)).env(SOFTWARE_GL_ENV, "1").status());
                std::process::exit(status.ok().and_then(|s| s.code()).unwrap_or(1));
            }
            previous(info);
            let text = if software {
                format!("{program} could not start its graphics, not even with the software OpenGL in the \"{MESA_DIR}\" folder:\n\n{message}")
            } else {
                format!(
                    "{program} needs a graphics driver with OpenGL 3.2 or newer. This PC or virtual machine has none (Windows' built-in OpenGL 1.1 isn't enough).\n\nInstall the graphics driver, or put the \"{MESA_DIR}\" folder (software OpenGL) back next to {program}'s .exe file.\n\n({message})"
                )
            };
            message_box(&program, &text);
            std::process::exit(1);
        }));
        if !software && std::env::var_os(TEST_NO_GL_ENV).is_some() {
            panic!("WGL: simulated missing OpenGL driver ({TEST_NO_GL_ENV})");
        }
    }

    trait WideNul {
        fn encode_wide_nul(&self) -> Vec<u16>;
    }

    impl WideNul for std::ffi::OsStr {
        fn encode_wide_nul(&self) -> Vec<u16> {
            use std::os::windows::ffi::OsStrExt;
            self.encode_wide().chain(std::iter::once(0)).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_opengl_context_failures() {
        assert!(is_gl_failure("WGL_ARB_pixel_format is required", None));
        assert!(is_gl_failure("something", Some("/x/miniquad-0.4.11/src/native/windows/wgl.rs")));
        assert!(!is_gl_failure("index out of bounds", Some("src/runtime/mod.rs")));
    }
}
