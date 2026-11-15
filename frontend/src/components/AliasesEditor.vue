<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->
<!-- cSpell: word Nurofen נורופן -->

<script setup lang="ts">
import { ref, useId, watch } from 'vue'

const inputId = useId()
const aliases = defineModel<Array<string>>('aliases', { required: true })

const rawInput = ref<string>('')
const isFocused = ref<boolean>(false)

function canonicalizeInput(items: Array<string>) {
  rawInput.value = items.join(', ')
}

function parseAliases(input: string): Array<string> {
  const result: Array<string> = []
  const seen = new Set<string>()

  for (const part of input.split(',')) {
    const trimmed = part.trim()
    if (trimmed.length === 0) continue
    const key = trimmed.toLowerCase()
    if (seen.has(key)) continue
    seen.add(key)
    result.push(trimmed)
  }

  return result
}

function arraysEqual(a: Array<string>, b: Array<string>): boolean {
  return a.length === b.length && a.every((v, i) => v === b[i])
}

watch(rawInput, (val) => {
  const parsed = parseAliases(val)
  if (!arraysEqual(aliases.value, parsed)) {
    aliases.value = parsed
  }
})

watch(
  aliases,
  (items) => {
    if (!isFocused.value) {
      canonicalizeInput(items)
    }
  },
  { immediate: true, deep: true },
)

function onInput(e: Event) {
  rawInput.value = (e.target as HTMLInputElement).value
}

function onBlur() {
  isFocused.value = false
  const parsed = parseAliases(rawInput.value)
  if (!arraysEqual(aliases.value, parsed)) {
    aliases.value = parsed
  }
  canonicalizeInput(parsed)
}

function onFocus() {
  isFocused.value = true
}
</script>

<template>
  <label :for="inputId">Aliases</label>
  <input
    :id="inputId"
    type="text"
    dir="auto"
    placeholder="Comma-separated aliases (e.g. Nurofen, נורופן)"
    :value="rawInput"
    @input="onInput"
    @blur="onBlur"
    @focus="onFocus"
  />
  <small>
    Comma-separated alternative names (such as brand or Hebrew names) matched by natural input in
    Telegram.
  </small>
</template>
