<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { Temporal } from '@js-temporal/polyfill'
import { onMounted, ref } from 'vue'

const daysUntilSwitch = ref<number | null>(null)
const closestSwitch = ref<Temporal.ZonedDateTime | null>(null)

onMounted(() => {
  const now = Temporal.Now.zonedDateTimeISO()
  const nextTransition = now.getTimeZoneTransition({ direction: 'next' })
  const prevTransition = now.getTimeZoneTransition({ direction: 'previous' })

  const daysUntilNext = nextTransition ? now.until(nextTransition).total({ unit: 'days' }) : 365
  const daysSincePrevious = prevTransition ? prevTransition.until(now).total({ unit: 'days' }) : 365

  closestSwitch.value = daysUntilNext < daysSincePrevious ? nextTransition : prevTransition
  daysUntilSwitch.value = closestSwitch.value
    ? Math.round(now.until(closestSwitch.value).total({ unit: 'days' }))
    : null
})
</script>

<template>
  <article
    v-if="daysUntilSwitch !== null && Math.abs(daysUntilSwitch) <= 2"
    class="pico-background-amber"
  >
    <strong>Warning:</strong> Daylight saving time change
    <span v-if="daysUntilSwitch >= 0">
      in {{ daysUntilSwitch }} day{{ daysUntilSwitch === 1 ? '' : 's' }}.
    </span>
    <span v-else> {{ -daysUntilSwitch }} day{{ daysUntilSwitch === -1 ? '' : 's' }} ago. </span>

    You should restart TrufotBot after this change, or reminders may fire at unexpected times. See
    <a href="https://github.com/lutzky/trufotbot/issues/30">#30</a>.
  </article>
</template>
