# Base for planner and builder
FROM rust:1.72.1-bullseye AS chef
WORKDIR /app
ENV RUST_BACKTRACE=full
ENV DATA_ROOT=/data
ENV CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_DEBUG=true
ENV WIKIWALK_ENV=production
ENV WIKIWALK_SKIP_FRONTEND_BUILD=true
ENV RCLONE_GCS_LOCATION="us-central1"
ENV RCLONE_GCS_STORAGE_CLASS="STANDARD"
ENV RCLONE_GCS_CHUNK_SIZE="32M"
ENV RCLONE_GCS_BUCKET_POLICY_ONLY="true"
ENV RCLONE_GCS_OBJECT_ACL="private"
ENV RCLONE_GCS_ENV_AUTH="true"

RUN echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] http://packages.cloud.google.com/apt cloud-sdk main" | \
  tee -a /etc/apt/sources.list.d/google-cloud-sdk.list && \
  curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | \
  apt-key --keyring /usr/share/keyrings/cloud.google.gpg  add -
RUN apt-get update \
  && apt-get install -y google-cloud-cli libssl-dev pkg-config libsqlite3-dev jq rclone && \
  rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-chef --locked

# Plan the build with chef
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path /app/recipe.json

# Build the frontend
FROM node:20-bullseye-slim AS frontend-build
WORKDIR /web
COPY wikiwalk-ui/package.json wikiwalk-ui/package-lock.json ./
RUN npm install
COPY wikiwalk-ui .
RUN npm run build

# Build the executables
FROM chef
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
COPY --from=frontend-build /web/dist wikiwalk-ui/dist
RUN cargo build --release --bin server
RUN cargo build --release --bin tool
RUN cp target/release/tool /tool
RUN cp target/release/server /server
