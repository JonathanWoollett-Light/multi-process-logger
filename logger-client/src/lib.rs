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
                    &format!("cargo run --bin logger-server -- --socket {socket}; exec bash"),
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

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.log_level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut guard = self.stream.lock().unwrap();

            let pid = nix::unistd::Pid::this();
            let pid_slice = pid.as_raw().to_ne_bytes();

            let pthread_id = nix::sys::pthread::pthread_self();
            let pthread_id_slice = pthread_id.to_ne_bytes();

            let string = format!("{} {}", record.level(), record.args());
            let string_bytes = string.as_bytes();
            let string_slice = [&string_bytes.len().to_ne_bytes(), string_bytes].concat();

            let bytes = pid_slice
                .into_iter()
                .chain(pthread_id_slice.into_iter())
                .chain(string_slice.into_iter())
                .collect::<Vec<_>>();
            guard.write_all(&bytes).unwrap();
        }
    }

    fn flush(&self) {
        self.stream.lock().unwrap().flush().unwrap();
    }
}
