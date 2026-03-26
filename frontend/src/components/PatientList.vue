<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { patientsList, type Patient } from '@/openapi'

const patients = ref<Patient[]>([])

onMounted(async () => {
  await loadData()
})

async function loadData() {
  try {
    const { data: data } = await patientsList({})
    if (data == null) {
      throw new Error('Null patients list')
    }
    patients.value = data
  } catch (error) {
    console.error('Error fetching patients:', error)
  }
}

const props = defineProps({
  reloadOnIncrement: { type: Number, default: 0 },
})

watch(
  () => props.reloadOnIncrement,
  async () => {
    await loadData()
  },
)
</script>

<template>
  <h1>Select Patient</h1>
  <div class="grid">
    <button
      v-for="patient in patients"
      :key="patient.id"
      @click="$router.push({ name: 'patient', params: { id: patient.id } })"
    >
      {{ patient.name }}
    </button>
  </div>
</template>
