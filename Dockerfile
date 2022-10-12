FROM rust:latest

# Add our source code.
ADD ./src/ ./src/
ADD Cargo.toml .
ADD Cargo.lock .

RUN cargo build --release

CMD ["cargo", "run", "--release"]
