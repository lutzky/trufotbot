// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test as base, expect } from '@playwright/test'

export const test = base.extend({
  page: async ({ page }, use) => {
    // Catch-all guardrail for unmocked API calls:
    // Returns 500 and logs an error rather than leaking requests to http://localhost:3000
    await page.route('**/api/**', async (route) => {
      console.error(
        `[E2E] Unmocked API request: ${route.request().method()} ${route.request().url()}`,
      )
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error: `Unmocked route in E2E test: ${route.request().url()}` }),
      })
    })

    // Default mock for /api/status so HomeView can render server status without contacting backend
    await page.route('**/api/status', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          server_time: new Date().toISOString(),
          timezone: 'UTC',
        }),
      })
    })

    await use(page)
  },
})

export { expect }
