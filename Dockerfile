FROM docker.io/library/rust:1.90-alpine3.22 AS builder

RUN apk add --no-cache musl-dev
WORKDIR /root/httprs

COPY Cargo.toml .
COPY httprs/  httprs/
COPY httprs-bin/  httprs-bin/

RUN cargo build --profile release

FROM docker.io/library/rockylinux:8

RUN useradd -m runner
USER runner

COPY --from=builder /root/httprs/target/release/httprs-bin /home/runner/httprs-bin

ENTRYPOINT ["/home/runner/httprs-bin"]