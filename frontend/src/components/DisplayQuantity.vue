<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<script setup lang="ts">
const props = withDefaults(
  defineProps<{
    value: number | null | undefined
    precision?: number
  }>(),
  {
    precision: 4,
  },
)

function formatQuantity(n: number | null | undefined): string {
  if (n == null || n === 0) return '0'
  const rounded = parseFloat(n.toPrecision(props.precision))
  return rounded.toString()
}
</script>

<template>{{ formatQuantity(value) }}</template>
