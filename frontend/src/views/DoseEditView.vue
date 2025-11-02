<script setup lang="ts">
import { dosesDelete, dosesGet, dosesUpdate, type GetDoseResponse } from '@/openapi'
import { getErrorMessage } from '@/errors'
import { onMounted, ref, watch } from 'vue'
import DoseDetails from '@/components/DoseDetails.vue'
import { useRouter } from 'vue-router'

const props = defineProps({
  patientId: {
    type: Number,
    required: true,
  },
  medicationId: {
    type: Number,
    required: true,
  },
  doseId: {
    type: Number,
    required: true,
  },
})

const dose = ref<GetDoseResponse>()
const isLoading = ref(false)
const loadError = ref<string | null>(null)

async function loadData() {
  dose.value = undefined
  isLoading.value = true
  loadError.value = null
  try {
    const { data } = await dosesGet({
      path: {
        patient_id: props.patientId,
        medication_id: props.medicationId,
        dose_id: props.doseId,
      },
    })
    dose.value = data
  } catch (error) {
    loadError.value = getErrorMessage(error)
  } finally {
    isLoading.value = false
  }
}

onMounted(loadData)

const router = useRouter()

const isSaved = ref(false)
const isSaving = ref(false)
const saveError = ref<string | null>(null)

const isDeleting = ref(false)

watch(
  () => dose.value,
  () => {
    isSaved.value = false
  },
  { deep: true },
)

async function updateDose() {
  if (!dose.value) {
    console.warn('Tried to updateDose before loading data')
    return
  }
  isSaved.value = false
  isSaving.value = true
  saveError.value = null
  try {
    await dosesUpdate({
      path: {
        patient_id: props.patientId,
        medication_id: props.medicationId,
        dose_id: props.doseId,
      },
      body: dose.value.dose.data,
    })
    isSaved.value = true
  } catch (error) {
    console.error('Error updating dose:', error)
    saveError.value = getErrorMessage(error)
  } finally {
    isSaving.value = false
  }
}

async function deleteDose() {
  if (!dose.value) {
    console.warn('Tried to deleteDose before loading data')
    return
  }
  if (
    !window.confirm(
      `Are you sure you want to delete this ${dose.value.medication_name} dose for ${dose.value.patient_name}?`,
    )
  ) {
    return
  }

  isDeleting.value = true
  saveError.value = null

  try {
    await dosesDelete({
      path: {
        patient_id: props.patientId,
        medication_id: props.medicationId,
        dose_id: props.doseId,
      },
    })
    router.push({
      name: 'patientMedicationDetail',
      params: {
        patientId: props.patientId,
        medicationId: props.medicationId,
      },
    })
  } catch (error) {
    console.error('Error deleting dose:', error)
    saveError.value = getErrorMessage(error)
  } finally {
    isDeleting.value = false
  }
}
</script>
<template>
  <div v-if="isLoading">
    <article aria-busy="true" />
  </div>
  <div v-else-if="loadError">
    <article class="pico-background-red">
      {{ loadError }}
    </article>
  </div>
  <div v-else-if="!dose">
    <article class="pico-background-red">Unexpectedly failed to load dose</article>
  </div>
  <div v-else>
    <RouterLink
      class="secondary"
      :to="{
        name: 'patientMedicationDetail',
        params: { patientId: patientId, medicationId: medicationId },
      }"
    >
      &lt; Back to {{ dose.patient_name }}'s {{ dose.medication_name }}
    </RouterLink>
    <hgroup>
      <h1>Dose {{ doseId }}</h1>
      <p>{{ dose.medication_name }} for {{ dose.patient_name }}</p>
    </hgroup>

    <DoseDetails
      v-model:quantity="dose.dose.data.quantity"
      v-model:takenAt="dose.dose.data.taken_at"
      v-model:notedBy="dose.dose.data.noted_by_user"
      @update:quantity="isSaved = false"
      @update:takenAt="isSaved = false"
      @update:notedBy="isSaved = false"
      :show-noted-by="true"
    />

    <article v-if="saveError" class="pico-background-red">
      {{ saveError }}
    </article>

    <div class="grid">
      <button @click="updateDose" :disabled="isSaving || isSaved" :aria-busy="isSaving">
        {{ isSaved ? 'Saved' : 'Save' }}
      </button>
      <button
        class="contrast"
        @click="deleteDose"
        :disabled="isSaving || isDeleting"
        :aria-busy="isDeleting"
      >
        Delete
      </button>
    </div>
  </div>
</template>
