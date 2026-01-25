FROM rust:latest AS build

WORKDIR /home/code

ADD ./src/ ./src/
ADD Cargo.toml .
ADD Cargo.lock .
ADD rust-toolchain.toml .

RUN cargo build --release

FROM rust:latest

RUN apt-get install -y curl
HEALTHCHECK --interval=30s --timeout=3s CMD curl -f http://localhost:8080/ || exit 1

WORKDIR /srv
COPY --from=build /home/code/target/release/cubic_storage /srv/cubic_storage

CMD ["/srv/cubic_storage"]
