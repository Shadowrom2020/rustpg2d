//! Messages for the terminal (the browser's console on the web).

pub fn write(level: &str, text: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        if level == "error" || level == "warn" {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    crate::web::log(level, text);
}

#[doc(hidden)]
#[macro_export]
macro_rules! __gfx_log_info {
    ($($t:tt)*) => { $crate::logging::write("info", &format!($($t)*)) };
}
#[doc(hidden)]
#[macro_export]
macro_rules! __gfx_log_warn {
    ($($t:tt)*) => { $crate::logging::write("warn", &format!($($t)*)) };
}
#[doc(hidden)]
#[macro_export]
macro_rules! __gfx_log_error {
    ($($t:tt)*) => { $crate::logging::write("error", &format!($($t)*)) };
}

pub use crate::__gfx_log_error as error;
pub use crate::__gfx_log_info as info;
pub use crate::__gfx_log_warn as warn;
