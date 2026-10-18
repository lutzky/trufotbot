<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { computed } from 'vue'
import type { TelegramGroupId } from '@/openapi/types.gen'

const name = defineModel<string>('name', { required: true })
// Telegram group IDs are strings on the wire; converting them to numbers could silently lose
// precision, so they stay strings in the frontend too.
const telegramGroupId = defineModel<TelegramGroupId | null>('telegramGroupId')

// A cleared <input> yields "" rather than null, which the server rejects.
const telegramGroupIdInput = computed<string>({
  get: () => telegramGroupId.value ?? '',
  set: (value) => {
    const trimmed = value.trim()
    telegramGroupId.value = trimmed === '' ? null : trimmed
  },
})
</script>

<template>
  <fieldset role="group">
    <input v-model="name" type="text" placeholder="Name" aria-label="Name" />
    <input
      v-model="telegramGroupIdInput"
      type="text"
      inputmode="numeric"
      pattern="-?[0-9]+"
      placeholder="Telegram Group ID"
      aria-label="Telegram Group ID"
    />
    <slot name="inline-button" />
  </fieldset>
</template>
