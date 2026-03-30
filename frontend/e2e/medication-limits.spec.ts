// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect } from '@playwright/test'

test.describe('Entering limits', () => {
  test('PatientMedicationDetailView - Entering limits, including the comma', async ({ page }) => {
    const mockDosesResponse = {
      patient_name: 'Alice',
      medication: { name: 'Aspirin', description: 'Pain reliever', dose_limits: [] },
      reminders: { cron_schedules: [] },
      doses: [],
      next_doses: [],
    }

    await page.route('**/api/patients/1/medications/1/doses', async (route) => {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(mockDosesResponse),
      })
    })

    await test.step('Navigate to medication detail page', async () => {
      await page.goto('/patients/1/medications/1')
      await page.waitForLoadState('domcontentloaded')
    })

    await test.step('Open edit medication form', async () => {
      await page
        .locator('details:has(summary:text("Edit medication"))')
        .evaluate((el: HTMLDetailsElement) => (el.open = true))
    })

    const limitsInput = page.getByRole('textbox', { name: 'Limits' })

    await test.step('Find limits input', async () => {
      await expect(limitsInput).toBeVisible()
    })

    await test.step('Input limits "12:34,"', async () => {
      await limitsInput.focus()
      await limitsInput.clear()
      await limitsInput.pressSequentially('12:34,', { delay: 10 })
    })

    await test.step('Should count as invalid, and have trailing comma', async () => {
      await expect(limitsInput).toHaveValue('12:34,')
      await expect(limitsInput).toHaveAttribute("aria-invalid", "true")
    })

    await test.step('Further input "56:78"', async () => {
      await limitsInput.pressSequentially('56:78', { delay: 10 })
    })

    await test.step('Should count as valid, and have full "12:34,56:78"', async () => {
      await expect(limitsInput).toHaveValue('12:34,56:78')
      await expect(limitsInput).toHaveAttribute("aria-invalid", "false")
    })
  })
})
