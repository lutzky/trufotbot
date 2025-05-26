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
# TODO This isn't cached by cargo chef for some reason.
RUN just release_frontend
ENV DATABASE_URL=sqlite:build.db
# TODO There's no need to actually seed the DB, just run the migrations. Then we
# can avoid setting TELEGRAM_GROUP_ID.
ENV TELEGRAM_GROUP_ID=0
RUN just reset_db
RUN cargo build --release --bin trufotbot

FROM debian:bookworm-slim AS runtime
WORKDIR /trufotbot
RUN apt-get update && \
	apt-get -y install libssl-dev ca-certificates tzdata && \
	rm -rf /var/lib/apt/lists/*
COPY --from=builder /trufotbot/target/release/trufotbot /usr/local/bin

EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/trufotbot"]
CMD ["--host", "0.0.0.0"]
