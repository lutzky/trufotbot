// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import ReminderScheduleEditor from '../ReminderScheduleEditor.vue'

describe('ReminderScheduleEditor', () => {
  const defaultProps = () => ({
    reminders: [] as string[],
    remindersModifiers: {},
  })

  it('translates English natural language input to 6-part cron and displays explanation', async () => {
    const props = defaultProps()
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    await textarea.setValue('Every day at 8:00 AM')

    // Expect emitted update for reminders with 6-part cron
    const emittedReminders = wrapper.emitted('update:reminders')
    expect(emittedReminders).toBeTruthy()
    const lastEmitted = emittedReminders![emittedReminders!.length - 1][0] as string[]
    expect(lastEmitted).toEqual(['0 0 8 * * *'])

    // Expect explanation shown at bottom
    const smallText = wrapper.find('small').text()
    expect(smallText).toContain('At 08:00 AM (cron: 0 0 8 * * *)')
  })

  it('accepts raw 6-part cron syntax input directly', async () => {
    const props = defaultProps()
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    await textarea.setValue('0 30 8,19 * * *')

    const emittedReminders = wrapper.emitted('update:reminders')
    expect(emittedReminders).toBeTruthy()
    const lastEmitted = emittedReminders![emittedReminders!.length - 1][0] as string[]
    expect(lastEmitted).toEqual(['0 30 8,19 * * *'])

    const smallText = wrapper.find('small').text()
    expect(smallText).toContain('At 08:30 AM and 07:30 PM (cron: 0 30 8,19 * * *)')
  })

  it('handles multi-line inputs with English and Cron mix', async () => {
    const props = defaultProps()
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    await textarea.setValue('Every day at 8am\n0 0 20 * * *')

    const emittedReminders = wrapper.emitted('update:reminders')
    expect(emittedReminders).toBeTruthy()
    const lastEmitted = emittedReminders![emittedReminders!.length - 1][0] as string[]
    expect(lastEmitted).toEqual(['0 0 8 * * *', '0 0 20 * * *'])

    const smallText = wrapper.find('small').text()
    expect(smallText).toContain('At 08:00 AM (cron: 0 0 8 * * *); At 08:00 PM (cron: 0 0 20 * * *)')
  })

  it('displays error and sets aria-invalid on invalid input', async () => {
    const props = defaultProps()
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    await textarea.setValue('invalid expression 123')

    expect(textarea.attributes('aria-invalid')).toBe('true')
    const smallText = wrapper.find('small').text()
    expect(smallText).toContain('Failed to parse "invalid expression 123"')

    const emittedValidity = wrapper.emitted('validity-change')
    expect(emittedValidity).toBeTruthy()
    const lastValidity = emittedValidity![emittedValidity!.length - 1][0]
    expect(lastValidity).toBe(false)
  })

  it('does not set aria-invalid attribute when input is valid', async () => {
    const props = defaultProps()
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    expect(textarea.attributes('aria-invalid')).toBeUndefined()

    await textarea.setValue('Every day at 8:00 AM')
    expect(textarea.attributes('aria-invalid')).toBeUndefined()
  })

  it('initializes textarea in clean English mode when loaded with existing cron reminders', async () => {
    const props = {
      ...defaultProps(),
      reminders: ['0 30 11,14 3 * *'],
    }
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    const textareaEl = textarea.element as HTMLTextAreaElement
    expect(textareaEl.value).toBe('at hour 11 and 14 at minute 30 on day 3')

    const smallText = wrapper.find('small').text()
    expect(smallText).toContain(
      'At 11:30 AM and 02:30 PM, on day 3 of the month (cron: 0 30 11,14 3 * *)',
    )
    const codeTag = wrapper.find('small code')
    expect(codeTag.text()).toBe('0 30 11,14 3 * *')

    // Verify mounting with existing reminders does not emit update:reminders
    expect(wrapper.emitted('update:reminders')).toBeUndefined()
  })

  it('handles weekday range when loaded with existing cron reminders without changing original cron', async () => {
    const props = {
      ...defaultProps(),
      reminders: ['0 0 8 * * 1-5'],
    }
    const wrapper = mount(ReminderScheduleEditor, { props })

    const textarea = wrapper.find('textarea#reminder_schedules')
    expect(textarea.attributes('aria-invalid')).toBeUndefined()

    const emittedValidity = wrapper.emitted('validity-change')
    expect(emittedValidity).toBeTruthy()
    expect(emittedValidity![emittedValidity!.length - 1][0]).toBe(true)

    const textareaEl = textarea.element as HTMLTextAreaElement
    expect(textareaEl.value).toBe('at 8am on monday to friday')

    const smallText = wrapper.find('small').text()
    expect(smallText).toContain('At 08:00 AM, Monday through Friday (cron: 0 0 8 * * 1-5)')

    // Verify mounting with existing reminders never emits update:reminders
    expect(wrapper.emitted('update:reminders')).toBeUndefined()
  })
})
