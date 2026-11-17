// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

// cSpell: word Nurofen נורופן Advil Acamol

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import PatientMedicationSummaryCard from '../PatientMedicationSummaryCard.vue'
import type { MedicationSummary } from '@/openapi'

function makeMedication(overrides: Partial<MedicationSummary> = {}): MedicationSummary {
  return {
    id: 1,
    name: 'Nurofen Kids',
    last_taken_at: null,
    next_doses: [],
    inventory: null,
    aliases: [],
    natural_input: false,
    ...overrides,
  }
}

describe('PatientMedicationSummaryCard', () => {
  it('renders chat icon without muted class when natural_input is true', () => {
    const wrapper = mount(PatientMedicationSummaryCard, {
      props: {
        medication: makeMedication({ natural_input: true }),
      },
    })

    const icon = wrapper.get('[role="img"]')
    expect(icon.text()).toBe('chat')
    expect(icon.attributes('aria-label')).toBe('Natural input enabled')
    expect(icon.attributes('title')).toBe('Natural input enabled')
    expect(icon.classes()).not.toContain('natural-input-disabled')
  })

  it('renders speaker_notes_off icon with muted class when natural_input is false', () => {
    const wrapper = mount(PatientMedicationSummaryCard, {
      props: {
        medication: makeMedication({ natural_input: false }),
      },
    })

    const icon = wrapper.get('[role="img"]')
    expect(icon.text()).toBe('speaker_notes_off')
    expect(icon.attributes('aria-label')).toBe('Natural input disabled')
    expect(icon.attributes('title')).toBe('Natural input disabled')
    expect(icon.classes()).toContain('natural-input-disabled')
  })

  it.each([true, false])(
    'renders comma-separated aliases with dir="auto" when natural_input is %s',
    (natural_input) => {
      const wrapper = mount(PatientMedicationSummaryCard, {
        props: {
          medication: makeMedication({
            natural_input,
            aliases: ['נורופן', 'Advil'],
          }),
        },
      })

      const aliasesEl = wrapper.find('hgroup p')
      expect(aliasesEl.exists()).toBe(true)
      expect(aliasesEl.text()).toBe('נורופן, Advil')
      expect(aliasesEl.attributes('dir')).toBe('auto')
    },
  )

  it('does not render an aliases paragraph inside hgroup when aliases is empty', () => {
    const wrapper = mount(PatientMedicationSummaryCard, {
      props: {
        medication: makeMedication({ aliases: [] }),
      },
    })

    expect(wrapper.find('hgroup p').exists()).toBe(false)
  })
})
