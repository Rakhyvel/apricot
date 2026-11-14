//! A small logger that prints to stderr, for apps that don't set up their own

use log::{LevelFilter, Log, Metadata, Record};

struct StderrLogger;

impl Log for StderrLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

/// Print log messages at `level` and above to stderr, unless a logger is already set up
pub(crate) fn init(level: LevelFilter) {
    static LOGGER: StderrLogger = StderrLogger;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(level);
    }
}
