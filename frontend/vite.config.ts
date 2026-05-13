// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

import { fileURLToPath, URL } from 'node:url'

import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'
import viteFaviconPlugin from 'vite-plugin-favicon-generator'
import { execSync } from 'node:child_process'

const getVersion = () => {
  if (process.env.VITE_APP_VERSION) return process.env.VITE_APP_VERSION;
  try {
    return execSync('git describe --tags --dirty --always').toString().trim();
  } catch {
    return 'vERROR-unknown-version';
  }
}

// https://vite.dev/config/
export default defineConfig({
  define: {
    'import.meta.env.VITE_APP_VERSION': JSON.stringify(getVersion()),
  },
  plugins: [
    vue(),
    vueDevTools(),
    viteFaviconPlugin({
      source: '../logo.svg',
      outputDir: 'public/favicons',
      publicPath: '/favicons',
      appName: 'TrufotBot',
      appShortName: 'TrufotBot',
      appDescription: 'Household medication management system',
    }),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      }
    }
  }
})
