<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { AvailableDose, MedicationSummary } from '@/openapi'
import RelativeTime from './RelativeTime.vue'
import { formatDuration, intervalToDuration } from 'date-fns'

const props = defineProps<{ medication: MedicationSummary; highlighted?: boolean }>()

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
  <article style="cursor: pointer" :class="{ 'hl-card': props.highlighted }">
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
      <p :class="{ 'hl-text': props.highlighted }">
        Last taken: <RelativeTime :time="props.medication.last_taken_at" />
      </p>
    </footer>
  </article>
</template>

<style scoped>
.hl-card {
  animation: glow-pulse 1s ease-out forwards;
}

@keyframes glow-pulse {
  0% {
    box-shadow: var(--pico-card-box-shadow);
  }
  50% {
    box-shadow:
      var(--pico-card-box-shadow),
      0 0 20px 0px color-mix(in srgb, var(--pico-primary), transparent 50%);
  }
  100% {
    box-shadow: var(--pico-card-box-shadow);
  }
}

.hl-text {
  animation: text-glow 1s ease-out forwards;
  position: relative;
  text-decoration: none;
}

@keyframes text-glow {
  0% {
    text-shadow: 0 0 0px transparent;
    color: inherit;
  }
  50% {
    text-shadow: 0 0 8px var(--pico-primary);
    color: var(--pico-primary);
  }
  100% {
    text-shadow: 0 0 0px transparent;
    color: inherit;
  }
}

.hl-text::after {
  content: '';
  position: absolute;
  left: 0;
  bottom: -2px;
  width: 100%;
  height: 2px;
  background-color: var(--pico-primary);
  transform: scaleX(0);
  opacity: 0;
  transition: none;
  animation: line-pulse 1s ease-out forwards;
}

@keyframes line-pulse {
  0% {
    transform: scaleX(0);
    opacity: 0;
  }
  50% {
    transform: scaleX(1);
    opacity: 1;
  }
  100% {
    transform: scaleX(1);
    opacity: 0;
  }
}
</style>
