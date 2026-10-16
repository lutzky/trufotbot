<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { computed } from 'vue'

const name = defineModel<string>('name', { required: true })
const telegramGroupId = defineModel<number | null>('telegramGroupId')

// A cleared <input type="number"> yields "" rather than null, which the server rejects.
const telegramGroupIdInput = computed<number | string>({
  get: () => telegramGroupId.value ?? '',
  set: (value) => {
    telegramGroupId.value = value === '' || value == null ? null : Number(value)
  },
})
</script>

<template>
  <fieldset role="group">
    <input v-model="name" type="text" placeholder="Name" aria-label="Name" />
    <input
      v-model="telegramGroupIdInput"
      type="number"
      placeholder="Telegram Group ID"
      aria-label="Telegram Group ID"
    />
    <slot name="inline-button" />
  </fieldset>
</template>
