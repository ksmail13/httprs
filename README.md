# httprs

`httprs` is a lightweight HTTP server written in Rust that demonstrates a **Prefork Multi‑Process Model** similar to the classic Apache httpd MPM Prefork.
It spawns a configurable number of worker processes that listen on a single TCP port and accept connections concurrently.

## Features

- **Prefork**: Spawns multiple worker processes at start‑up, each with its own event loop.
- **Extensible Architecture**:
  - `Worker` trait lets you plug in different worker types (TCP, UDP, TLS, etc.).
  - `Process` trait allows you to implement any protocol (HTTP, Echo, WebSocket, etc.).
  - `Handler` trait for routing HTTP requests to custom logic.

## Project Layout

```
httprs/
├── src/
│   ├── args.rs              # CLI arguments
│   ├── main.rs              # Application entry point
│   ├── server/
│   │   └── mod.rs           # Server orchestration
│   ├── worker/
│   │   ├── error.rs         # Worker errors
│   │   ├── group.rs         # WorkerGroup abstraction
│   │   ├── helper.rs        # Forking / cleanup utilities
│   │   ├── manager.rs       # Manager that keeps child processes alive
│   │   ├── mod.rs           # Public worker trait
│   │   └── tcp.rs           # TCP worker implementation
│   ├── http/
│   │   ├── header.rs        # Header utilities
│   │   ├── http.rs          # `Http1` process implementation
│   │   ├── request.rs       # `HttpRequest` type
│   │   ├── response.rs      # `HttpResponse` type
│   │   ├── value.rs         # HTTP enums & errors
│   │   └── mod.rs
│   └── process/
│       ├── echo.rs          # Example Echo process
│       └── mod.rs
├── Cargo.toml
├── Cargo.lock
└── README.md
```

## Getting Started

### Build

```sh
cargo build --release
Run

```sh
./target/release/httprs \
  --host 127.0.0.1 \
  --port 8080 \
  --worker 4 \
  --timeout-ms 2000 \
  --max-header-size 8192
```

This command starts a HTTP server listening on `127.0.0.1:8080` with 4 preforked worker processes and a 2‑second accept timeout.

## Extending the Server

1. **Custom Handler** – Implement the `Handler` trait and pass it to `Http1::new()`.  
2. **Custom Process** – Implement the `Process` trait (e.g., a WebSocket server).  
3. **Worker Customization** – Replace `TcpWorker` with a UDP worker or add TLS support.

## Tests

Run the test suite with:

```sh
cargo test
```

Tests cover:

- URL parsing (`http/http.rs`)
- Echo server logic (`process/echo.rs`)
- Worker manager integration (`worker/manager.rs`)
