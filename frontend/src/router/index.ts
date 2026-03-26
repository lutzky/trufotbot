// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { createRouter, createWebHistory } from 'vue-router'
import HomeView from '@/views/HomeView.vue'
import DoseEditView from '@/views/DoseEditView.vue'
import PatientDetailView from '@/views/PatientDetailView.vue'
import PatientMedicationDetailView from '@/views/PatientMedicationDetailView.vue'

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: HomeView,
    },

    {
      path: '/patients/:id',
      name: 'patient',
      component: PatientDetailView,
      props: (route) => ({
        id: parseInt(route.params.id as string),
      }),
    },
    {
      path: '/patients/:patientId/medications/:medicationId',
      name: 'patientMedicationDetail',
      component: PatientMedicationDetailView,
      props: (route) => ({
        patientId: parseInt(route.params.patientId as string),
        medicationId: parseInt(route.params.medicationId as string),
        reminderMessageId: route.query.message_id
          ? parseInt(route.query.message_id as string)
          : undefined,
        reminderMessageTimestamp: route.query.message_time
          ? parseInt(route.query.message_time as string)
          : undefined,
      }),
    },
    {
      path: '/patients/:patientId/medications/:medicationId/doses/:doseId',
      name: 'doseEdit',
      component: DoseEditView,
      props: (route) => ({
        patientId: parseInt(route.params.patientId as string),
        medicationId: parseInt(route.params.medicationId as string),
        doseId: parseInt(route.params.doseId as string),
      }),
    },
  ],
})

export default router
