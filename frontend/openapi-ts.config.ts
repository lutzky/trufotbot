// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { defineConfig } from '@hey-api/openapi-ts';

export default defineConfig({
  input: './trufotbot-openapi.json',
  output: 'src/openapi',
  plugins: [
    {
      name: '@hey-api/client-fetch',
      throwOnError: true,
    },
    {
      name: '@hey-api/transformers',
      dates: true,
    },
    {
      name: '@hey-api/sdk',
      transformer: true,
    },
  ]
});
