<script setup lang="ts">
import { lightFormat, parse } from 'date-fns'
import { computed, defineModel } from 'vue'

// Using this instead of formatISO because it can't have a timezone; see:
// * https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/datetime-local
// * https://github.com/orgs/date-fns/discussions/2834
//
// ...and because we also don't want second-level resolution.
const localISOFormat = `yyyy-MM-dd'T'HH:mm`

defineProps<{ showNotedBy?: boolean }>()

const takenAt = defineModel<Date>('takenAt', { required: true })
const quantity = defineModel<number>('quantity', { required: true })
const notedBy = defineModel<string | null>('notedBy')

const notedByForInput = computed<string>({
  get() {
    return notedBy.value ?? ''
  },
  set(newValue) {
    notedBy.value = newValue === '' ? null : newValue
  },
})

const takenAtAsString = computed<string>({
  get() {
    if (!takenAt.value) {
      return ''
    }
    return lightFormat(takenAt.value, localISOFormat)
  },
  set(newValue) {
    takenAt.value = parse(newValue, localISOFormat, new Date())
  },
})
</script>

<template>
  <input type="datetime-local" v-model="takenAtAsString" step="60" />
  <input
    name="quantity"
    v-model="quantity"
    aria-label="Quantity"
    type="number"
    step="any"
    placeholder="How much of it?"
  />
  <input
    v-if="showNotedBy"
    v-model="notedByForInput"
    name="noted-by"
    aria-label="Noted by"
    placeholder="Who gave this?"
    type="text"
  />
</template>
