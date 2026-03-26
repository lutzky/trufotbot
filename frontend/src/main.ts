// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import './assets/main.scss'

import { createApp } from 'vue'
import App from './App.vue'
import router from './router'

const app = createApp(App)

app.use(router)

app.mount('#app')
