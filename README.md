# multi-process-logger

A logger implementation for multi-process systems.

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

This will spawn the server process if the socket is not found, otherwise it will attempt to connect to the socket.

### Server control

- `q` Exit
- `w` Up process
- `s` Down process
- `e` Up thread
- `d` Down thread
- `r` Up log
- `f` Down log