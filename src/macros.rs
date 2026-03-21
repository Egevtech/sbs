#[macro_export]
macro_rules! log {
    (PANIC_CODE, $exitCode:expr, $($msg:tt)*) => {
        eprintln!("[PANIC/CODE {}] {}", $exitCode, format_args!($($msg)*));
        std::process::exit($exitCode);
    };

    (PANIC, $($msg:tt)*) => {
        eprintln!("[PANIC] {}", format_args!($($msg)*));
        std::process::exit(-1);
    };

    (OOPS, $($msg:tt)*) => {
        eprintln!("[OOPS] {}", format_args!($($msg)*));
        std::process::exit(-1);
    };

    (WARN, $($msg:tt)*) => {
        eprintln!("[WARN] {}", format_args!($($msg)*));
    };

    ($msgtype:ident, $($msg:tt)*) => {
        #[cfg(debug_assertions)]
        println!("[{}] {}", stringify!($msgtype), format_args!($($msg)*));
    };
}

