# Base for planner and builder
FROM rust:1.72.1-bookworm AS chef
WORKDIR /app
ENV RUST_BACKTRACE=1
ENV DATA_ROOT=/data
RUN apt-get update && apt-get install -y libssl-dev pkg-config libsqlite3-dev
RUN cargo install cargo-chef --locked

# Plan the build with chef
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json


# Build the tool and server
FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin tool
RUN cargo build --release --bin server

# Build the data to bake into the image
FROM scratch AS data-builder
COPY --from=builder /app/target/release/tool /tool
ENV DATA_ROOT=/data
ENV RUST_BACKTRACE=1
RUN /tool pull

# Final image
FROM scratch
ENV DATA_ROOT=/data
ENV RUST_BACKTRACE=1
ENV PORT 8080
COPY --from=data-builder /data /data
COPY --from=builder /app/target/release/server /wikiwalk-server
CMD [ "/wikiwalk-server" ]
