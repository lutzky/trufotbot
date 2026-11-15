<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import type { DoseLimit } from '@/openapi'
import { ref, watch } from 'vue'
import AliasesEditor from './AliasesEditor.vue'
import DoseLimitsEditor from './DoseLimitsEditor.vue'
import ReminderScheduleEditor from './ReminderScheduleEditor.vue'

defineProps<{ creating?: boolean; patientName?: string }>()

const name = defineModel<string>('name', { required: true })
const description = defineModel<string | null>('description')
const inventory = defineModel<number | null>('inventory')
const doseLimits = defineModel<Array<DoseLimit>>('doseLimits', { required: true })
const reminders = defineModel<Array<string>>('reminders', { required: true })
const aliases = defineModel<Array<string>>('aliases', { default: () => [] })
const naturalInput = defineModel<boolean | null>('naturalInput', { default: false })

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
    <AliasesEditor v-model:aliases="aliases" />
    <ReminderScheduleEditor
      v-model:reminders="reminders"
      @validity-change="remindersAreValid = $event"
    />
    <label>
      <input type="checkbox" role="switch" v-model="naturalInput" />
      Allow natural input{{ patientName ? ` for ${patientName}` : '' }}
    </label>
    <p>
      <small>
        Allow logging doses in Telegram by sending a message like
        <code>{{ name || 'Medication' }} 5</code> in
        {{ patientName ? `${patientName}'s` : "this patient's" }} group.
      </small>
    </p>
    <DoseLimitsEditor v-model:doseLimits="doseLimits" @validity-change="limitsAreValid = $event" />
  </template>
</template>
