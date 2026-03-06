use std::sync::OnceLock;
use std::time::Instant;

/// Toggleable debug flags for diagnostic output.
/// Set any flag to `true` to enable that category of logging.
/// All flags default to `false` for normal builds.
#[allow(dead_code)]
pub const CURSOR_DBG: bool = false;
#[allow(dead_code)]
pub const FFT_DBG: bool = true;
#[allow(dead_code)]
pub const PLAYBACK_DBG: bool = false;
#[allow(dead_code)]
pub const RENDER_DBG: bool = false;

static START_TIME: OnceLock<Instant> = OnceLock::new();

fn elapsed_since_start() -> f64 {
    let start = START_TIME.get_or_init(Instant::now);
    start.elapsed().as_secs_f64()
}

/// Return a log prefix like "[123.456s]" (seconds since program start).
pub fn log_time_prefix() -> String {
    format!("[{:.3}s]", elapsed_since_start())
}

/// Helper to format `Instant` values relative to the program start.
pub fn instant_since_start(instant: Instant) -> String {
    let start = START_TIME.get_or_init(Instant::now);
    let delta = instant.duration_since(*start);
    format!("{:.3}s", delta.as_secs_f64())
}

pub(crate) fn print_log(category: &str, message: std::fmt::Arguments<'_>) {
    eprintln!("{} [{}] {}", log_time_prefix(), category, message);
}

/// Debug logging macro gated by a flag. Adds `[time][category]` automatically.
#[macro_export]
macro_rules! dbg_log {
    ($flag:expr, $category:expr, $($arg:tt)*) => {
        if $flag {
            $crate::debug_flags::print_log($category, format_args!($($arg)*));
        }
    };
}

/// Always-on logging macro for operational messages.
#[macro_export]
macro_rules! app_log {
    ($category:expr, $($arg:tt)*) => {
        $crate::debug_flags::print_log($category, format_args!($($arg)*));
    };
}
