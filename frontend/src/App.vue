<script setup lang="ts">
import { RouterView } from 'vue-router'
import NoUserConfiguredWarning from './components/NoUserConfiguredWarning.vue'
</script>

<template>
  <NoUserConfiguredWarning />
  <RouterView />
</template>
