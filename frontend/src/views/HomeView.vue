<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import PatientList from '@/components/PatientList.vue'
import PatientSettings from '@/components/PatientSettings.vue'
import { ref, computed, onMounted } from 'vue'
import {
  patientsCreate,
  statusGet,
  type PatientCreateRequest,
  type StatusResponse,
} from '@/openapi'
import { useUsername } from '@/username'

const reloadChildren = ref(0)
const userName = useUsername()

const patientToCreate = ref<PatientCreateRequest>({ name: '' })
const serverStatus = ref<StatusResponse | null>(null)

async function createPatient() {
  await patientsCreate({ body: patientToCreate.value })
  reloadChildren.value += 1
}

const localTimezone = Intl.DateTimeFormat().resolvedOptions().timeZone

const serverTimeFormatted = computed(() => {
  return serverStatus.value?.server_time ?? ''
})

const localTimeFormatted = computed(() => {
  if (!serverStatus.value) return new Date().toLocaleString()
  return new Date().toLocaleString()
})

onMounted(async () => {
  const { data } = await statusGet()
  serverStatus.value = data ?? null
})
</script>

<template>
  <PatientList :reloadOnIncrement="reloadChildren" />

  <hr />

  <label id="username-label" for="username"
    >User name:<input
      v-model="userName"
      type="text"
      id="username"
      placeholder="Who's giving the medication?"
  /></label>

  <hr />

  <details>
    <summary>Create patient</summary>
    <form @submit.prevent="createPatient">
      <PatientSettings
        v-model:name="patientToCreate.name"
        v-model:telegramGroupId="patientToCreate.telegram_group_id"
      >
        <template #inline-button>
          <button type="submit">Save</button>
        </template>
      </PatientSettings>
    </form>
  </details>

  <details>
    <summary>Timezone status</summary>
    <ul>
      <li>
        <strong>Server: </strong>
        <span v-if="serverStatus"
          >{{ serverTimeFormatted }} ({{ serverStatus.timezone }})
          <small>(configure via <code>TZ</code> environment variable)</small></span
        ><span v-else>Loading...</span>
      </li>
      <li>
        <strong>Your browser: </strong>
        <span>{{ localTimeFormatted }} ({{ localTimezone }})</span>
      </li>
    </ul>
  </details>
</template>
