<script setup lang="ts">
import { AlertCircle, LoaderCircle, Search } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../stores/app'
import { useProvidersStore } from '../stores/providers'
import { usePlayerStore } from '../stores/player'
import { toYouTubePlayerTrack } from '../services/audioPlayback'
import YouTubeResults from '../components/YouTubeResults.vue'
import PageState from './PageState.vue'
import type { YouTubeVideo } from '../types/providers'

const appStore = useAppStore()
const providers = useProvidersStore()
const apiKey = ref('')
const query = ref('')
const feedback = ref('')
const desktopReady = computed(() => appStore.bridgeState === 'ready')
const player = usePlayerStore()

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

function searchYoutube(): void {
  if (query.value.trim()) void providers.searchYoutube(query.value)
}

function playYoutubeResult(video: YouTubeVideo): void {
  const videos = providers.youtubeResults?.items ?? [video]
  const tracks = videos.map(toYouTubePlayerTrack)
  const index = tracks.findIndex((track) => track.videoId === video.videoId)
  if (index >= 0) void player.playQueue(tracks, index)
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
          <p>Searches use the official YouTube Data API v3. Results play in the official embedded player, separate from local audio files.</p>
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

    <section class="provider-section discover-search" aria-labelledby="youtube-search-title">
      <div class="provider-section__top">
        <div>
          <h2 id="youtube-search-title">Find a video</h2>
          <p>Search results play with YouTube’s official embedded player and remain separate from your local library.</p>
        </div>
      </div>
      <form class="search-panel" role="search" @submit.prevent="searchYoutube">
        <Search :size="18" :stroke-width="1.8" aria-hidden="true" />
        <label class="sr-only" for="discover-youtube-search">Search YouTube</label>
        <input id="discover-youtube-search" v-model="query" type="search" maxlength="120" placeholder="Track, artist, or video" />
        <button class="primary-button" type="submit" :disabled="!desktopReady || providers.isSearchingYoutube || !query.trim()">Search</button>
      </form>
      <section v-if="providers.isSearchingYoutube" class="content-region">
        <PageState :icon="LoaderCircle" title="Searching YouTube" description="Looking for matching videos through the official API." state="loading" />
      </section>
      <section v-else-if="providers.youtubeResults?.state === 'unavailable'" class="content-region">
        <PageState :icon="Search" title="Connect YouTube search" :description="providers.youtubeResults.message || 'Add a YouTube Data API key above to search.'" />
      </section>
      <section v-else-if="providers.youtubeResults?.state === 'empty'" class="content-region">
        <PageState :icon="Search" title="No YouTube matches" :description="providers.youtubeResults.message || 'YouTube returned no matching videos.'" />
      </section>
      <section v-else-if="providers.youtubeResults && providers.youtubeResults.state !== 'ready'" class="content-region">
        <PageState :icon="AlertCircle" title="YouTube search could not finish" :description="providers.youtubeResults.message || 'Check the connection and provider settings, then try again.'" state="error" />
      </section>
      <YouTubeResults v-else-if="providers.youtubeResults?.state === 'ready'" :videos="providers.youtubeResults.items" @play="playYoutubeResult" />
      <p v-if="providers.actionError" class="provider-feedback provider-feedback--error" role="alert">{{ providers.actionError }}</p>
    </section>
  </div>
</template>