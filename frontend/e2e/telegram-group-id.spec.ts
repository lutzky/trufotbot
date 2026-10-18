// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect } from './fixtures'

// Larger than Number.MAX_SAFE_INTEGER (2^53 - 1): it would be silently corrupted if it were ever
// converted to a JS number on its way to the server or back.
const HUGE_GROUP_ID = '-1001234567890123456'

// Mirrors the server's TelegramGroupId, which is a decimal *string* on the wire (numbers are
// rejected). Returns an error message if the body would be rejected with a 422.
function validateGroupId(groupId: unknown): string | undefined {
  if (groupId === undefined || groupId === null) return undefined
  if (typeof groupId !== 'string') {
    return `telegram_group_id: invalid type: ${JSON.stringify(groupId)}, expected a string`
  }
  if (
    !/^-?[0-9]+$/.test(groupId) ||
    BigInt(groupId) < -(2n ** 63n) ||
    BigInt(groupId) >= 2n ** 63n
  ) {
    return `telegram_group_id: invalid digit or out of range for i64: ${JSON.stringify(groupId)}`
  }
  return undefined
}

function rejectInvalidGroupId(body: { telegram_group_id?: unknown }) {
  const message = validateGroupId(body.telegram_group_id)
  return message === undefined
    ? undefined
    : {
        status: 422,
        contentType: 'text/plain',
        body: `Failed to deserialize the JSON body into the target type: ${message}`,
      }
}

test.describe('PatientDetailView - Telegram group ID', () => {
  test('can set and then unset the Telegram group ID', async ({ page }) => {
    const patientId = 1
    const store: { name: string; telegram_group_id: string | null } = {
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

        const rejection = rejectInvalidGroupId(body)
        if (rejection) return route.fulfill(rejection)

        store.name = body.name
        store.telegram_group_id = body.telegram_group_id ?? null
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

    await test.step('Verify it was sent as a string', async () => {
      expect(updateBodies.at(-1)).toEqual({ name: 'Alice', telegram_group_id: '-123' })
      expect(store.telegram_group_id).toBe('-123')

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

  test('preserves a group ID too large for a JS number', async ({ page }) => {
    const patientId = 1
    const store: { name: string; telegram_group_id: string | null } = {
      name: 'Alice',
      telegram_group_id: HUGE_GROUP_ID,
    }
    const updateBodies: unknown[] = []

    await page.route(`**/api/patients/${patientId}`, async (route) => {
      const request = route.request()
      if (request.method() === 'GET') {
        // Serialized by hand, as the server does: the ID is a JSON string.
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ ...store, medications: [] }),
        })
      }
      if (request.method() === 'PUT') {
        const body = request.postDataJSON()
        updateBodies.push(body)
        const rejection = rejectInvalidGroupId(body)
        if (rejection) return route.fulfill(rejection)
        store.name = body.name
        store.telegram_group_id = body.telegram_group_id ?? null
        return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
      }
      return route.fulfill({ status: 404, body: '{}' })
    })

    await page.goto(`/patients/${patientId}`)
    await page.locator('summary', { hasText: 'Edit patient' }).click()

    const groupIdInput = page.getByLabel('Telegram Group ID')
    await expect(groupIdInput).toHaveValue(HUGE_GROUP_ID)

    // Saving without touching the ID must send back exactly what was loaded.
    await page.getByLabel('Name', { exact: true }).fill('Alice Renamed')
    await page.getByRole('button', { name: 'Save' }).click()
    await expect(page.getByRole('button', { name: 'Saved' })).toBeVisible()

    expect(updateBodies.at(-1)).toEqual({
      name: 'Alice Renamed',
      telegram_group_id: HUGE_GROUP_ID,
    })
    expect(store.telegram_group_id).toBe(HUGE_GROUP_ID)
  })
})
