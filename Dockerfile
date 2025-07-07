# Stage 1: Builder
FROM rust:latest AS chef

RUN cargo install cargo-chef

WORKDIR /trufotbot

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json
RUN sh -c "cd frontend; cargo chef prepare --recipe-path recipe_wasm.json"

FROM chef AS builder
COPY --from=planner /trufotbot/recipe.json recipe.json
COPY --from=planner /trufotbot/frontend/recipe_wasm.json frontend/recipe_wasm.json

# Note: If adding anything here, also add to README.md
RUN sh -c "curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash"
RUN rustup target add wasm32-unknown-unknown
RUN cargo binstall just --version 1.40.0
RUN cargo binstall trunk --version 0.21.13
RUN cargo binstall sqlx-cli --version 0.8.5

# Everything beyond this point may need to be rebuilt if source changes, so put
# expensive operations earlier if possible for improved caching.
RUN cargo chef cook --release --recipe-path recipe.json
RUN sh -c "cd frontend; CARGO_TARGET_DIR=../target cargo chef cook --profile wasm-release --target wasm32-unknown-unknown --recipe-path recipe_wasm.json"

COPY . .
ENV DATABASE_URL=sqlite:build.db
RUN just reset_db
RUN just release_frontend
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
