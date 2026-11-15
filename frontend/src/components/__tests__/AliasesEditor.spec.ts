// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

// cSpell: word Nurofen נורופן אקמול

import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import AliasesEditor from '../AliasesEditor.vue'

describe('AliasesEditor', () => {
  it('renders existing aliases joined by comma-space and sets dir="auto"', () => {
    const wrapper = mount(AliasesEditor, {
      props: {
        aliases: ['Nurofen', 'נורופן'],
        aliasesModifiers: {},
      },
    })

    const input = wrapper.find('input')
    expect(input.attributes('dir')).toBe('auto')
    expect((input.element as HTMLInputElement).value).toBe('Nurofen, נורופן')
    expect(wrapper.emitted('update:aliases')).toBeUndefined()
  })

  it('splits on commas, trims whitespace, drops empties, and dedupes case-insensitively', async () => {
    const wrapper = mount(AliasesEditor, {
      props: {
        aliases: [],
        aliasesModifiers: {},
      },
    })

    const input = wrapper.find('input')
    await input.trigger('focus')
    await input.setValue('  Advil , , nurofen ,  NUROFEN ,  נורופן , אקמול  ')

    const emitted = wrapper.emitted('update:aliases')
    expect(emitted).toBeTruthy()
    expect(emitted!.at(-1)?.[0]).toEqual(['Advil', 'nurofen', 'נורופן', 'אקמול'])
  })

  it('handles Hebrew aliases', async () => {
    const wrapper = mount(AliasesEditor, {
      props: {
        aliases: [],
        aliasesModifiers: {},
      },
    })

    const input = wrapper.find('input')
    await input.trigger('focus')
    await input.setValue('נורופן, אקמול')

    const emitted = wrapper.emitted('update:aliases')
    expect(emitted).toBeTruthy()
    expect(emitted!.at(-1)?.[0]).toEqual(['נורופן', 'אקמול'])
  })

  it('preserves trailing comma while focused and canonicalizes on blur', async () => {
    const wrapper = mount(AliasesEditor, {
      props: {
        aliases: [],
        aliasesModifiers: {},
        'onUpdate:aliases': (value: string[]) => wrapper.setProps({ aliases: value }),
      },
    })

    const input = wrapper.find('input')
    const inputEl = input.element as HTMLInputElement

    await input.trigger('focus')
    await input.setValue('Nurofen,')
    expect(inputEl.value).toBe('Nurofen,')
    expect(wrapper.emitted('update:aliases')?.at(-1)?.[0]).toEqual(['Nurofen'])

    await input.setValue('Nurofen,  נורופן , ')
    expect(inputEl.value).toBe('Nurofen,  נורופן , ')

    await input.trigger('blur')
    expect(inputEl.value).toBe('Nurofen, נורופן')
    expect(wrapper.emitted('update:aliases')?.at(-1)?.[0]).toEqual(['Nurofen', 'נורופן'])
  })
})
