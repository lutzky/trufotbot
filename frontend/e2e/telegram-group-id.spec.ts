// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect } from './fixtures'

test.describe('PatientDetailView - Telegram group ID', () => {
  test('can set and then unset the Telegram group ID', async ({ page }) => {
    const patientId = 1
    const store: { name: string; telegram_group_id: number | null } = {
      name: 'Alice',
      telegram_group_id: null,
    }
    const updateBodies: unknown[] = []

    await page.route(`**/api/patients/${patientId}`, async (route) => {
      const request = route.request()
      if (request.method() === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ ...store, medications: [] }),
        })
      }
      if (request.method() === 'PUT') {
        const body = request.postDataJSON()
        updateBodies.push(body)

        // Mirrors the server's PatientCreateRequest: name: String, telegram_group_id: Option<i64>
        const groupId = body.telegram_group_id
        if (groupId !== undefined && groupId !== null && !Number.isInteger(groupId)) {
          return route.fulfill({
            status: 422,
            contentType: 'text/plain',
            body: `Failed to deserialize the JSON body into the target type: telegram_group_id: invalid type: ${JSON.stringify(groupId)}, expected i64`,
          })
        }
        store.name = body.name
        store.telegram_group_id = groupId ?? null
        return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
      }
      return route.fulfill({ status: 404, body: '{}' })
    })

    const groupIdInput = page.getByLabel('Telegram Group ID')

    async function openEditPatient() {
      await page.locator('summary', { hasText: 'Edit patient' }).click()
    }

    await test.step('Open the patient page with no group set', async () => {
      await page.goto(`/patients/${patientId}`)
      await openEditPatient()
      await expect(groupIdInput).toHaveValue('')
    })

    await test.step('Set the Telegram group ID', async () => {
      await groupIdInput.fill('-123')
      await page.getByRole('button', { name: 'Save' }).click()
      await expect(page.getByRole('button', { name: 'Saved' })).toBeVisible()
    })

    await test.step('Verify it was set', async () => {
      expect(updateBodies.at(-1)).toEqual({ name: 'Alice', telegram_group_id: -123 })
      expect(store.telegram_group_id).toBe(-123)

      await page.reload()
      await openEditPatient()
      await expect(groupIdInput).toHaveValue('-123')
    })

    await test.step('Unset the Telegram group ID', async () => {
      await groupIdInput.fill('')
      await page.getByRole('button', { name: 'Save' }).click()
      await expect(page.getByRole('button', { name: 'Saved' })).toBeVisible()
    })

    await test.step('Verify it was unset', async () => {
      expect(updateBodies.at(-1)).toEqual({ name: 'Alice', telegram_group_id: null })
      expect(store.telegram_group_id).toBeNull()

      await page.reload()
      await openEditPatient()
      await expect(groupIdInput).toHaveValue('')
    })
  })
})
