// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import DisplayQuantity from '../DisplayQuantity.vue'

interface TestCase {
  description: string
  props: { value: number | null | undefined; precision?: number }
  expected: string
}

describe('DisplayQuantity', () => {
  const testCases: TestCase[] = [
    {
      description: 'renders 0 as "0"',
      props: { value: 0 },
      expected: '0',
    },
    {
      description: 'renders null as "0"',
      props: { value: null },
      expected: '0',
    },
    {
      description: 'renders undefined as "0"',
      props: { value: undefined },
      expected: '0',
    },
    {
      description: 'renders exact integers without trailing zeros',
      props: { value: 1 },
      expected: '1',
    },
    {
      description: 'renders 0.1 without floating-point noise',
      props: { value: 0.1 },
      expected: '0.1',
    },
    {
      description: 'clamps floating-point noise: 0.20000000000000018 → 0.2',
      props: { value: 0.20000000000000018 },
      expected: '0.2',
    },
    {
      description: 'rounds to 4 significant digits: 0.123456 → 0.1235',
      props: { value: 0.123456 },
      expected: '0.1235',
    },
    {
      description: 'clamps floating-point noise: 0.10000000000000005 → 0.1',
      props: { value: 0.10000000000000005 },
      expected: '0.1',
    },
    {
      description: 'renders larger numbers correctly',
      props: { value: 12.345 },
      expected: '12.35',
    },
    {
      description: 'uses custom precision when specified',
      props: { value: 0.1234567, precision: 3 },
      expected: '0.123',
    },
  ]

  it.each(testCases)('$description', ({ props, expected }) => {
    const wrapper = mount(DisplayQuantity, { props })
    expect(wrapper.text()).toBe(expected)
  })
})
