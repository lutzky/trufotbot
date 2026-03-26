// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect } from '@playwright/test'

test.describe('DoseEditView - Non-integer dose values', () => {
  test('should update dose quantity to non-integer value (1.5)', async ({ page }) => {
    const patientId = 1
    const medicationId = 1
    const doseId = 1

    const mockDoseResponse = {
      dose: {
        id: doseId,
        data: {
          quantity: 1,
          taken_at: new Date('2024-01-15T08:00:00'),
          noted_by_user: null,
        },
      },
      inventory: null,
      medication_name: 'Aspirin',
      patient_name: 'Alice',
    }

    let lastUpdateData: { quantity: number } | null = null

    await page.route(`**/api/patients/${patientId}/medications/${medicationId}/doses/${doseId}`, async (route) => {
      const request = route.request()
      const method = request.method()

      if (method === 'GET') {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(mockDoseResponse),
        })
      }
      if (method === 'PUT') {
        lastUpdateData = request.postDataJSON()
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(mockDoseResponse),
        })
      }
      return route.fulfill({ status: 404, body: '{}' })
    })

    await test.step('Navigate to dose edit page', async () => {
      await page.goto(`/patients/${patientId}/medications/${medicationId}/doses/${doseId}`)
      await page.waitForLoadState('domcontentloaded')
    })

    await test.step('Verify initial quantity is 1', async () => {
      const quantityInput = page.locator('input[name="quantity"]')
      await expect(quantityInput).toHaveValue('1')
    })

    await test.step('Update quantity to 1.5', async () => {
      const quantityInput = page.locator('input[name="quantity"]')
      await quantityInput.focus()
      await quantityInput.fill('1.5')
    })

    await test.step('Click Save button', async () => {
      await page.getByRole('button', { name: 'Save' }).click()
    })

    await test.step('Verify save was successful', async () => {
      await expect(page.getByRole('button', { name: 'Saved' })).toBeVisible()
    })

    await test.step('Verify API was called with quantity 1.5', async () => {
      await expect
        .poll(async () => {
          return lastUpdateData
        })
        .not.toBeNull()
      expect(lastUpdateData!.quantity).toBe(1.5)
    })
  })
})
