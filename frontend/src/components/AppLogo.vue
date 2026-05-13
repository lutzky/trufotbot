<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { ref } from 'vue'

const showVersion = ref(false)
const version = import.meta.env.VITE_APP_VERSION ?? 'unknown'

function toggleVersion() {
  showVersion.value = !showVersion.value
}
</script>

<template>
  <img src="/logo.svg" alt="" class="app-logo" @click="toggleVersion" />
  <kbd v-if="showVersion" class="app-version">{{ version }}</kbd>
</template>

<style scoped>
.app-logo {
  width: 32px;
  height: 32px;
  vertical-align: middle;
  margin-left: 0.5rem;
  float: right;
  cursor: pointer;
}

.app-version {
  float: right;
  margin-left: 0.5rem;
  margin-top: 0.25rem;
  font-size: 0.8rem;
}
</style>
