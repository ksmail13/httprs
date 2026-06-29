# AGENT

## Overview

`httprs` is a lightweight HTTP server. 
It aims to provide predictable HTTP server behavior without requiring external runtimes like tokio or async-std yet. 

## Key Components

### 1. `Server` (`server/mod.rs`)

* Accepts a list of `WorkerInfo` objects.
* Sets socket options (REUSEADDR, REUSEPORT, RECEIVE_TIMEOUT).
* Builds a list of `WorkerGroup` instances, each of which will spawn `worker` processes.
* Starts the `WorkerManager` that manages all child processes.

### 2. `WorkerManager` (`worker/manager.rs`)

* Handles `SIGINT` to gracefully terminate all workers.
* Uses `wait()` to reap terminated child processes.
* Re‑spawns workers immediately when they exit to maintain the configured pool size.

### 3. `WorkerGroup` (`worker/group.rs`)

* Encapsulates the worker type (`TcpWorker`) and the desired count.

### 4. `Worker` Trait (`worker/mod.rs`)

```rust
pub trait Worker {
    fn init(&self);
    fn run(&self);
    fn cleanup(&self);
}
```

### 5. `Http1` Process (`http/http.rs`)

* Implements the `Process` trait for handling HTTP/1.x requests.
* Parses request line, headers, and query parameters.
* Uses a `Handler` implementation to generate a response.

### 6. `Handler` Trait (`http/handler.rs`)

```rust
pub trait Handler {
    fn handle(&self, req: &mut HttpRequest, res: &mut HttpResponse);
}
```

The `SimpleHandler` in `main.rs` is a minimal example that echoes request headers back.

### 7. `EchoProcess` (`process/echo.rs`)

* A simple echo server used in tests.
* Demonstrates how to implement a custom `Process`.

## Running the Server

```bash
# Build
cargo build --release

# Run
./target/release/server_rs --host 127.0.0.1 --port 8080 --worker 4 --timeout-ms 2000
```

The above command starts a server listening on `127.0.0.1:8080` with 4 preforked worker processes and a 2‑second accept timeout.

## Extending the Server

1. **Custom Handler** – Implement the `Handler` trait and pass it to `Http1::new()`.
2. **Custom Process** – Implement the `Process` trait (e.g., a WebSocket server).
3. **Worker Customization** – Replace `TcpWorker` with a UDP worker or add TLS support.

## Tests

The repository contains a few unit tests:

```bash
cargo test
```

* `http/http.rs` tests URL parsing.
* `process/echo.rs` tests the echo server logic.
* `worker/manager.rs` contains integration tests for the worker manager.

## Design Rationale

* **Prefork Model** – Simplifies concurrent connection handling without sharing state between processes.
* **Dynamic Dispatch** – `WorkerGroup` holds a `Rc<dyn Worker>` to allow different worker types.
* **Signal Handling** – `WorkerManager` traps `SIGINT` to shutdown cleanly.
* **Timeouts** – `setsockopt(ReceiveTimeout)` prevents workers from blocking indefinitely on `accept`.

## Dependencies

* `clap` – CLI argument parsing.
* `colog` – Structured logging.
* `nix` – Low‑level Unix syscalls.
* `env_logger` – Log initialization.
