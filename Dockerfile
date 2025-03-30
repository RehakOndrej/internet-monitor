FROM rust:1.81 as builder

WORKDIR /usr/src/internet-monitor
COPY Cargo.toml .
# Create a dummy main.rs to build dependencies
RUN mkdir -p src && echo 'fn main() { println!("Dummy"); }' > src/main.rs
RUN cargo build --release
# Remove the dummy build artifacts
RUN rm -rf src && rm -f target/release/internet-monitor target/release/deps/internet_monitor*

# Copy the real source code and build again
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl-dev \
    iputils-ping \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/internet-monitor/target/release/internet-monitor /usr/local/bin/internet-monitor

ENTRYPOINT ["internet-monitor"]