<script setup lang="ts">
import cronstrue from 'cronstrue'
import type { DoseLimit } from '@/openapi'
import { computed, ref, watch, watchEffect } from 'vue'

defineProps<{ creating?: boolean }>()

const name = defineModel<string>('name', { required: true })
const description = defineModel<string | null>('description')
const inventory = defineModel<number | null>('inventory')
const doseLimits = defineModel<Array<DoseLimit>>('doseLimits', { required: true })
const reminders = defineModel<Array<string>>('reminders', { required: true })

const emit = defineEmits<{
  (e: 'update:isValid', isValid: boolean): void
}>()

const cronScheduleAsString = computed<string>({
  get() {
    return reminders.value.join('\n')
  },
  set(newValue) {
    reminders.value = newValue.split('\n').filter((s) => s)
  },
})

const scheduleExplanations = computed<{ isValid: boolean; explanation: string }>(() => {
  try {
    return {
      isValid: true,
      explanation: reminders.value
        .map((s) => {
          try {
            if (s.split(' ').length != 6) {
              throw 'Only 6-part cron schedules are supported'
            }
            return cronstrue.toString(s)
          } catch (error) {
            throw `Failed to parse ${JSON.stringify(s)}: ${error}`
          }
        })
        .join('; '),
    }
  } catch (error) {
    return { isValid: false, explanation: `${error}` }
  }
})

const rawLimitsInput = ref<string>('')
const invalidLimits = ref<boolean>(false)

watch([scheduleExplanations, invalidLimits], () => {
  emit('update:isValid', !invalidLimits.value && scheduleExplanations.value.isValid)
})

watchEffect(() => {
  rawLimitsInput.value = doseLimits.value.map((lim) => `${lim.hours}:${lim.amount}`).join(',')
})

function parseLimitsInput() {
  const parts = rawLimitsInput.value.split(',').filter((s) => s)
  const parsed = []

  for (const part of parts) {
    const [hoursStr, amountStr] = part.split(':')
    const hours = parseInt(hoursStr)
    const amount = parseFloat(amountStr)

    if (!isNaN(hours) && !isNaN(amount)) {
      parsed.push({ hours, amount })
    } else {
      invalidLimits.value = true
      return
    }
  }

  invalidLimits.value = false
  doseLimits.value = parsed
}
</script>

<template>
  <form>
    <input type="string" placeholder="Medication name" v-model="name" />
    <textarea placeholder="Medication description" v-model="description"></textarea>
    <template v-if="!creating">
      <label>Inventory<input type="number" placeholder="Inventory" v-model="inventory" /></label>
      <details>
        <summary>🛈 Reminder schedule syntax</summary>
        <p>
          Reminders use Cron syntax, where each schedule has 6 parts - second, minute, hour,
          day-of-month, month, and day-of-week; separate these by a space. Hours are 24-hour-based.
          Multiple reminder schedules can be specified, one per line.
        </p>
        <table>
          <thead>
            <tr>
              <th>S</th>
              <th>M</th>
              <th>H</th>
              <th>D</th>
              <th>M</th>
              <th>W</th>
              <th>Explanation</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>0</td>
              <td>0</td>
              <td>8</td>
              <td>*</td>
              <td>*</td>
              <td>*</td>
              <td>Every day at 8:00 AM</td>
            </tr>
            <tr>
              <td>0</td>
              <td>30</td>
              <td>8,19</td>
              <td>*</td>
              <td>*</td>
              <td>*</td>
              <td>Every day at 8:30 AM and 7:30 PM</td>
            </tr>
            <tr>
              <td>0</td>
              <td>0</td>
              <td>7</td>
              <td>*</td>
              <td>*</td>
              <td>1</td>
              <td>Every Monday at 7:00 AM</td>
            </tr>
            <tr>
              <td>0</td>
              <td>0</td>
              <td>9</td>
              <td>1</td>
              <td>*</td>
              <td>*</td>
              <td>On the 1st of every month at 9:00 AM</td>
            </tr>
            <tr>
              <td>0</td>
              <td>0</td>
              <td>*/6</td>
              <td>*</td>
              <td>*</td>
              <td>*</td>
              <td>Every 6 hours</td>
            </tr>
          </tbody>
        </table>
      </details>
      <label for="reminder_schedules">Reminder schedules</label>
      <textarea
        id="reminder_schedules"
        :aria-invalid="!scheduleExplanations.isValid"
        placeholder="Reminders (cron schedules)"
        v-model="cronScheduleAsString"
      ></textarea>
      <small>{{ scheduleExplanations.explanation }}</small>
      <label for="limits">Limits</label>
      <textarea
        id="limits"
        :aria-invalid="invalidLimits"
        placeholder="Limits (hours:amount,hours:amount,...)"
        v-model="rawLimitsInput"
        @input="parseLimitsInput"
      ></textarea>
    </template>
  </form>
</template>
