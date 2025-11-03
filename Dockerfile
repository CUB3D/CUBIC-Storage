FROM rust:latest

# Add our source code.
ADD ./src/ ./src/
ADD Cargo.toml .
ADD Cargo.lock .
ADD rust-toolchain.toml .

RUN cargo build --release

HEALTHCHECK --interval=30s --timeout=3s CMD curl -f http://localhost:8080/ || exit 1

CMD ["cargo", "run", "--release"]
