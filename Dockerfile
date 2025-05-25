# Stage 1: Builder
FROM rust:latest AS chef

RUN cargo install cargo-chef

WORKDIR /trufotbot

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /trufotbot/recipe.json recipe.json
RUN rustup target add wasm32-unknown-unknown
RUN cargo chef cook --release --recipe-path recipe.json
RUN cargo install just --version 1.40.0
RUN cargo install trunk --version 0.21.13
RUN cargo install sqlx-cli --version 0.8.5

COPY . .
RUN just release_frontend
# TODO: Look at cargo sqlx prepare, consider using that instead
ENV DATABASE_URL=sqlite:build.db
ENV TELEGRAM_GROUP_ID=0
RUN just reset_db
RUN cargo build --release --bin trufotbot

FROM debian:bookworm-slim AS runtime
WORKDIR /trufotbot
RUN apt-get update && apt-get -y install libssl-dev
COPY --from=builder /trufotbot/target/release/trufotbot /usr/local/bin

# Command to run the application
ENTRYPOINT ["/usr/local/bin/trufotbot"]
