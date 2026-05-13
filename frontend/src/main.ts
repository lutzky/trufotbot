// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import './assets/main.scss'

import { createApp } from 'vue'
import App from './App.vue'
import router from './router'

const app = createApp(App)

console.log(`%c TrufotBot Version: ${import.meta.env.VITE_APP_VERSION} `, 'background: #42b883; color: #fff; font-weight: bold;');

app.use(router)

app.mount('#app')
