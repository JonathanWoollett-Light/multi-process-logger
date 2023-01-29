# multi-process-logger

A logger implementation for multi-process systems.

It has only been tested on Linux.

![server terminal example](server.png)

### Setup

Install the logger server:

```rust
cargo install logger-server --git https://github.com/JonathanWoollett-Light/multi-process-logger.git
```

Add the logger client dependency to your project:

```rust
logger-client = { git = "https://github.com/JonathanWoollett-Light/multi-process-logger.git", rev="d64be88108e56fbf908efbdf8a94ae53fa6959f8" }
```

Within your project initialize the logger with:

```rust
Logger::init("/tmp/my-unix-socket", LevelFilter::Debug).unwrap();
```

### Example

```rust
use log::LevelFilter;
use logger_client::Logger;
use std::thread::sleep;
use std::time::Duration;

const SPACING: Duration = Duration::from_millis(100);
fn main() {
    Logger::init("./a-local-socket", LevelFilter::Debug).unwrap();

    let handles = (0..10)
        .map(|_| std::thread::spawn(tester))
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
}

fn tester() {
    for _ in 0..5 {
        log::trace!("test trace");
        sleep(SPACING);
        log::debug!("test debug");
        sleep(SPACING);
        log::info!("test info");
        sleep(SPACING);
        log::warn!("test warn");
        sleep(SPACING);
        log::error!("test error");
        sleep(SPACING);
    }
}
```

This will spawn the server process if the socket is not found, otherwise it will attempt to connect to the socket.

### Server control

- `q` Exit
- `w` Up process
- `s` Down process
- `e` Up thread
- `d` Down thread
- `r` Up log
- `f` Down log
