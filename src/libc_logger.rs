use std::io::{self, Write};
use std::sync::Mutex;
use libc::{getenv, isatty, STDERR_FILENO};
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl Level {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "error" => Level::Error,
            "warn" => Level::Warn,
            "info" => Level::Info,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }
}

static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

pub struct Logger {
    level: Level,
    use_color: bool,
}

pub fn init_from_env() {
    unsafe {
        let rust_log = getenv(b"RUST_LOG\0".as_ptr() as *const libc::c_char);
        let level = if rust_log.is_null() {
            Level::Info
        } else {
            let c_str = CStr::from_ptr(rust_log);
            if let Ok(s) = c_str.to_str() {
                Level::from_str(s)
            } else {
                Level::Info
            }
        };
        
        let use_color = isatty(STDERR_FILENO) != 0;
        
        let logger = Logger { level, use_color };
        *LOGGER.lock().unwrap() = Some(logger);
    }
}

pub fn init_with_level(level: &str) {
    let level = Level::from_str(level);
    let use_color = unsafe { isatty(STDERR_FILENO) != 0 };
    
    let logger = Logger { level, use_color };
    *LOGGER.lock().unwrap() = Some(logger);
}

pub fn log(level: Level, args: std::fmt::Arguments) {
    let guard = LOGGER.lock().unwrap();
    if let Some(logger) = guard.as_ref() {
        if level <= logger.level {
            let mut stderr = io::stderr();
            
            if logger.use_color {
                let color = match level {
                    Level::Error => "\x1b[31m", // Red
                    Level::Warn => "\x1b[33m",  // Yellow
                    Level::Info => "\x1b[32m",  // Green
                    Level::Debug => "\x1b[36m", // Cyan
                    Level::Trace => "\x1b[90m", // Gray
                };
                let _ = write!(stderr, "{}{}\x1b[0m ", color, level.as_str());
            } else {
                let _ = write!(stderr, "{} ", level.as_str());
            }
            
            let _ = writeln!(stderr, "{}", args);
        }
    }
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::libc_logger::log($crate::libc_logger::Level::Error, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::libc_logger::log($crate::libc_logger::Level::Warn, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::libc_logger::log($crate::libc_logger::Level::Info, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        $crate::libc_logger::log($crate::libc_logger::Level::Debug, format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        $crate::libc_logger::log($crate::libc_logger::Level::Trace, format_args!($($arg)*))
    };
}