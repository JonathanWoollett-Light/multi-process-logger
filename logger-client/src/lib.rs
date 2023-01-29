#![warn(clippy::pedantic)]
#![allow(clippy::needless_pass_by_value)]

use std::{error::Error, io::Write, os::unix::net::UnixStream, sync::Mutex};

use log::{LevelFilter, Metadata, Record};

pub struct Logger {
    stream: Mutex<UnixStream>,
    log_level: LevelFilter,
}

impl Logger {
    /// Initializes the logger.
    ///
    /// Spawns a new server process if it cannot find the socket.
    ///
    /// # Errors
    ///
    /// When failing:
    /// - To spawn the new server process.
    /// - To socket to the server unix socket.
    /// - [`log::set_boxed_logger`].
    pub fn init(socket: &str, log_level: LevelFilter) -> Result<(), Box<dyn Error>> {
        // If socket doesn't exist, boot new server
        if !std::path::Path::new(socket).exists() {
            std::process::Command::new("gnome-terminal")
                .args([
                    "&",
                    "disown",
                    "--",
                    "sh",
                    "-c",
                    &format!("logger-server --socket {socket}; exec bash"),
                    // &format!("cargo run --bin logger-server -- --socket {socket}; exec bash"),
                ])
                .spawn()?;
            // Wait for process to start
            std::thread::sleep(std::time::Duration::from_secs(5));
        }

        let logger = Self {
            stream: Mutex::new(UnixStream::connect(socket)?),
            log_level,
        };
        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(log_level);
        Ok(())
    }
}

#[repr(C)]
struct LogData {
    secs: u64,
    nanos: u32,
    pid: nix::unistd::Pid,
    pthread: nix::sys::pthread::Pthread,
    length: usize,
    level: log::Level,
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let system_time = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap();

            let message = record.args().to_string();
            let message_bytes = message.as_bytes();

            let fixed = LogData {
                secs: system_time.as_secs(),
                nanos: system_time.subsec_nanos(),
                pid: nix::unistd::Pid::this(),
                pthread: nix::sys::pthread::pthread_self(),
                length: message_bytes.len(),
                level: record.level(),
            };
            let array =
                unsafe { std::mem::transmute::<_, [u8; std::mem::size_of::<LogData>()]>(fixed) };

            let bytes = array
                .into_iter()
                .chain(message_bytes.iter().copied())
                .collect::<Vec<_>>();

            self.stream.lock().unwrap().write_all(&bytes).unwrap();
        }
    }

    fn flush(&self) {
        self.stream.lock().unwrap().flush().unwrap();
    }
}
