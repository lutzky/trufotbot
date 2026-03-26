<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { useUsername } from '@/username'

const username = useUsername()
</script>

<template>
  <article v-if="!username" class="pico-background-amber">
    <strong>Warning:</strong> No user set. Please configure your username
    <a href="/#username-label">in the settings</a>.
  </article>
</template>
