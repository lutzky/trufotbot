<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { format, formatDistanceToNow } from 'date-fns'
defineProps<{
  time: Date | null | undefined
  /**
   * If true, clamps the display of past times to "now".
   * For example, instead of "5 minutes ago", it will display "now".
   * This is useful for situations where displaying a past due time isn't logical.
   */
  clampFuture?: boolean
}>()
</script>

<template>
  <template v-if="time">
    <template v-if="clampFuture && time.getTime() <= Date.now()">now</template>
    <template v-else>
      {{ formatDistanceToNow(time, { addSuffix: true }) }}
      <small style="font-size: 0.7em; color: var(--pico-muted-color)">
        {{ format(time, 'yyyy-MM-dd (EEE) HH:mm') }}
      </small>
    </template>
  </template>
  <template v-else> never</template>
</template>
