<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { DoseLimit } from '@/openapi'
import { ref, watch } from 'vue'

const doseLimits = defineModel<Array<DoseLimit>>('doseLimits', { required: true })

const emit = defineEmits<{
  (e: 'validity-change', isValid: boolean): void
}>()

const rawInput = ref<string>('')
const isFocused = ref<boolean>(false)
const isValid = ref<boolean>(true)

function splitOnce(s: string, d: string): [string, string | undefined] {
  const idx = s.indexOf(d)
  return idx === -1 ? [s, undefined] : [s.substring(0, idx), s.substring(idx + 1)]
}

function canonicalizeInput(limits: Array<DoseLimit>) {
  rawInput.value = limits.map((l) => `${l.hours}:${l.amount}`).join(',')
}

function parseLimits(input: string): Array<DoseLimit> | null {
  if (input === '') return []

  const parts = input.split(',')
  const result: Array<DoseLimit> = []

  for (const part of parts) {
    let [hoursStr, amountStr] = splitOnce(part, ':')
    if (amountStr === undefined) return null
    hoursStr = hoursStr.trim()
    amountStr = amountStr.trim()
    if (hoursStr === '' || amountStr === '') return null
    const hours = Number(hoursStr)
    const amount = Number(amountStr)
    if (!Number.isFinite(hours) || !Number.isFinite(amount)) return null
    if (!Number.isInteger(hours)) return null
    result.push({ hours, amount })
  }
  return result
}

function processInput(text: string) {
  const trimmed = text.trim()
  if (trimmed === '') {
    isValid.value = true
    doseLimits.value = []
    emit('validity-change', true)
    return
  }
  const parsed = parseLimits(trimmed)
  if (parsed !== null) {
    isValid.value = true
    doseLimits.value = parsed
  } else {
    isValid.value = false
  }
  emit('validity-change', isValid.value)
}

watch(rawInput, (val) => processInput(val))

watch(
  doseLimits,
  (limits) => {
    if (!isFocused.value) {
      canonicalizeInput(limits)
    }
  },
  { immediate: true, deep: true },
)

function onInput(e: Event) {
  rawInput.value = (e.target as HTMLTextAreaElement).value
}

function onBlur() {
  isFocused.value = false
  const trimmed = rawInput.value.trim()
  const parsed = parseLimits(trimmed)
  if (parsed !== null) {
    canonicalizeInput(parsed)
  }
}

function onFocus() {
  isFocused.value = true
}
</script>

<template>
  <label>
    Limits
    <textarea
      :aria-invalid="!isValid"
      placeholder="hours:amount,hours:amount,..."
      :value="rawInput"
      @input="onInput"
      @blur="onBlur"
      @focus="onFocus"
    />
  </label>
</template>
