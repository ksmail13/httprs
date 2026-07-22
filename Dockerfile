FROM rust:trixie AS builder

WORKDIR /root/httprs

COPY Cargo.toml .
COPY httprs/  httprs/
COPY httprs-bin/  httprs-bin/

RUN cargo build --profile release

FROM debian:trixie

RUN useradd -m runner
USER runner

COPY --from=builder /root/httprs/target/release/httprs-bin /home/runner/httprs-bin

ENTRYPOINT ["/home/runner/httprs-bin"]
