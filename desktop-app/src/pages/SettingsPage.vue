<script setup lang="ts">
import { Compass, Download, Info, LibraryBig, Settings2, UserRound } from '@lucide/vue'
import { computed, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useLibraryStore } from '../stores/library'
import { useProvidersStore } from '../stores/providers'

const appStore = useAppStore()
const library = useLibraryStore()
const providers = useProvidersStore()
const libraryValue = computed(() => {
  if (appStore.bridgeState === 'browser') return 'Desktop app only'
  if (appStore.bridgeState === 'error') return 'Unavailable'
  if (library.storageInfo) return `${library.storageInfo.trackCount} tracks`
  if (library.storageError) return 'Unavailable'
  return appStore.bridgeState === 'ready' ? 'Loading' : 'Not connected'
})
const storageValue = computed(() => {
  if (appStore.bridgeState === 'browser') return 'Desktop app only'
  if (appStore.bridgeState === 'error') return 'Unavailable'
  if (!library.storageInfo) return library.storageError ? 'Unavailable' : 'Loading'
  return `${formatBytes(library.storageInfo.storageBytes)} · ${library.storageInfo.trackCount} tracks`
})
const musicSourcesValue = computed(() => {
  if (appStore.bridgeState === 'browser') return 'Desktop app only'
  if (appStore.bridgeState === 'error') return 'Unavailable'
  if (!providers.youtubeStatus) return appStore.bridgeState === 'ready' ? 'Loading' : 'Not connected'
  return `YouTube · ${providers.youtubeStatus.state}`
})

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready') {
      void library.loadStorageInfo()
      void providers.loadYoutubeStatus()
    }
  },
  { immediate: true },
)

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes
  let unit = -1
  do {
    value /= 1024
    unit += 1
  } while (value >= 1024 && unit < units.length - 1)
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`
}
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Settings</h1>
        <p>Preferences and connections for this device.</p>
      </div>
    </header>

    <section class="settings-list" aria-label="Settings sections">
      <RouterLink class="settings-row" :to="{ name: 'profile' }">
        <UserRound :size="19" aria-hidden="true" />
        <span><strong>Local profile</strong><small>Display name and optional details. No online account is required.</small></span>
        <span class="settings-value">Not created</span>
      </RouterLink>
      <RouterLink class="settings-row" :to="{ name: 'library' }">
        <LibraryBig :size="19" aria-hidden="true" />
        <span><strong>Music library</strong><small>Audio is copied locally; original files stay untouched.</small></span>
        <span class="settings-value">{{ libraryValue }}</span>
      </RouterLink>
      <RouterLink class="settings-row" :to="{ name: 'discover' }">
        <Compass :size="19" aria-hidden="true" />
        <span><strong>Music sources</strong><small>YouTube search uses an official API; no downloads are enabled.</small></span>
        <span class="settings-value">{{ musicSourcesValue }}</span>
      </RouterLink>
      <RouterLink class="settings-row" :to="{ name: 'downloads' }">
        <Download :size="19" aria-hidden="true" />
        <span><strong>Downloads</strong><small>Tasks require a source with explicit download permission.</small></span>
        <span class="settings-value">Unavailable</span>
      </RouterLink>
      <div class="settings-row settings-row--static">
        <Settings2 :size="19" aria-hidden="true" />
        <span><strong>Managed storage</strong><small>Audio and metadata remain in this device’s app-data folder.</small></span>
        <span class="settings-value">{{ storageValue }}</span>
      </div>
      <RouterLink class="settings-row" :to="{ name: 'about' }">
        <Info :size="19" aria-hidden="true" />
        <span><strong>About Buitenzorg Player</strong><small>Project identity and philosophy.</small></span>
        <span class="settings-value">About</span>
      </RouterLink>
    </section>

    <p class="settings-footnote">Local library metadata, audio copies, and listening history stay on this device. YouTube searches send the search terms to YouTube; no local listening data is sent.</p>
  </div>
</template>