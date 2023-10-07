# Build the frontend
FROM node:20-bookworm-slim AS frontend-build
WORKDIR /web
COPY wikiwalk-ui/package.json wikiwalk-ui/package-lock.json ./
RUN npm install
COPY wikiwalk-ui .
RUN npm run build

FROM us-central1-docker.pkg.dev/supervillains/supervillains/wikiwalk-build AS builder
WORKDIR /app
ENV DATA_ROOT=/data
ENV WIKIWALK_ENV=production
ENV WIKIWALK_SKIP_FRONTEND_BUILD=true
COPY --from=frontend-build /web/dist /app/wikiwalk-ui/dist
RUN cargo build --release --bin server
RUN cp target/release/server /server

FROM debian:bullseye-slim
WORKDIR /app
ENV DATA_ROOT=/data
ENV WIKIWALK_ENV=production
ENV RUST_BACKTRACE=full
COPY --from=us-central1-docker.pkg.dev/supervillains/supervillains/wikiwalk-data /data /data
COPY --from=builder /app/target/release/server server
CMD [ "/server" ]
