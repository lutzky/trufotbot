// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect } from '@playwright/test'

test('username warning appears and disappears', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.clear()
  })

  const mockPatients = [
    { id: 1, name: 'Alice' },
  ]
  await page.route('**/api/patients', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(mockPatients),
    })
  })

  const mockPatient = {
    id: 1,
    name: 'Alice',
    telegram_group_id: null,
    medications: [],
  }
  await page.route('**/api/patients/1', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(mockPatient),
    })
  })

  await page.goto('/')

  const warning = page.locator('article:has-text("Warning: No user set")')
  await expect(warning).toBeVisible()

  const usernameInput = page.locator('#username')
  await usernameInput.fill('TestUser')

  await expect(warning).toBeHidden()

  await usernameInput.fill('')

  await expect(warning).toBeVisible()

  await page.goto('/patients/1')

  await expect(warning).toBeVisible()

  await page.goBack()

  await expect(warning).toBeVisible()
})
