#![allow(unused)]
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOGGER: OnceLock<Logger> = OnceLock::new();

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERROR",
        }
    }
}

struct Logger {
    level: LogLevel,
    sinks: Mutex<Vec<Box<dyn Write + Send>>>,
}

impl Logger {
    fn write(&self, level: LogLevel, message: &str) {
        if level < self.level {
            return;
        }
        let line = format!("[{}] {}", level.as_str(), message);

        eprintln!("{line}");

        let mut sinks = self.sinks.lock().unwrap_or_else(|e| e.into_inner());
        for sink in sinks.iter_mut() {
            let _ = writeln!(sink, "{line}");
        }
    }
}

pub fn init(level: LogLevel) {
    let _ = LOGGER.set(Logger {
        level,
        sinks: Mutex::new(Vec::new()),
    });
}

pub fn add_file_sink(path: impl AsRef<Path>) -> std::io::Result<()> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let logger = LOGGER
        .get()
        .expect("logger not initialized; call logger::init first");
    logger
        .sinks
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Box::new(file));
    Ok(())
}

fn log(level: LogLevel, message: String) {
    match LOGGER.get() {
        Some(logger) => logger.write(level, &message),
        None => eprintln!("[{}] {}", level.as_str(), message),
    }
}

#[doc(hidden)]
pub fn __debug(message: String) {
    log(LogLevel::Debug, message);
}
#[doc(hidden)]
pub fn __info(message: String) {
    log(LogLevel::Info, message);
}
#[doc(hidden)]
pub fn __warn(message: String) {
    log(LogLevel::Warn, message);
}
#[doc(hidden)]
pub fn __error(message: String) {
    log(LogLevel::Error, message);
}

#[doc(hidden)]
pub fn __type_name_of<T>(_: T) -> &'static str {
    std::any::type_name::<T>()
}

#[doc(hidden)]
macro_rules! __function_name {
    () => {{
        fn f() {}
        $crate::logger::__type_name_of(f)
            .rsplit("::")
            .find(|&part| part != "f" && part != "{{closure}}")
            .expect("Short function name")
    }};
}

#[doc(hidden)]
macro_rules! debug {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            let fn_name = __function_name!();
            $crate::logger::__debug(format!(
                "{}() {} ({}:{})",
                fn_name,
                format_args!($($arg)*),
                file!(),
                line!(),
            ));
        }
    }};
}

#[doc(hidden)]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::logger::__info(format!($($arg)*))
    };
}

#[doc(hidden)]
macro_rules! warn {
    ($($arg:tt)*) => {{
        let fn_name = __function_name!();
        $crate::logger::__warn(format!(
            "{}() {} ({}:{})",
            fn_name,
            format!($($arg)*),
            file!(),
            line!(),
        ));
    }};
}

#[doc(hidden)]
macro_rules! error {
    ($($arg:tt)*) => {{
        let fn_name = __function_name!();
        $crate::logger::__error(format!(
            "{}() {} ({}:{})",
            fn_name,
            format!($($arg)*),
            file!(),
            line!(),
        ));
    }};
}
