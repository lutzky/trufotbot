<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { AvailableDose, MedicationSummary } from '@/openapi'
import RelativeTime from './RelativeTime.vue'
import { formatDuration, intervalToDuration } from 'date-fns'

const props = defineProps<{ medication: MedicationSummary }>()

function renderCanTakeAbridged(next_doses: AvailableDose[]) {
  return next_doses
    .map((dose) => {
      const now = new Date()
      const duration = intervalToDuration({ start: new Date(), end: dose.time })
      const when =
        dose.time.getTime() > now.getTime() + 1000 * 60
          ? `in ${formatDuration(duration, {
              format: ['days', 'hours', 'minutes'],
            })}`
          : 'now'
      const quantityString = dose.quantity ? `${dose.quantity} ` : ''
      return `${quantityString} ${when}`
    })
    .join(', or ')
}
</script>

<template>
  <article style="cursor: pointer">
    <h2>{{ props.medication.name }} ›</h2>
    <footer>
      <p>
        <span v-if="props.medication.next_doses?.length > 0">
          Can take {{ renderCanTakeAbridged(props.medication.next_doses) }}.</span
        >
        <small v-else style="font-style: italic; font-size: 0.7em; color: var(--pico-muted-color)">
          No limits set
        </small>
      </p>
      <p>Last taken: <RelativeTime :time="props.medication.last_taken_at" /></p>
    </footer>
  </article>
</template>
