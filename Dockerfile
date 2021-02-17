FROM rust:latest

# Add our source code.
ADD ./src/ ./src/
ADD Cargo.toml .

RUN cargo build --release

CMD ["cargo", "run", "--release"]
