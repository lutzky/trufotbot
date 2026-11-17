<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { DoseLimit, PatientDoseLimits } from '@/openapi'
import { ref, useId, watch } from 'vue'

defineProps<{
  otherPatientsLimits?: Array<PatientDoseLimits>
}>()

const inputId = useId()
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

function formatLimits(limits: Array<DoseLimit>): string {
  return limits.map((l) => `${l.hours}:${l.amount}`).join(',')
}

function canonicalizeInput(limits: Array<DoseLimit>) {
  rawInput.value = formatLimits(limits)
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

function copyLimits(limits: Array<DoseLimit>) {
  const cloned = limits.map((l) => ({ hours: l.hours, amount: l.amount }))
  doseLimits.value = cloned
  canonicalizeInput(cloned)
  isValid.value = true
  emit('validity-change', true)
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
  rawInput.value = (e.target as HTMLInputElement).value
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
  <label :for="inputId">Limits</label>
  <input
    :id="inputId"
    type="text"
    :aria-invalid="!isValid"
    placeholder="hours:amount,hours:amount,..."
    :value="rawInput"
    @input="onInput"
    @blur="onBlur"
    @focus="onFocus"
  />
  <small>
    Format: <code>hours:amount</code>, comma-separated (e.g. <code>4:1,24:4</code> for max 1 every
    4h and 4 every 24h).
  </small>
  <p v-if="otherPatientsLimits && otherPatientsLimits.length > 0" class="other-limits">
    <small>Other patients' limits (click to copy):</small>
    <button
      v-for="other in otherPatientsLimits"
      :key="other.patient_name"
      type="button"
      class="outline secondary copy-limit-button"
      @click="copyLimits(other.dose_limits)"
    >
      {{ other.patient_name }} ({{ formatLimits(other.dose_limits) }})
    </button>
  </p>
</template>

<style scoped>
.other-limits {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.25rem;
}

.copy-limit-button {
  width: auto;
  margin-bottom: 0;
  padding: 0.25rem 0.6rem;
  font-size: 0.85em;
}
</style>
