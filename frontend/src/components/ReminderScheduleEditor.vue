<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import cronstrue from 'cronstrue'
import { toCron, toHuman } from 'cron-translate'
import { computed, ref, watch } from 'vue'

const reminders = defineModel<Array<string>>('reminders', { required: true })

const emit = defineEmits<{
  (e: 'validity-change', isValid: boolean): void
}>()

function parseNaturalLanguageToCron(text: string): string {
  return toCron(text.trim())
}

function cronToHumanText(cron: string): string {
  try {
    return toHuman(cron)
  } catch {
    return cron
  }
}

function cronToExplanation(cron: string): string {
  try {
    return cronstrue.toString(cron)
  } catch {
    try {
      return toHuman(cron)
    } catch {
      return cron
    }
  }
}

const userInput = ref<string>('')
const isUserEdited = ref<boolean>(false)
let lastEmittedCronJoined = ''

function syncFromReminders() {
  const currentCronJoined = reminders.value.join('\n')
  if (currentCronJoined !== lastEmittedCronJoined) {
    lastEmittedCronJoined = currentCronJoined
    userInput.value = reminders.value.map(cronToHumanText).join('\n')
    isUserEdited.value = false
  }
}

function onInput() {
  isUserEdited.value = true
}

watch(
  reminders,
  () => {
    syncFromReminders()
  },
  { immediate: true, deep: true },
)

interface ScheduleItem {
  description: string
  cron: string
}

const scheduleExplanations = computed<{
  isValid: boolean
  items: ScheduleItem[]
  error: string
}>(() => {
  const lines = userInput.value.split('\n').filter((l) => l.trim().length > 0)
  if (lines.length === 0) {
    return { isValid: true, items: [], error: '' }
  }

  const items: ScheduleItem[] = []
  for (const rawLine of lines) {
    const line = rawLine.trim()

    // 1. Check if line is 6-part raw cron syntax
    const parts = line.split(/\s+/)
    let isValidCron = false
    if (parts.length === 6) {
      try {
        cronstrue.toString(line)
        isValidCron = true
      } catch {
        // Not valid 6-part cron
      }
    }

    if (isValidCron) {
      items.push({
        description: cronToExplanation(line),
        cron: line,
      })
      continue
    }

    // 2. Try converting natural language English text -> Cron
    try {
      const parsedCron = parseNaturalLanguageToCron(line)
      cronstrue.toString(parsedCron)
      items.push({
        description: cronToExplanation(parsedCron),
        cron: parsedCron,
      })
    } catch (error) {
      const errMsg = error instanceof Error ? error.message : String(error)
      return { isValid: false, items: [], error: `Failed to parse "${line}": ${errMsg}` }
    }
  }

  return { isValid: true, items, error: '' }
})

watch(
  scheduleExplanations,
  (newVal) => {
    if (newVal.isValid && isUserEdited.value) {
      const crons = newVal.items.map((i) => i.cron)
      const currentJoined = crons.join('\n')
      if (currentJoined !== lastEmittedCronJoined) {
        lastEmittedCronJoined = currentJoined
        reminders.value = crons
      }
    }
    emit('validity-change', newVal.isValid)
  },
  { immediate: true },
)
</script>

<template>
  <details>
    <summary>🛈 Reminder schedule syntax</summary>
    <p>
      Reminders can be entered as natural English text (e.g., "Every day at 8:00 AM" or "Every 6
      hours") or Cron syntax, where each schedule has 6 parts - second, minute, hour, day-of-month,
      month, and day-of-week; separate these by a space. Multiple reminder schedules can be
      specified, one per line.
    </p>
    <table>
      <thead>
        <tr>
          <th>Cron Syntax</th>
          <th>English Description</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td><code>0 0 8 * * *</code></td>
          <td>Every day at 8:00 AM</td>
        </tr>
        <tr>
          <td><code>0 30 8,19 * * *</code></td>
          <td>Every day at 8:30 AM and 7:30 PM</td>
        </tr>
        <tr>
          <td><code>0 0 7 * * 1</code></td>
          <td>Every Monday at 7:00 AM</td>
        </tr>
        <tr>
          <td><code>0 0 */6 * * *</code></td>
          <td>Every 6 hours</td>
        </tr>
        <tr>
          <td><code>0 0 9 1 * *</code></td>
          <td>On the 1st of every month at 9:00 AM</td>
        </tr>
      </tbody>
    </table>
  </details>
  <label for="reminder_schedules">Reminder schedules</label>
  <textarea
    id="reminder_schedules"
    :aria-invalid="scheduleExplanations.isValid ? undefined : true"
    placeholder="Reminders (e.g. Every day at 8:00 AM or 0 0 8 * * *)"
    v-model="userInput"
    @input="onInput"
  ></textarea>
  <small v-if="scheduleExplanations.isValid && scheduleExplanations.items.length > 0">
    <template v-for="(item, idx) in scheduleExplanations.items" :key="idx">
      {{ item.description }} (cron: <code>{{ item.cron }}</code
      >)<template v-if="idx < scheduleExplanations.items.length - 1">; </template>
    </template>
  </small>
  <small v-else-if="!scheduleExplanations.isValid">
    {{ scheduleExplanations.error }}
  </small>
</template>
