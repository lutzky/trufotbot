<script setup lang="ts">
import PatientList from '@/components/PatientList.vue'
import PatientSettings from '@/components/PatientSettings.vue'
import { ref } from 'vue'
import { patientsCreate, type PatientCreateRequest } from '@/openapi'
import { useUsername } from '@/username'

const reloadChildren = ref(0)
const userName = useUsername()

const patientToCreate = ref<PatientCreateRequest>({ name: '' })

async function createPatient() {
  await patientsCreate({ body: patientToCreate.value })
  reloadChildren.value += 1
}
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
</template>
