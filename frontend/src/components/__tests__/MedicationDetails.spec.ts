// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import MedicationDetails from '../MedicationDetails.vue'
import AliasesEditor from '../AliasesEditor.vue'
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

  it('renders inputs and subcomponents grouped by shared vs patient-specific when creating is false', () => {
    const props = { ...defaultProps(), patientName: 'Alice' }
    const wrapper = mount(MedicationDetails, { props })

    const sections = wrapper.findAll('article')
    expect(sections).toHaveLength(2)

    const sharedSection = sections[0]!
    expect(sharedSection.find('header').text()).toBe('Shared across all patients')
    expect(sharedSection.find('input[placeholder="Medication name"]').exists()).toBe(true)
    expect(sharedSection.find('textarea[placeholder="Medication description"]').exists()).toBe(true)
    expect(sharedSection.find('input[placeholder="Inventory"]').exists()).toBe(true)
    expect(sharedSection.findComponent(AliasesEditor).exists()).toBe(true)

    const patientSection = sections[1]!
    expect(patientSection.find('header').text()).toBe('Settings for Alice (for Aspirin)')
    expect(patientSection.find('header em').text()).toBe('Aspirin')
    expect(patientSection.find('label#natural-input').exists()).toBe(true)
    expect(patientSection.find('input[role="switch"]').exists()).toBe(true)
    expect(patientSection.find('a[href="#medication-aliases"]').text()).toBe('aliases')
    expect(patientSection.findComponent(ReminderScheduleEditor).exists()).toBe(true)
    expect(patientSection.findComponent(DoseLimitsEditor).exists()).toBe(true)
  })

  it('uses fallback patient and medication labels in header when omitted', () => {
    const wrapper = mount(MedicationDetails, { props: defaultProps() })
    const headers = wrapper.findAll('article > header').map((l) => l.text())

    expect(headers).toEqual([
      'Shared across all patients',
      'Settings for this patient (for Aspirin)',
    ])

    const emptyNameWrapper = mount(MedicationDetails, {
      props: { ...defaultProps(), name: '' },
    })
    expect(emptyNameWrapper.findAll('article > header')[1]!.text()).toBe(
      'Settings for this patient (for this medication)',
    )
  })

  it('shows patient name on the natural input switch label', () => {
    const props = { ...defaultProps(), patientName: 'Alice' }
    const wrapper = mount(MedicationDetails, { props })

    expect(wrapper.text()).toContain('Allow natural input for Alice')
  })

  it('hides inventory, sections, and subcomponents when creating is true', () => {
    const props = { ...defaultProps(), creating: true }
    const wrapper = mount(MedicationDetails, { props })

    expect(wrapper.find('article').exists()).toBe(false)
    expect(wrapper.find('input[placeholder="Medication name"]').exists()).toBe(true)
    expect(wrapper.find('textarea[placeholder="Medication description"]').exists()).toBe(true)
    expect(wrapper.find('input[placeholder="Inventory"]').exists()).toBe(false)
    expect(wrapper.findComponent(AliasesEditor).exists()).toBe(false)
    expect(wrapper.find('input[role="switch"]').exists()).toBe(false)
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
