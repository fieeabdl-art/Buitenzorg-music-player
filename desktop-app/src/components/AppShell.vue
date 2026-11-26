<script setup lang="ts">
import { AlertCircle, Check, LoaderCircle, MonitorDown } from '@lucide/vue'
import { ref, watch } from 'vue'
import { useRoute } from 'vue-router'
import { useAppStore } from '../stores/app'
import PersistentPlayer from './PersistentPlayer.vue'
import TopNavigation from './TopNavigation.vue'

const appStore = useAppStore()
const route = useRoute()
const mainContent = ref<HTMLElement | null>(null)

watch(
  () => route.fullPath,
  () => {
    if (mainContent.value) {
      mainContent.value.scrollTop = 0
    }
  },
  { flush: 'post' },
)
</script>

<template>
  <div class="app-shell">
    <a class="skip-link" href="#main-content">Skip to content</a>
    <TopNavigation />

    <main id="main-content" ref="mainContent" class="main-content" tabindex="-1">
      <div v-if="appStore.bridgeState === 'loading'" class="bridge-notice" role="status">
        <LoaderCircle :size="15" class="spin" aria-hidden="true" />
        Connecting to the desktop app
      </div>
      <div v-else-if="appStore.bridgeState === 'browser'" class="bridge-notice" role="status">
        <MonitorDown :size="15" aria-hidden="true" />
        Browser preview · desktop services are unavailable
      </div>
      <div v-else-if="appStore.bridgeState === 'ready'" class="bridge-notice bridge-notice--ready" role="status">
        <Check :size="15" aria-hidden="true" />
        Local desktop · {{ appStore.appInfo?.version }}
      </div>
      <div v-else-if="appStore.bridgeState === 'error'" class="bridge-notice bridge-notice--error" role="alert">
        <AlertCircle :size="15" aria-hidden="true" />
        <span>{{ appStore.bridgeError }}</span>
        <button type="button" class="text-button" @click="appStore.retry">Retry</button>
      </div>

      <RouterView />
    </main>

    <PersistentPlayer />
  </div>
</template>