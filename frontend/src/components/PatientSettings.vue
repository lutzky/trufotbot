<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
const name = defineModel<string>('name', { required: true })
const telegramGroupId = defineModel<number | null>('telegramGroupId')
</script>

<template>
  <fieldset role="group">
    <input v-model="name" type="text" placeholder="Name" aria-label="Name" />
    <input
      v-model="telegramGroupId"
      type="number"
      placeholder="Telegram Group ID"
      aria-label="Telegram Group ID"
    />
    <slot name="inline-button" />
  </fieldset>
</template>
