<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

# Regenerating the API Client

The frontend and backend communicate via an API defined in the backend's Rust
code. The API is defined using `utoipa` macros. The main definition is in the
`ApiDoc` struct in `server/src/main.rs`, and the various types and handlers are
elsewhere in the codebase. When you make changes to that affect the API, you
must regenerate the frontend's TypeScript client.

To do this, simply run:

```shell
just api-update
```

This command will:

1. Build and run a small part of the backend to generate an updated
   `trufotbot-openapi.json` schema file.
2. Run the frontend's code generation script to create updated TypeScript
   client code based on the new schema.

After regeneration, it's a good idea to run the frontend's type checker and
tests to ensure the new API client is integrated correctly. You may need to
update frontend code to match the new API.

```shell
just frontend-check
```
