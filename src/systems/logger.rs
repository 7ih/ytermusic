use std::io::Write;

use flume::Sender;
use once_cell::sync::Lazy;

static LOG: Lazy<Sender<String>> = Lazy::new(|| {
    let (tx, rx) = flume::unbounded::<String>();
    std::thread::spawn(move || {
        let mut buffer = String::new();
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("log.txt")
            .unwrap();
        while let Ok(e) = rx.recv() {
            buffer.clear();
            buffer.push_str(&(e + "\n"));
            while let Ok(e) = rx.try_recv() {
                buffer.push_str(&(e + "\n"));
            }
            file.write_all(buffer.as_bytes()).unwrap();
        }
    });
    tx
});

static LOGGER: SimpleLogger = SimpleLogger;
static LEVEL: Lazy<(LevelFilter, Level)> = Lazy::new(|| {
    let logger_env = std::env::var("YTERMUSIC_LOG");
    if let Ok(logger_env) = logger_env {
        if logger_env == "true" {
            (LevelFilter::Trace, Level::Trace)
        } else {
            (LevelFilter::Info, Level::Info)
        }
    } else {
        (LevelFilter::Info, Level::Info)
    }
});

pub fn init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LEVEL.0))?;
    info!("Logger mode {}",LEVEL.1);
    return Ok(());
}

use log::{info, Level, LevelFilter, Metadata, Record, SetLoggerError};

static FILTER: &[&str] = &["rustls","tokio-util","want-","mio-"];

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LEVEL.1
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            if FILTER.iter().any(|x| record.file().unwrap().contains(x)) {
                return;
            }
            LOG.send(format!(
                "{} - {} [{}]",
                record.level(),
                record.args(),
                record.file().unwrap_or_default()
            ))
            .unwrap();
        }
    }

    fn flush(&self) {}
}
