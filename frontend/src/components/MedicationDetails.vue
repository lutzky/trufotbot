<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { DoseLimit } from '@/openapi'
import { ref, watch } from 'vue'
import DoseLimitsEditor from './DoseLimitsEditor.vue'
import ReminderScheduleEditor from './ReminderScheduleEditor.vue'

defineProps<{ creating?: boolean }>()

const name = defineModel<string>('name', { required: true })
const description = defineModel<string | null>('description')
const inventory = defineModel<number | null>('inventory')
const doseLimits = defineModel<Array<DoseLimit>>('doseLimits', { required: true })
const reminders = defineModel<Array<string>>('reminders', { required: true })

const emit = defineEmits<{
  (e: 'update:isValid', isValid: boolean): void
}>()

const limitsAreValid = ref<boolean>(true)
const remindersAreValid = ref<boolean>(true)

watch(
  [limitsAreValid, remindersAreValid],
  () => {
    emit('update:isValid', limitsAreValid.value && remindersAreValid.value)
  },
  { immediate: true },
)
</script>

<template>
  <input type="text" placeholder="Medication name" v-model="name" />
  <textarea placeholder="Medication description" v-model="description"></textarea>
  <template v-if="!creating">
    <label>Inventory<input type="number" placeholder="Inventory" v-model="inventory" /></label>
    <ReminderScheduleEditor
      v-model:reminders="reminders"
      @validity-change="remindersAreValid = $event"
    />
    <DoseLimitsEditor v-model:doseLimits="doseLimits" @validity-change="limitsAreValid = $event" />
  </template>
</template>
