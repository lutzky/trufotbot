// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import RelativeTime from '../RelativeTime.vue'

interface TestProps {
  time: Date
  clampFuture?: boolean
}

interface TestCase {
  description: string
  props: TestProps
  expected: string
}

describe('RelativeTime', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    const now = new Date('2023-10-27T10:00:00Z')
    vi.setSystemTime(now)
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  const testCases: TestCase[] = [
    {
      description: 'renders a time in the past as relative when not clamped',
      props: { time: new Date('2023-10-27T09:00:00Z') },
      expected: 'about 1 hour ago',
    },
    {
      description: 'renders a time in the future as relative when not clamped',
      props: { time: new Date('2023-10-27T10:05:00Z') },
      expected: 'in 5 minutes',
    },
    {
      description: 'renders a time in the past as "now" when clamped',
      props: { time: new Date('2023-10-27T09:59:50Z'), clampFuture: true },
      expected: 'now',
    },
    {
      description: 'renders a time in the future as relative when clamped',
      props: { time: new Date('2023-10-27T10:00:10Z'), clampFuture: true },
      expected: 'in less than a minute',
    },
  ]

  it.each(testCases)('$description', ({ props, expected }) => {
    const wrapper = mount(RelativeTime, { props })
    expect(wrapper.text()).toContain(expected)
  })
})
