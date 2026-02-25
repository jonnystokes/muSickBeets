/// Toggleable debug flags for diagnostic output.
/// Set any flag to `true` to enable that category of eprintln! logging.
/// All flags default to `false` for normal builds.

pub const CURSOR_DBG: bool = false;
pub const FFT_DBG: bool = false;
pub const PLAYBACK_DBG: bool = false;
pub const RENDER_DBG: bool = false;

/// Convenience macro: prints to stderr only if the given flag is true.
/// Usage: `dbg_log!(CURSOR_DBG, "value is {}", x);`
#[macro_export]
macro_rules! dbg_log {
    ($flag:expr, $($arg:tt)*) => {
        if $flag {
            eprintln!($($arg)*);
        }
    };
}
