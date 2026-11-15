/// Conditionally print to stdout. Suppressed when `$state.quiet` is true.
#[macro_export]
macro_rules! qprintln {
    ($state:expr, $($arg:tt)*) => {
        if !$state.quiet {
            println!($($arg)*);
        }
    };
}
