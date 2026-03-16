import { test, expect } from '@playwright/test'

// See here how to get started:
// https://playwright.dev/docs/intro
test('visits the app root url', async ({ page }) => {
  const mockPatients = [
    { id: 1, name: 'Alice' },
    { id: 3, name: 'Bob' },
  ]
  await page.route('**/api/patients', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(mockPatients),
    })
  })

  await page.goto('/')
  await expect(page.locator('h1')).toHaveText('Select Patient')

  await expect(page.getByRole('button', { name: 'Alice' })).toBeVisible()
  await expect(page.getByRole('button', { name: 'Bob' })).toBeVisible()
})
