use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

struct Logger {
    enabled: bool,
    path: PathBuf,
}

fn get_log_path() -> PathBuf {
    let dir = dirs::home_dir()
        .map(|h| h.join(".config").join("wtch"))
        .expect("home directory not found");
    let _ = fs::create_dir_all(&dir);
    let date = chrono::Local::now().format("%Y-%m-%d");
    dir.join(format!("{date}.log"))
}

/// Initializes the logger. Can be called multiple times to toggle debug on/off.
pub fn init(debug: bool) {
    let path = get_log_path();
    match LOGGER.get() {
        Some(mutex) => {
            if let Ok(mut logger) = mutex.lock() {
                logger.enabled = debug;
                logger.path = path;
            }
        }
        None => {
            let _ = LOGGER.set(Mutex::new(Logger {
                enabled: debug,
                path,
            }));
        }
    }
    if debug {
        write_log("INFO", "debug logging started");
    }
}

/// Writes a log line to the file if debug is enabled.
pub fn write_log(level: &str, message: &str) {
    let Some(mutex) = LOGGER.get() else { return };
    let Ok(logger) = mutex.lock() else { return };
    if !logger.enabled {
        return;
    }
    let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
    let line = format!("[{timestamp}] {level:<5} {message}\n");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&logger.path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Convenience macros-like functions.
pub fn debug(message: &str) {
    write_log("DEBUG", message);
}

pub fn info(message: &str) {
    write_log("INFO", message);
}

pub fn warn(message: &str) {
    write_log("WARN", message);
}

pub fn error(message: &str) {
    write_log("ERROR", message);
}
