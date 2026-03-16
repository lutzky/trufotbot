import { test, expect } from '@playwright/test'

const mockPatients = [
  { id: 1, name: 'Alice' },
  { id: 3, name: 'Bob' },
]

test.describe('Form Enter key submission', () => {
  test('HomeView - Create patient form submits on Enter', async ({ page }) => {
    await page.route('**/api/patients', async (route) => {
      if (route.request().method() === 'GET') {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify(mockPatients),
        })
      } else {
        await route.abort()
      }
    })
    await page.route('**/api/patients', async (route) => {
      if (route.request().method() === 'POST') {
        await route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ id: 2, name: 'New Patient' }),
        })
      } else {
        await route.abort()
      }
    })

    await test.step('Navigate to home page', async () => {
      await page.goto('/')
    })

    await test.step('Open create patient form', async () => {
      await page.locator('summary', { hasText: 'Create patient' }).click()
    })

    await test.step('Fill and submit form with Enter key', async () => {
      const nameInput = page.getByLabel('Name', { exact: true })
      await nameInput.fill('New Patient')
      await nameInput.press('Enter')
    })

    await test.step('Verify save button is visible', async () => {
      await expect(page.getByRole('button', { name: 'Save' })).toBeVisible()
    })
  })

  test('PatientDetailView - Add new medication form submits on Enter', async ({ page }) => {
    const mockPatient = {
      id: 1,
      name: 'Alice',
      telegram_group_id: null,
      medications: [
        {
          id: 1,
          name: 'Preexisting',
          inventory: null,
          last_taken_at: null,
          next_doses: [],
        },
      ],
    }

    await page.route('**/api/medications', async (route) => {
      if (route.request().method() === 'POST') {
        const postData = route.request().postDataJSON()
        mockPatient.medications.push({
          id: 2,
          name: postData.name,
          inventory: null,
          last_taken_at: null,
          next_doses: [],
        })
        return route.fulfill({
          status: 201,
          contentType: 'application/json',
          body: JSON.stringify({ id: 2, name: postData.name }),
        })
      }
      return route.fulfill({ status: 404, body: '{}' })
    })
    await page.route('**/api/patients/1', async (route) => {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(mockPatient),
      })
    })

    await test.step('Navigate to patient detail page', async () => {
      await page.goto('/patients/1')
      await page.waitForLoadState('domcontentloaded')
    })

    await test.step('Open add medication form', async () => {
      await page
        .locator('details:has(summary:text("Add new medication"))')
        .evaluate((el: HTMLDetailsElement) => (el.open = true))
    })

    await test.step('Fill medication name and submit with Enter', async () => {
      const nameInput = page.locator('input[placeholder="Medication name"]')
      await expect(nameInput).toBeVisible()
      await nameInput.fill('New Med')
      await nameInput.press('Enter')
    })

    await test.step('Verify new medication appears', async () => {
      await expect(page.getByRole('heading', { name: 'New Med' })).toBeVisible()
    })
  })

  test('PatientMedicationDetailView - Edit medication form submits on Enter', async ({ page }) => {
    const mockDosesResponse = {
      patient_name: 'Alice',
      medication: { name: 'Aspirin', description: 'Pain reliever', dose_limits: [] },
      reminders: { cron_schedules: [] },
      doses: [],
      next_doses: [],
    }

    let lastPutData: { medication: { name: string } } | null = null

    await page.route('**/api/patients/1/medications/1', async (route) => {
      if (route.request().method() === 'PUT') {
        lastPutData = route.request().postDataJSON()
        const postData = route.request().postDataJSON()
        return route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ id: 1, name: postData.medication.name }),
        })
      }
      return route.fulfill({ status: 404, body: '{}' })
    })
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

    await test.step('Update medication name and submit with Enter', async () => {
      const nameInput = page.locator('input[placeholder="Medication name"]')
      await expect(nameInput).toBeVisible()
      await nameInput.focus()
      await nameInput.fill('Updated Med')
      await nameInput.press('Enter')
    })

    await test.step('Verify medication was updated', async () => {
      await expect
        .poll(async () => {
          return lastPutData
        })
        .not.toBeNull()
      expect(lastPutData!.medication.name).toBe('Updated Med')
    })
  })
})
