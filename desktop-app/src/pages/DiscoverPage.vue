<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../stores/app'
import { useProvidersStore } from '../stores/providers'

const appStore = useAppStore()
const providers = useProvidersStore()
const apiKey = ref('')
const feedback = ref('')
const desktopReady = computed(() => appStore.bridgeState === 'ready')

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready' || state === 'browser') void providers.loadYoutubeStatus()
  },
  { immediate: true },
)

async function saveApiKey(): Promise<void> {
  feedback.value = ''
  const saved = await providers.saveYoutubeApiKey(apiKey.value)
  if (saved) {
    apiKey.value = ''
    feedback.value = 'API key saved in the operating system credential store.'
  }
}

async function checkConnection(): Promise<void> {
  feedback.value = ''
  const connected = await providers.checkYoutubeConnection()
  if (connected) feedback.value = 'YouTube search connection verified.'
}

async function removeApiKey(): Promise<void> {
  feedback.value = ''
  if (await providers.removeYoutubeApiKey()) feedback.value = 'YouTube API key removed.'
}
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Discover</h1>
        <p>Connect an official source to search beyond your local library.</p>
      </div>
    </header>

    <section class="provider-section" aria-labelledby="youtube-provider-title">
      <div class="provider-section__top">
        <div>
          <h2 id="youtube-provider-title">YouTube search</h2>
          <p>Searches use the official YouTube Data API v3. Results open on YouTube; they are not local audio files.</p>
        </div>
        <span class="state-label" role="status" aria-live="polite">
          {{ providers.isLoadingStatus ? 'Checking status' : providers.youtubeStatus?.state ?? 'Not connected' }}
        </span>
      </div>

      <p class="provider-status-copy" aria-live="polite">{{ providers.youtubeStatus?.message || 'Checking the secure provider connection.' }}</p>

      <form class="provider-key-form" @submit.prevent="saveApiKey">
        <label for="youtube-api-key">YouTube Data API key</label>
        <input
          id="youtube-api-key"
          v-model="apiKey"
          class="provider-key-input"
          type="password"
          autocomplete="new-password"
          spellcheck="false"
          maxlength="256"
          required
          :disabled="!desktopReady || providers.isSavingKey"
          aria-describedby="youtube-key-help"
        />
        <p id="youtube-key-help" class="provider-help">
          The key is stored in your operating system’s credential store, not SQLite. Search terms are sent to YouTube; your local library and listening history are not.
        </p>
        <div class="provider-actions">
          <button class="primary-button" type="submit" :disabled="!desktopReady || providers.isSavingKey || !apiKey.trim()">
            {{ providers.isSavingKey ? 'Saving…' : 'Save API key' }}
          </button>
          <button
            v-if="providers.youtubeStatus?.hasApiKey"
            class="secondary-button"
            type="button"
            :disabled="!desktopReady || providers.isCheckingConnection || providers.isSavingKey"
            @click="checkConnection"
          >
            {{ providers.isCheckingConnection ? 'Checking…' : 'Verify connection' }}
          </button>
          <button
            v-if="providers.youtubeStatus?.hasApiKey"
            class="quiet-button"
            type="button"
            :disabled="!desktopReady || providers.isSavingKey || providers.isCheckingConnection"
            @click="removeApiKey"
          >
            Remove key
          </button>
        </div>
      </form>

      <p v-if="providers.actionError" class="provider-feedback provider-feedback--error" role="alert">{{ providers.actionError }}</p>
      <p v-else-if="feedback" class="provider-feedback" role="status" aria-live="polite">{{ feedback }}</p>
      <p v-if="appStore.bridgeState === 'browser'" class="provider-help">Key management and provider search are available in the desktop app only.</p>
    </section>
  </div>
</template>