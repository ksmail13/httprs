# httprs

`httprs` is a lightweight HTTP server. 
It aims to provide predictable HTTP server behavior without requiring external runtimes like tokio or async-std.

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
