// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { test, expect, type Page } from './fixtures'

const PATIENT_ID = 1
const MEDICATION_ID = 1

interface MockDosesBody {
  patient_name: string
  medication: {
    name: string
    description: string
    aliases: Array<string>
  }
  dose_limits: Array<{ hours: number; amount: number }>
  other_patients_dose_limits: Array<{
    patient_name: string
    dose_limits: Array<{ hours: number; amount: number }>
  }>
  reminders: { cron_schedules: Array<string> }
  natural_input: boolean
  doses: Array<unknown>
  next_doses: Array<unknown>
}

function makeDosesResponse(
  doseLimits: Array<{ hours: number; amount: number }> = [],
  otherPatientsDoseLimits: MockDosesBody['other_patients_dose_limits'] = [],
): MockDosesBody {
  return {
    patient_name: 'Alice',
    medication: {
      name: 'Aspirin',
      description: 'Pain reliever',
      aliases: [],
    },
    dose_limits: doseLimits,
    other_patients_dose_limits: otherPatientsDoseLimits,
    reminders: { cron_schedules: [] },
    natural_input: false,
    doses: [],
    next_doses: [],
  }
}

async function setupPage(
  page: Page,
  doseLimits: Array<{ hours: number; amount: number }> = [],
  otherPatientsDoseLimits: MockDosesBody['other_patients_dose_limits'] = [],
) {
  const response = makeDosesResponse(doseLimits, otherPatientsDoseLimits)
  await page.route(
    `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}/doses`,
    async (route) => {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(response),
      })
    },
  )
}

async function openEditForm(page: Page) {
  await page.goto(`/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`)
  await page.waitForLoadState('domcontentloaded')
  await page
    .locator('details:has(summary:text("Edit medication"))')
    .evaluate((el: HTMLDetailsElement) => (el.open = true))
}

function limitsInput(page: Page) {
  return page.getByRole('textbox', { name: 'Limits' })
}

function saveButton(page: Page) {
  return page.getByRole('button', { name: 'Save' })
}

async function setCursorPosition(input: ReturnType<typeof limitsInput>, pos: number) {
  await input.evaluate((el: HTMLInputElement, p: number) => {
    el.setSelectionRange(p, p)
  }, pos)
}

test.describe('Entering limits', () => {
  test('PatientMedicationDetailView - Entering limits, including the comma', async ({ page }) => {
    await setupPage(page)
    await openEditForm(page)

    const input = limitsInput(page)

    await test.step('Find limits input', async () => {
      await expect(input).toBeVisible()
    })

    await test.step('Input limits "12:34,"', async () => {
      await input.focus()
      await input.clear()
      await input.pressSequentially('12:34,', { delay: 10 })
    })

    await test.step('Should count as invalid, and have trailing comma', async () => {
      await expect(input).toHaveValue('12:34,')
      await expect(input).toHaveAttribute('aria-invalid', 'true')
    })

    await test.step('Further input "56:7.8"', async () => {
      await input.pressSequentially('56:7.', { delay: 10 })
      await expect(input).toHaveValue('12:34,56:7.')
      await expect(input).toHaveAttribute('aria-invalid', 'false')
      await input.pressSequentially('8', { delay: 10 })
      await expect(input).toHaveValue('12:34,56:7.8')
      await expect(input).toHaveAttribute('aria-invalid', 'false')
    })
  })
})

test.describe('Issue #88: Limits input bugs', () => {
  // https://github.com/lutzky/trufotbot/issues/88
  test.describe('Double period destruction', () => {
    test('typing a period where one already exists does not erase text', async ({ page }) => {
      await setupPage(page, [{ hours: 12, amount: 3.5 }])
      await openEditForm(page)

      const input = limitsInput(page)
      await expect(input).toHaveValue('12:3.5')

      await input.focus()
      await setCursorPosition(input, 4) // between '3' and '.'
      await input.pressSequentially('.', { delay: 10 })

      await expect(input).toHaveValue('12:3..5')
      await expect(input).toHaveAttribute('aria-invalid', 'true')
    })
  })

  test.describe('Adding a period to non-last limit', () => {
    test('typing a decimal point before a comma works', async ({ page }) => {
      await setupPage(page)
      await openEditForm(page)

      const input = limitsInput(page)
      await input.focus()
      await input.fill('1:1,2:2')

      await setCursorPosition(input, 3) // between '1' and ','
      await input.pressSequentially('.5', { delay: 10 })

      await expect(input).toHaveValue('1:1.5,2:2')
      await expect(input).toHaveAttribute('aria-invalid', 'false')
    })
  })

  test.describe('Leading period', () => {
    test('typing "." after colon inserts period without rewriting to "0."', async ({ page }) => {
      await setupPage(page)
      await openEditForm(page)

      const input = limitsInput(page)
      await input.focus()
      await input.fill('1:1')

      await setCursorPosition(input, 2) // after ':'
      await input.pressSequentially('.', { delay: 10 })

      await expect(input).toHaveValue('1:.1')
      await expect(input).toHaveAttribute('aria-invalid', 'false')
    })
  })

  test.describe('Save button dirty tracking', () => {
    test('save button toggles correctly and sends correct body', async ({ page }) => {
      await setupPage(page)
      await openEditForm(page)

      const input = limitsInput(page)
      const btn = saveButton(page)

      let savedBody: unknown = null
      let putCount = 0
      await page.route(
        `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`,
        async (route) => {
          if (route.request().method() === 'PUT') {
            savedBody = route.request().postDataJSON()
            putCount += 1
            return route.fulfill({
              status: 200,
              contentType: 'application/json',
              body: JSON.stringify({}),
            })
          }
          return route.fulfill({ status: 404 })
        },
      )

      await test.step('Initially disabled (no changes)', async () => {
        await expect(btn).toBeDisabled()
      })

      await test.step('Enabled after typing valid limits', async () => {
        await input.focus()
        await input.fill('12:3.5')
        await expect(btn).toBeEnabled()
      })

      await test.step('Disabled after clicking save, body matches input', async () => {
        await btn.click()
        await expect(btn).toBeDisabled()
        expect(putCount).toBe(1)
        expect(savedBody).toEqual({
          medication: {
            name: 'Aspirin',
            description: 'Pain reliever',
            aliases: [],
          },
          dose_limits: [{ hours: 12, amount: 3.5 }],
          reminders: { cron_schedules: [] },
          natural_input: false,
        })
      })
    })

    test('save button disabled when limits become invalid, re-enabled when fixed', async ({
      page,
    }) => {
      await setupPage(page)
      await openEditForm(page)

      const input = limitsInput(page)
      const btn = saveButton(page)

      await test.step('Type valid "12:34" → enabled', async () => {
        await input.focus()
        await input.pressSequentially('12:34', { delay: 10 })
        await expect(btn).toBeEnabled()
      })

      await test.step('Type trailing comma "12:34," → disabled', async () => {
        await input.pressSequentially(',', { delay: 10 })
        await expect(btn).toBeDisabled()
      })

      await test.step('Backspace to "12:34" → enabled again', async () => {
        await page.keyboard.press('Backspace')
        await expect(btn).toBeEnabled()
      })
    })
  })

  test.describe('Save leading period without blur', () => {
    test('edit "1:1" → "1:.1" saves amount 0.1 not 1', async ({ page }) => {
      await setupPage(page, [{ hours: 1, amount: 1 }])
      await openEditForm(page)

      const nameInput = page.getByPlaceholder('Medication name')
      const limits = limitsInput(page)
      const btn = saveButton(page)

      const savedBodies: Array<unknown> = []
      let putCount = 0
      await page.route(
        `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`,
        async (route) => {
          if (route.request().method() === 'PUT') {
            putCount += 1
            savedBodies.push(route.request().postDataJSON())
            return route.fulfill({
              status: 200,
              contentType: 'application/json',
              body: JSON.stringify({}),
            })
          }
          return route.fulfill({ status: 404 })
        },
      )

      await test.step('Initially shows "1:1"', async () => {
        await expect(limits).toHaveValue('1:1')
      })

      await test.step('Dirty form with a name change, then save sends amount 1', async () => {
        await nameInput.focus()
        await nameInput.press('End')
        await nameInput.pressSequentially('!', { delay: 10 })
        await btn.click()
        expect(putCount).toBe(1)
        expect(savedBodies[0]).toEqual({
          medication: {
            name: 'Aspirin!',
            description: 'Pain reliever',
            aliases: [],
          },
          dose_limits: [{ hours: 1, amount: 1 }],
          reminders: { cron_schedules: [] },
          natural_input: false,
        })
        // Wait for saveMedication's finally block to update
        // originalMedicationJson before we modify the model, otherwise
        // the finally block captures the new state and isDirty stays false.
        await expect(btn).not.toHaveAttribute('aria-busy', 'true')
      })

      await test.step('Type "." after colon → shows "1:.1" (not "1:0.1")', async () => {
        await limits.focus()
        await setCursorPosition(limits, 2)
        await limits.pressSequentially('.', { delay: 10 })
        await expect(limits).toHaveValue('1:.1')
      })

      await test.step('Save again sends amount 0.1', async () => {
        await expect(btn).toBeEnabled()
        await btn.click()
        expect(putCount).toBe(2)
        expect(savedBodies[1]).toEqual({
          medication: {
            name: 'Aspirin!',
            description: 'Pain reliever',
            aliases: [],
          },
          dose_limits: [{ hours: 1, amount: 0.1 }],
          reminders: { cron_schedules: [] },
          natural_input: false,
        })
      })
    })
  })

  test.describe('Blur normalization', () => {
    test('typing "1:.1,2:2." then tabbing away normalizes to "1:0.1,2:2"', async ({ page }) => {
      await setupPage(page)
      await openEditForm(page)

      const input = limitsInput(page)

      await test.step('Type "1:.1,2:2." → shown as typed', async () => {
        await input.focus()
        await input.pressSequentially('1:.1,2:2.', { delay: 10 })
        await expect(input).toHaveValue('1:.1,2:2.')
        await expect(input).toHaveAttribute('aria-invalid', 'false')
      })

      await test.step('Tab away → normalizes to "1:0.1,2:2"', async () => {
        await page.keyboard.press('Tab')
        await expect(input).toHaveValue('1:0.1,2:2')
        await expect(input).toHaveAttribute('aria-invalid', 'false')
      })
    })
  })
})

test.describe('Per-patient dose limits UI', () => {
  test('displays shared vs patient sections and copies limits from another patient', async ({
    page,
  }) => {
    await setupPage(
      page,
      [],
      [
        {
          patient_name: 'Bob',
          dose_limits: [
            { hours: 4, amount: 1 },
            { hours: 24, amount: 4 },
          ],
        },
        {
          patient_name: 'Carol',
          dose_limits: [{ hours: 6, amount: 2.5 }],
        },
      ],
    )

    let savedBody: unknown = null
    let putCount = 0
    await page.route(
      `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`,
      async (route) => {
        if (route.request().method() === 'PUT') {
          savedBody = route.request().postDataJSON()
          putCount += 1
          return route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({}),
          })
        }
        return route.fulfill({ status: 404 })
      },
    )

    await openEditForm(page)

    const input = limitsInput(page)
    const btn = saveButton(page)
    const bobButton = page.getByRole('button', { name: 'Bob (4:1,24:4)' })
    const carolButton = page.getByRole('button', { name: 'Carol (6:2.5)' })

    await test.step('Shows shared and patient-specific headers', async () => {
      await expect(
        page.locator('article > header', { hasText: 'Shared across all patients' }),
      ).toBeVisible()
      await expect(
        page.locator('article > header', { hasText: 'Settings for Alice' }),
      ).toBeVisible()
    })

    await test.step('Copies Bob limits into input without auto-saving', async () => {
      await expect(btn).toBeDisabled()
      await expect(bobButton).toBeEnabled()
      await expect(carolButton).toBeEnabled()

      await bobButton.click()

      await expect(input).toHaveValue('4:1,24:4')
      await expect(btn).toBeEnabled()
      expect(putCount).toBe(0)
    })

    await test.step('Saving submits copied per-patient dose_limits', async () => {
      await btn.click()
      await expect(btn).toBeDisabled()
      await expect.poll(() => putCount).toBe(1)
      expect(savedBody).toEqual({
        medication: {
          name: 'Aspirin',
          description: 'Pain reliever',
          aliases: [],
        },
        dose_limits: [
          { hours: 4, amount: 1 },
          { hours: 24, amount: 4 },
        ],
        reminders: { cron_schedules: [] },
        natural_input: false,
      })
    })
  })
})
