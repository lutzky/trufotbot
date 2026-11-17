// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import DoseLimitsEditor from '../DoseLimitsEditor.vue'
import type { DoseLimit, PatientDoseLimits } from '@/openapi'

describe('DoseLimitsEditor', () => {
  const otherPatients: PatientDoseLimits[] = [
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
  ]

  it('renders syntax help and hides copy section when no other patients have limits', () => {
    const wrapper = mount(DoseLimitsEditor, {
      props: {
        doseLimits: [] as DoseLimit[],
        otherPatientsLimits: [],
      },
    })

    expect(wrapper.text()).toContain('hours:amount')
    expect(wrapper.find('.other-limits').exists()).toBe(false)
  })

  it('renders other patients limits as buttons', () => {
    const wrapper = mount(DoseLimitsEditor, {
      props: {
        doseLimits: [{ hours: 6, amount: 2.5 }],
        otherPatientsLimits: otherPatients,
      },
    })

    const buttons = wrapper.findAll('.other-limits button')
    expect(buttons).toHaveLength(2)
    expect(buttons[0]!.text()).toBe('Bob (4:1,24:4)')
    expect(buttons[1]!.text()).toBe('Carol (6:2.5)')
  })

  it('clicking another patient button copies their limits and marks valid', async () => {
    let currentLimits: DoseLimit[] = []
    const wrapper = mount(DoseLimitsEditor, {
      props: {
        doseLimits: currentLimits,
        'onUpdate:doseLimits': (val: DoseLimit[]) => {
          currentLimits = val
          wrapper.setProps({ doseLimits: val })
        },
        otherPatientsLimits: otherPatients,
      },
    })

    const bobButton = wrapper.findAll('.other-limits button')[0]!
    await bobButton.trigger('click')
    await nextTick()

    expect(currentLimits).toEqual([
      { hours: 4, amount: 1 },
      { hours: 24, amount: 4 },
    ])
    expect(wrapper.find('input').element.value).toBe('4:1,24:4')
    expect(wrapper.emitted('validity-change')?.at(-1)).toEqual([true])
  })
})
