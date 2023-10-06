# Base for planner and builder
FROM rust:1.72.1-bookworm AS chef
WORKDIR /app
ENV RUST_BACKTRACE=full
ENV DATA_ROOT=/data
ENV CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_DEBUG=true
RUN apt-get update && apt-get install -y libssl-dev pkg-config libsqlite3-dev nodejs npm
RUN cargo install cargo-chef --locked

# Plan the build with chef
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Build the frontend
FROM node:20-bookworm-slim AS frontend-build
WORKDIR /web
COPY wikiwalk-ui/package.json wikiwalk-ui/package-lock.json ./
RUN npm install
COPY wikiwalk-ui .
RUN npm run build

# Build the tool and server
FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
COPY --from=frontend-build /web/dist /app/wikiwalk-ui/dist
RUN cargo build --release --bin tool
RUN cargo build --release --bin server

# Build the data to bake into the image
FROM builder AS data-builder
ENV DATA_ROOT=/data
ENV RUST_BACKTRACE=1
COPY --from=builder /app/target/release/tool /tool
RUN /tool pull

# Final image
FROM builder
ENV DATA_ROOT=/data
ENV RUST_BACKTRACE=1
ENV PORT 8080
COPY --from=data-builder /data /data
COPY --from=builder /app/target/release/server /server
CMD [ "/server" ]
