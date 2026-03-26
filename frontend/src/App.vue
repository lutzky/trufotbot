<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { RouterView } from 'vue-router'
import NoUserConfiguredWarning from './components/NoUserConfiguredWarning.vue'
</script>

<template>
  <NoUserConfiguredWarning />
  <RouterView />
</template>
