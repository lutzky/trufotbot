// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

// cSpell: word Nurofen נורופן

import { test, expect, type Page } from './fixtures'

const PATIENT_ID = 1
const MEDICATION_ID = 1

interface StoredMedicationState {
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
}

function makeInitialState(): StoredMedicationState {
  return {
    patient_name: 'Alice',
    medication: {
      name: 'Nurofen Kids',
      description: 'Pediatric ibuprofen',
      aliases: [],
    },
    dose_limits: [],
    other_patients_dose_limits: [],
    reminders: { cron_schedules: [] },
    natural_input: false,
  }
}

function validateUpdateBody(
  body: unknown,
): { status: number; contentType: string; body: string } | undefined {
  if (typeof body !== 'object' || body === null) {
    return { status: 422, contentType: 'text/plain', body: 'Expected JSON object' }
  }
  const obj = body as Record<string, unknown>
  if (typeof obj.medication !== 'object' || obj.medication === null) {
    return { status: 422, contentType: 'text/plain', body: 'Missing medication field' }
  }
  const med = obj.medication as Record<string, unknown>
  if (!Array.isArray(med.aliases) || med.aliases.some((a) => typeof a !== 'string')) {
    return {
      status: 422,
      contentType: 'text/plain',
      body: 'medication.aliases: expected an array of strings',
    }
  }
  for (const alias of med.aliases as Array<string>) {
    if (alias.includes(',') || alias.includes('\n') || alias.includes('\r')) {
      return {
        status: 400,
        contentType: 'text/plain',
        body: `Invalid alias ${JSON.stringify(alias)}: aliases cannot contain commas or newlines`,
      }
    }
  }
  if (
    !Array.isArray(obj.dose_limits) ||
    obj.dose_limits.some((limit) => {
      if (typeof limit !== 'object' || limit === null) {
        return true
      }
      const { hours, amount } = limit as Record<string, unknown>
      return (
        typeof hours !== 'number' ||
        !Number.isInteger(hours) ||
        hours < 0 ||
        hours > 65535 ||
        typeof amount !== 'number' ||
        !Number.isFinite(amount)
      )
    })
  ) {
    return {
      status: 422,
      contentType: 'text/plain',
      body: 'dose_limits: expected an array of { hours: u16, amount: f64 }',
    }
  }
  if (
    obj.natural_input !== undefined &&
    obj.natural_input !== null &&
    typeof obj.natural_input !== 'boolean'
  ) {
    return {
      status: 422,
      contentType: 'text/plain',
      body: 'natural_input: expected boolean or null',
    }
  }
  return undefined
}

async function openEditForm(page: Page) {
  await page.goto(`/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`)
  await page.waitForLoadState('domcontentloaded')
  await page
    .locator('details:has(summary:text("Edit medication"))')
    .evaluate((el: HTMLDetailsElement) => (el.open = true))
}

function aliasesInput(page: Page) {
  return page.getByLabel('Aliases', { exact: true })
}

function naturalInputSwitch(page: Page) {
  return page.getByRole('switch', { name: 'Allow natural input for Alice' })
}

function saveButton(page: Page) {
  return page.getByRole('button', { name: 'Save' })
}

test.describe('Natural input UI', () => {
  test('edits aliases, toggles natural input switch, saves, and persists across reload', async ({
    page,
  }) => {
    const store = makeInitialState()
    const updateBodies: unknown[] = []

    await page.route(
      `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}/doses`,
      async (route) => {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ ...store, doses: [], next_doses: [] }),
        })
      },
    )

    await page.route(
      `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`,
      async (route) => {
        if (route.request().method() === 'PUT') {
          const body = route.request().postDataJSON()
          updateBodies.push(body)

          const rejection = validateUpdateBody(body)
          if (rejection) {
            return route.fulfill(rejection)
          }

          store.medication = body.medication
          store.dose_limits = body.dose_limits
          store.reminders = body.reminders
          if (typeof body.natural_input === 'boolean') {
            store.natural_input = body.natural_input
          }

          return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
        }
        return route.fulfill({ status: 404, body: '{}' })
      },
    )

    await openEditForm(page)

    const aliases = aliasesInput(page)
    const toggle = naturalInputSwitch(page)
    const save = saveButton(page)

    await test.step('Initial state shows empty aliases and unchecked switch', async () => {
      await expect(aliases).toHaveValue('')
      await expect(aliases).toHaveAttribute('dir', 'auto')
      await expect(toggle).not.toBeChecked()
      await expect(save).toBeDisabled()
    })

    await test.step('Typing trailing comma survives while focused and normalizes on blur', async () => {
      await aliases.focus()
      await aliases.fill('נורופן,')
      await expect(aliases).toHaveValue('נורופן,')

      await aliases.fill('נורופן, Advil, ')
      await expect(aliases).toHaveValue('נורופן, Advil, ')

      await aliases.blur()
      await expect(aliases).toHaveValue('נורופן, Advil')
    })

    await test.step('Toggling natural input and saving sends expected PUT body', async () => {
      await toggle.check()
      await expect(toggle).toBeChecked()
      await expect(save).toBeEnabled()

      await save.click()
      await expect(save).toBeDisabled()
      expect(updateBodies).toHaveLength(1)
      expect(updateBodies[0]).toEqual({
        medication: {
          name: 'Nurofen Kids',
          description: 'Pediatric ibuprofen',
          aliases: ['נורופן', 'Advil'],
        },
        dose_limits: [],
        reminders: { cron_schedules: [] },
        natural_input: true,
      })
    })

    await test.step('Reloading restores aliases and natural input switch', async () => {
      await page.reload()
      await page.waitForLoadState('domcontentloaded')
      await page
        .locator('details:has(summary:text("Edit medication"))')
        .evaluate((el: HTMLDetailsElement) => (el.open = true))

      await expect(aliasesInput(page)).toHaveValue('נורופן, Advil')
      await expect(naturalInputSwitch(page)).toBeChecked()
    })
  })

  test('shows 400 collision error banner and preserves typed form state', async ({ page }) => {
    const store = makeInitialState()
    const collisionMessage =
      '"Nurofen" would collide with "Nurofen Kids" (alias "nurofen"), which is also enabled for natural input for Alice.'
    let putCount = 0

    await page.route(
      `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}/doses`,
      async (route) => {
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ ...store, doses: [], next_doses: [] }),
        })
      },
    )

    await page.route(
      `**/api/patients/${PATIENT_ID}/medications/${MEDICATION_ID}`,
      async (route) => {
        if (route.request().method() === 'PUT') {
          const body = route.request().postDataJSON()
          putCount += 1

          const rejection = validateUpdateBody(body)
          if (rejection) {
            return route.fulfill(rejection)
          }

          if (body.natural_input === true && body.medication.aliases.includes('Nurofen')) {
            return route.fulfill({
              status: 400,
              contentType: 'text/plain',
              body: collisionMessage,
            })
          }

          store.medication = body.medication
          store.dose_limits = body.dose_limits
          store.reminders = body.reminders
          if (typeof body.natural_input === 'boolean') {
            store.natural_input = body.natural_input
          }

          return route.fulfill({ status: 200, contentType: 'application/json', body: '{}' })
        }
        return route.fulfill({ status: 404, body: '{}' })
      },
    )

    await openEditForm(page)

    const aliases = aliasesInput(page)
    const toggle = naturalInputSwitch(page)
    const save = saveButton(page)

    await aliases.fill('Nurofen')
    await aliases.blur()
    await toggle.check()
    await save.click()

    await test.step('Collision error is displayed and form keeps user input', async () => {
      await expect(page.getByRole('alert')).toContainText(collisionMessage)
      await expect(aliases).toHaveValue('Nurofen')
      await expect(toggle).toBeChecked()
      await expect(save).toBeEnabled()
      expect(putCount).toBe(1)
    })

    await test.step('User can uncheck the switch and save successfully', async () => {
      await toggle.uncheck()
      await save.click()
      await expect(save).toBeDisabled()
      await expect(page.getByRole('alert')).toBeHidden()
      expect(putCount).toBe(2)
      expect(store.natural_input).toBe(false)
      expect(store.medication.aliases).toEqual(['Nurofen'])
    })
  })
})
