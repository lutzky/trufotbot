<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Release Building

## Direct Build

```shell
just release_frontend
cargo build --release --bin trufotbot
```

Your output binary is now in `target/release/trufotbot`. It's a self-contained binary.

## Docker Build

Docker builds are entirely containerized, meaning that your local build system
should not affect them. They take longer as a result, but some caching is
performed for subsequent builds.

```shell
docker build . -t trufotbot:latest
```

To run this build, use the same `docker-compose.yml` from [Getting
started](getting-started.md), but use `image: trufotbot:latest`.
