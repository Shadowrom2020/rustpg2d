//! Keeps the game and editor binaries runnable on older Linux distributions.
//!
//! A binary linked on a very new glibc asks for the newest *version* of every libc function it
//! calls, even when an older one would do. glibc 2.43 / 2.44 re-versioned a few math functions
//! (`atan2f`, `acosf`, `cosh`, `sinh`; 2.35: `hypot`, `hypotf`), so a binary built on a current rolling-release distribution would
//! refuse to start on Debian 12, Ubuntu 22.04 or inside a Flatpak runtime. Defining them in the
//! binary itself (portable implementations from the `libm` crate) means the dynamic linker
//! never looks them up in glibc. Every binary linking the engine gets them: the linker takes
//! these definitions before reaching libm. The editor's export shows which glibc a game binary needs.

#[no_mangle]
pub extern "C" fn atan2f(y: f32, x: f32) -> f32 {
    libm::atan2f(y, x)
}

#[no_mangle]
pub extern "C" fn acosf(x: f32) -> f32 {
    libm::acosf(x)
}

#[no_mangle]
pub extern "C" fn cosh(x: f64) -> f64 {
    libm::cosh(x)
}

#[no_mangle]
pub extern "C" fn sinh(x: f64) -> f64 {
    libm::sinh(x)
}

// glibc 2.35 re-versioned these; without them the floor is 2.34 (Debian 12, Ubuntu 22.04,
// Fedora 35, RHEL / Rocky / Alma 9), set by the threading functions merged into libc in 2.34.
#[no_mangle]
pub extern "C" fn hypotf(x: f32, y: f32) -> f32 {
    libm::hypotf(x, y)
}

#[no_mangle]
pub extern "C" fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

// glibc 2.43 re-versioned these too; the platform layer's libraries (wgpu's shader compiler, the
// sound mixer) call them.
#[no_mangle]
pub extern "C" fn acoshf(x: f32) -> f32 {
    libm::acoshf(x)
}

#[no_mangle]
pub extern "C" fn asinf(x: f32) -> f32 {
    libm::asinf(x)
}

#[no_mangle]
pub extern "C" fn coshf(x: f32) -> f32 {
    libm::coshf(x)
}

#[no_mangle]
pub extern "C" fn log10f(x: f32) -> f32 {
    libm::log10f(x)
}

#[no_mangle]
pub extern "C" fn sinhf(x: f32) -> f32 {
    libm::sinhf(x)
}
