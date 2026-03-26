<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import MedicationDetails from '@/components/MedicationDetails.vue'
import PatientMedicationSummaryCard from '@/components/PatientMedicationSummaryCard.vue'
import PatientSettings from '@/components/PatientSettings.vue'
import { getErrorMessage } from '@/errors'
import {
  medicationCreate,
  patientsDelete,
  patientsGet,
  patientsUpdate,
  type PatientCreateRequest,
  type PatientMedicationUpdateRequest,
  type PatientsGetResponse,
} from '@/openapi'
import { useRouter } from 'vue-router'

const router = useRouter()

const props = defineProps({
  id: {
    type: Number,
    required: true,
  },
})

const patientDetails = ref<PatientsGetResponse>()
const patientId: number = props.id

const everTakenMedications = computed(() => {
  if (!patientDetails.value?.medications) {
    return []
  }
  return patientDetails.value.medications
    .filter((med) => med.last_taken_at)
    .sort((a, b) => new Date(b.last_taken_at!).getTime() - new Date(a.last_taken_at!).getTime())
})

const neverTakenMedications = computed(() => {
  if (!patientDetails.value?.medications) {
    return []
  }
  return patientDetails.value.medications.filter((med) => !med.last_taken_at)
})

const isLoading = ref(true)
const loadError = ref<string | null>(null)

const isSaving = ref(false)
const isSaved = ref(false)
const saveError = ref<string | null>(null)

const isDeleting = ref(false)

async function loadData() {
  try {
    isLoading.value = true
    loadError.value = null
    const { data } = await patientsGet({ path: { id: patientId } })
    patientDetails.value = data
  } catch (error) {
    loadError.value = getErrorMessage(error)
  } finally {
    isLoading.value = false
  }
}

onMounted(loadData)

watch(
  () => patientDetails,
  () => {
    isSaved.value = false
  },
  { deep: true },
)

function goToMedicationDetail(medicationId: number) {
  router.push({
    name: 'patientMedicationDetail',
    params: { patientId: patientId, medicationId: medicationId },
  })
}

async function savePatient() {
  isSaving.value = true
  isSaved.value = false
  saveError.value = null
  try {
    if (!patientDetails.value) {
      throw 'stuff'
    }
    const body: PatientCreateRequest = patientDetails.value
    await patientsUpdate({ path: { id: patientId }, body })
    isSaved.value = true
  } catch (err) {
    saveError.value = getErrorMessage(err)
  } finally {
    isSaving.value = false
  }
}

async function deletePatient() {
  if (
    !window.confirm(
      `Are you sure you want to delete ${patientDetails.value?.name ?? 'this patient'}?`,
    )
  ) {
    return
  }
  try {
    isDeleting.value = true
    saveError.value = null
    await patientsDelete({ path: { id: patientId } })
    router.push({ name: 'home' })
  } catch (err) {
    saveError.value = getErrorMessage(err)
  } finally {
    isDeleting.value = false
  }
}

const medicationToCreate = ref<PatientMedicationUpdateRequest>({
  medication: { name: '', dose_limits: [] },
  reminders: { cron_schedules: [] },
})

const isCreatingMedication = ref(false)
const createError = ref<string | null>(null)

async function createMedication() {
  isCreatingMedication.value = true
  createError.value = null
  try {
    await medicationCreate({ body: medicationToCreate.value.medication })
    loadData()
  } catch (err) {
    createError.value = getErrorMessage(err)
  } finally {
    isCreatingMedication.value = false
  }
}
</script>

<template>
  <a href="/" class="secondary">&lt; Back to Patient List</a>
  <template v-if="isLoading">
    <article aria-busy="true" />
  </template>
  <template v-else-if="loadError">
    <article class="pico-background-red">{{ loadError }}</article>
  </template>
  <template v-else-if="!patientDetails">
    <article class="pico-background-red">No patient details available</article>
  </template>
  <template v-else-if="patientDetails">
    <h1>Medications for {{ patientDetails.name }}</h1>

    <div
      v-for="medication in everTakenMedications"
      :key="medication.id"
      @click="goToMedicationDetail(medication.id)"
    >
      <PatientMedicationSummaryCard :medication="medication" />
    </div>

    <hr v-if="everTakenMedications.length > 0 && neverTakenMedications.length > 0" />

    <div
      v-for="medication in neverTakenMedications"
      :key="medication.id"
      @click="goToMedicationDetail(medication.id)"
    >
      <PatientMedicationSummaryCard :medication="medication" />
    </div>

    <details>
      <summary>Edit patient</summary>
      <form @submit.prevent="savePatient">
        <PatientSettings
          v-model:name="patientDetails.name"
          v-model:telegramGroupId="patientDetails.telegram_group_id"
        />
        <article v-if="saveError" class="pico-background-red">
          {{ saveError }}
        </article>
        <div class="grid">
          <button type="submit" :disabled="isSaving || isSaved" :aria-busy="isSaving">
            {{ isSaved ? 'Saved' : 'Save' }}
          </button>
          <button
            type="button"
            class="contrast"
            @click="deletePatient"
            :disabled="isDeleting"
            :aria-busy="isDeleting"
          >
            Delete Patient
          </button>
        </div>
      </form>
    </details>
    <hr />
    <details>
      <summary>Add new medication</summary>
      <form @submit.prevent="createMedication">
        <MedicationDetails
          v-model:name="medicationToCreate.medication.name"
          v-model:description="medicationToCreate.medication.description"
          v-model:doseLimits="medicationToCreate.medication.dose_limits"
          v-model:reminders="medicationToCreate.reminders.cron_schedules"
          :creating="true"
        />
        <article v-if="createError" class="pico-background-red">
          {{ createError }}
        </article>
        <div class="grid">
          <button type="submit" :disabled="isCreatingMedication" :aria-busy="isCreatingMedication">
            Create
          </button>
        </div>
      </form>
    </details>
  </template>
</template>
