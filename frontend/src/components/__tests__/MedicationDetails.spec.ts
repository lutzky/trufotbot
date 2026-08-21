// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MedicationDetails from '../MedicationDetails.vue'
import DoseLimitsEditor from '../DoseLimitsEditor.vue'
import ReminderScheduleEditor from '../ReminderScheduleEditor.vue'
import type { DoseLimit } from '@/openapi'

describe('MedicationDetails', () => {
  const defaultProps = () => ({
    name: 'Aspirin',
    nameModifiers: {},
    description: 'Pain reliever',
    descriptionModifiers: {},
    inventory: 10,
    inventoryModifiers: {},
    doseLimits: [] as DoseLimit[],
    doseLimitsModifiers: {},
    reminders: [] as string[],
    remindersModifiers: {},
  })

  it('renders inputs and subcomponents when creating is false', () => {
    const props = defaultProps()
    const wrapper = mount(MedicationDetails, { props })

    expect(wrapper.find('input[placeholder="Medication name"]').exists()).toBe(true)
    expect(wrapper.find('textarea[placeholder="Medication description"]').exists()).toBe(true)
    expect(wrapper.find('input[placeholder="Inventory"]').exists()).toBe(true)
    expect(wrapper.findComponent(ReminderScheduleEditor).exists()).toBe(true)
    expect(wrapper.findComponent(DoseLimitsEditor).exists()).toBe(true)
  })

  it('hides inventory and subcomponents when creating is true', () => {
    const props = { ...defaultProps(), creating: true }
    const wrapper = mount(MedicationDetails, { props })

    expect(wrapper.find('input[placeholder="Medication name"]').exists()).toBe(true)
    expect(wrapper.find('textarea[placeholder="Medication description"]').exists()).toBe(true)
    expect(wrapper.find('input[placeholder="Inventory"]').exists()).toBe(false)
    expect(wrapper.findComponent(ReminderScheduleEditor).exists()).toBe(false)
    expect(wrapper.findComponent(DoseLimitsEditor).exists()).toBe(false)
  })

  it('combines validity from subcomponents', async () => {
    const props = defaultProps()
    const wrapper = mount(MedicationDetails, { props })

    const reminderEditor = wrapper.findComponent(ReminderScheduleEditor)
    const doseLimitsEditor = wrapper.findComponent(DoseLimitsEditor)

    expect(wrapper.emitted('update:isValid')?.at(-1)?.[0]).toBe(true)

    // Trigger invalid state in reminder editor
    await reminderEditor.vm.$emit('validity-change', false)
    expect(wrapper.emitted('update:isValid')?.at(-1)?.[0]).toBe(false)

    // Reset reminder editor to valid
    await reminderEditor.vm.$emit('validity-change', true)
    expect(wrapper.emitted('update:isValid')?.at(-1)?.[0]).toBe(true)

    // Trigger invalid state in dose limits editor
    await doseLimitsEditor.vm.$emit('validity-change', false)
    expect(wrapper.emitted('update:isValid')?.at(-1)?.[0]).toBe(false)
  })
})
