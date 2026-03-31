<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
import { getErrorMessage } from '@/errors'
import RelativeTime from '@/components/RelativeTime.vue'
import {
  dosesList,
  dosesRecord,
  medicationDelete,
  medicationUpdate,
  type CreateDose,
  type PatientGetDosesResponse,
  type PatientMedicationUpdateRequest,
} from '@/openapi'
import DoseDetails from '@/components/DoseDetails.vue'
import MedicationDetails from '@/components/MedicationDetails.vue'
import { computed, onMounted, ref, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'

const isLoading = ref(true)
const loadError = ref<string | null>(null)

const isMedicationSaving = ref(false)
const isMedicationSaved = ref(false)
const medicationSaveError = ref<string | null>(null)

const isMedicationDeleting = ref(false)

const showDoseRecordedNotification = ref(false)
const isNotificationHiding = ref(false)
let notificationTimer: number | null = null

const props = defineProps({
  patientId: {
    type: Number,
    required: true,
  },
  medicationId: {
    type: Number,
    required: true,
  },
  reminderMessageId: {
    type: Number,
    required: false,
  },
  reminderMessageTimestamp: {
    type: Number,
    required: false,
  },
})

function latestQuantity(response: PatientGetDosesResponse | undefined): number {
  return (response?.doses ?? []).map((d) => d.data.quantity).find((q) => q > 0) ?? 1
}

import { useUsername } from '@/username'

const username = useUsername()

const doseToCreate = ref<CreateDose>({
  quantity: 1,
  taken_at: new Date(),
  noted_by_user: username.value,
})

const dosesResponse = ref<PatientGetDosesResponse | null>(null)
const showZeroDoses = ref(true)

const filteredDoses = computed(() => {
  if (!dosesResponse.value) {
    return []
  }
  if (showZeroDoses.value) {
    return dosesResponse.value.doses
  }
  return dosesResponse.value.doses.filter((d) => d.data.quantity > 0)
})

// This is a weird decision from the API itself... creating a medication doesn't supply
// reminders, but updating it does. That's because reminders are per-patient, but medication is
// created independently of patients.
const medication = ref<PatientMedicationUpdateRequest>({
  medication: { name: '', dose_limits: [] },
  reminders: { cron_schedules: [] },
})

async function loadData() {
  isLoading.value = true
  dosesResponse.value = null
  loadError.value = null
  try {
    const { data } = await dosesList({
      path: { patient_id: props.patientId, medication_id: props.medicationId },
    })
    if (!data) {
      throw 'Nil data returned'
    }
    isLoading.value = false
    dosesResponse.value = data
    doseToCreate.value.quantity = latestQuantity(data)
    medication.value.medication = data?.medication
    medication.value.reminders = data?.reminders
  } catch (err) {
    loadError.value = getErrorMessage(err)
  } finally {
    isLoading.value = false
  }
}

onMounted(loadData)

onUnmounted(() => {
  if (notificationTimer) {
    clearTimeout(notificationTimer)
  }
})

const medicationFormValid = ref(true)

function handleMedicationFormValidity(isValid: boolean) {
  medicationFormValid.value = isValid
}

async function notifyDoseRecorded() {
  showDoseRecordedNotification.value = true
  if (notificationTimer) {
    clearTimeout(notificationTimer)
  }
  notificationTimer = setTimeout(() => {
    isNotificationHiding.value = true
    setTimeout(() => {
      showDoseRecordedNotification.value = false
      isNotificationHiding.value = false
      notificationTimer = null
    }, 300)
  }, 3000)
}

async function logDose() {
  const params: Parameters<typeof dosesRecord>[0] = {
    path: { patient_id: props.patientId, medication_id: props.medicationId },
    body: doseToCreate.value,
  }
  if (props.reminderMessageId && props.reminderMessageTimestamp) {
    params.query = {
      reminder_message_id: props.reminderMessageId,
      reminder_sent_time: new Date(props.reminderMessageTimestamp * 1000),
    }
  }
  await dosesRecord(params)
  loadData()

  notifyDoseRecorded()

  // We are no longer responding to a reminder, so remove the query parameters from the URL.
  // This prevents accidental re-use of the same reminder link.
  if (props.reminderMessageId || props.reminderMessageTimestamp) {
    router.replace({
      name: 'patientMedicationDetail',
      params: {
        patientId: props.patientId,
        medicationId: props.medicationId,
      },
    })
  }
}

async function saveMedication() {
  isMedicationSaving.value = true
  medicationSaveError.value = null
  try {
    await medicationUpdate({
      path: { patient_id: props.patientId, medication_id: props.medicationId },
      body: medication.value,
    })
    isMedicationSaved.value = true
  } catch (error) {
    medicationSaveError.value = getErrorMessage(error)
  } finally {
    isMedicationSaving.value = false
  }
}

const router = useRouter()

async function deleteMedication() {
  try {
    isMedicationDeleting.value = true
    medicationSaveError.value = null

    if (
      !window.confirm(
        `Are you sure you want to delete ${medication.value?.medication.name ?? 'this medication'}?`,
      )
    ) {
      return
    }
    await medicationDelete({ path: { id: props.medicationId } })
    router.push({ name: 'patient', params: { id: props.patientId } })
  } catch (error) {
    medicationSaveError.value = getErrorMessage(error)
  } finally {
    isMedicationDeleting.value = false
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
  <div v-else-if="!dosesResponse">
    <article class="pico-background-red">No dose response available</article>
  </div>
  <div v-else>
    <article
      v-if="showDoseRecordedNotification"
      class="dose-recorded-notification"
      :class="{ hiding: isNotificationHiding }"
    >
      <span class="notification-icon">✓</span>
      Dose recorded successfully!
    </article>

    <RouterLink class="secondary" :to="{ name: 'patient', params: { id: patientId } }">
      &lt; Back to {{ dosesResponse.patient_name }}
    </RouterLink>

    <hgroup>
      <h1>
        <img src="/logo.svg" alt="" id="app-logo" />
        {{ dosesResponse.medication.name }}
      </h1>
      <p>{{ dosesResponse.medication.description }}</p>
    </hgroup>

    <article v-if="dosesResponse.next_doses.length == 1">
      Can take {{ dosesResponse.next_doses[0].quantity }}
      <RelativeTime :time="dosesResponse.next_doses[0].time" :clampFuture="true" />
    </article>
    <article v-else-if="dosesResponse.next_doses.length > 0">
      <p>Can take:</p>
      <ul>
        <li
          v-for="next_dose in dosesResponse.next_doses"
          :key="`${next_dose.time}-${next_dose.quantity}`"
        >
          {{ next_dose.quantity }}
          <RelativeTime :time="next_dose.time" :clampFuture="true" />
        </li>
      </ul>
    </article>
    <small v-else style="font-style: italic; font-size: 0.7em; color: var(--pico-muted-color)">
      No limits set
    </small>
    <form @submit.prevent="logDose">
      <DoseDetails
        v-model:takenAt="doseToCreate.taken_at"
        v-model:quantity="doseToCreate.quantity"
      />
      <button type="submit">Log dose</button>
      <p v-if="medication.medication.inventory">
        <small
          >Inventory after this dose: {{ medication.medication.inventory }} →
          {{ medication.medication.inventory - doseToCreate.quantity }}
        </small>
      </p>
    </form>
    <div v-if="reminderMessageId">
      <small>Note: To mark this as a "skipped" dose, set the quantity to 0.</small>
    </div>
    <h2>Dose history</h2>
    <label>
      <input type="checkbox" v-model="showZeroDoses" role="switch" />
      Show skipped (0-quantity) doses
    </label>
    <table>
      <thead>
        <tr>
          <th>Time taken</th>
          <th>Quantity</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="dose in filteredDoses"
          :key="dose.id"
          :class="{ 'zero-dose': dose.data.quantity === 0 }"
        >
          <td>
            <RelativeTime :time="dose.data.taken_at" />
          </td>
          <td>{{ dose.data.quantity }}</td>
          <td style="text-align: right">
            <RouterLink
              class="secondary"
              :to="{
                name: 'doseEdit',
                params: { patientId: patientId, medicationId: medicationId, doseId: dose.id },
              }"
            >
              <span class="material-symbols-rounded">edit</span>
            </RouterLink>
          </td>
        </tr>
      </tbody>
    </table>
    <details>
      <summary>Edit medication</summary>
      <form @submit.prevent="saveMedication">
        <MedicationDetails
          v-model:name="medication.medication.name"
          v-model:description="medication.medication.description"
          v-model:inventory="medication.medication.inventory"
          v-model:doseLimits="medication.medication.dose_limits"
          v-model:reminders="medication.reminders.cron_schedules"
          @update:isValid="handleMedicationFormValidity"
          @update:name="isMedicationSaved = false"
          @update:description="isMedicationSaved = false"
          @update:inventory="isMedicationSaved = false"
          @update:doseLimits="isMedicationSaved = false"
          @update:reminders="isMedicationSaved = false"
        />
        <article v-if="medicationSaveError" class="pico-background-red">
          {{ medicationSaveError }}
        </article>
        <div class="grid">
          <button
            type="submit"
            @click="saveMedication"
            :disabled="
              !medicationFormValid ||
              isMedicationSaving ||
              isMedicationDeleting ||
              isMedicationSaved
            "
            :aria-busy="isMedicationSaving"
          >
            Save
          </button>
          <button
            type="button"
            @click="deleteMedication"
            class="contrast"
            :aria-busy="isMedicationDeleting"
            :disabled="isMedicationSaving || isMedicationDeleting"
          >
            Delete
          </button>
        </div>
      </form>
    </details>
  </div>
</template>

<style scoped>
.zero-dose td {
  color: var(--pico-muted-color);
  background-color: var(--pico-table-row-stripped-background-color);
}

.dose-recorded-notification {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 1000;
  background-color: var(--pico-background-color);
  border: 1px solid var(--pico-primary-border);
  border-left: 4px solid var(--pico-primary);
  padding: 1rem;
  animation: slideIn 0.3s ease-out;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.notification-icon {
  color: var(--pico-primary);
  font-weight: bold;
  font-size: 1.2em;
}

.dose-recorded-notification.hiding {
  animation: slideOut 0.3s ease-in forwards;
}

@keyframes slideIn {
  from {
    transform: translateX(100%);
    opacity: 0;
  }
  to {
    transform: translateX(0);
    opacity: 1;
  }
}

@keyframes slideOut {
  from {
    transform: translateX(0);
    opacity: 1;
  }
  to {
    transform: translateX(100%);
    opacity: 0;
  }
}
</style>
