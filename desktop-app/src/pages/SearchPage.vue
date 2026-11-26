<script setup lang="ts">
import { AlertCircle, FileAudio, LoaderCircle, Play, Search } from '@lucide/vue'
import { ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useLibraryStore } from '../stores/library'
import { toPlayerTrack } from '../services/audioPlayback'
import { usePlayerStore } from '../stores/player'
import type { LibraryItem } from '../types/library'
import PageState from './PageState.vue'

const route = useRoute()
const router = useRouter()
const appStore = useAppStore()
const library = useLibraryStore()
const player = usePlayerStore()
const query = ref(typeof route.query.q === 'string' ? route.query.q : '')
const submittedQuery = ref(query.value)

watch(
  () => route.query.q,
  (value) => {
    query.value = typeof value === 'string' ? value : ''
    submittedQuery.value = query.value
    if (appStore.bridgeState === 'ready') {
      void library.searchLocal(submittedQuery.value)
    }
  },
  { immediate: true },
)

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready' && submittedQuery.value) {
      void library.searchLocal(submittedQuery.value)
    } else if (state === 'browser' && submittedQuery.value) {
      void library.searchLocal(submittedQuery.value)
    } else if (state === 'error') {
      library.searchStatus = 'error'
      library.searchError = appStore.bridgeError
    }
  },
)

function search() {
  submittedQuery.value = query.value.trim()
  const routeQuery = typeof route.query.q === 'string' ? route.query.q : ''
  if (routeQuery === submittedQuery.value) {
    void library.searchLocal(submittedQuery.value)
  } else {
    void router.replace({ name: 'search', query: submittedQuery.value ? { q: submittedQuery.value } : {} })
  }
}

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes <= 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${value.toFixed(unit === 0 || value >= 10 ? 0 : 1)} ${units[unit]}`
}

function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '—'
  return `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`
}

function availabilityLabel(item: LibraryItem): string {
  if (item.availability === 'missing') return 'File missing'
  if (item.availability === 'unreadable') return 'Unavailable'
  return 'On device'
}

function playItem(item: LibraryItem): void {
  if (item.availability !== 'available') {
    player.error = 'This local audio file is missing or unavailable.'
    return
  }
  const tracks = library.searchResults
    .filter((result) => result.availability === 'available')
    .map(toPlayerTrack)
  const index = tracks.findIndex((track) => track.id === item.id)
  if (index >= 0) void player.playQueue(tracks, index)
}
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Search</h1>
        <p>Search audio already stored in your local library. Official provider search is not enabled yet.</p>
      </div>
    </header>

    <form class="search-panel" role="search" @submit.prevent="search">
      <Search :size="18" :stroke-width="1.8" aria-hidden="true" />
      <label class="sr-only" for="page-search">Search music</label>
      <input id="page-search" v-model="query" type="search" maxlength="120" placeholder="Track, artist, or album" />
      <button class="primary-button" type="submit">Search</button>
    </form>

    <p v-if="submittedQuery && library.searchStatus === 'ready'" class="search-summary" aria-live="polite">{{ library.searchTotalCount }} local result{{ library.searchTotalCount === 1 ? '' : 's' }} for “{{ submittedQuery }}”</p>

    <section v-if="appStore.bridgeState === 'loading' || appStore.bridgeState === 'idle' || library.searchStatus === 'loading'" class="content-region">
      <PageState
        :icon="LoaderCircle"
        title="Searching your local library"
        description="Matching imported track metadata on this device."
        state="loading"
      />
    </section>
    <section v-else-if="appStore.bridgeState === 'browser' || library.searchStatus === 'unavailable'" class="content-region">
      <PageState :icon="Search" title="Open the desktop app to search local audio" description="Browser preview cannot read the SQLite library or local files." />
    </section>
    <section v-else-if="library.searchStatus === 'error'" class="content-region">
      <PageState :icon="AlertCircle" title="Local search failed" :description="library.searchError || 'The local database did not respond.'" state="error" />
    </section>
    <section v-else-if="submittedQuery && library.searchStatus === 'empty'" class="content-region">
      <PageState :icon="Search" title="No local matches" description="No imported track matched this title, artist, album, or source filename." action-label="Browse your library" action-to="/library" />
    </section>
    <section v-else-if="!submittedQuery" class="content-region">
      <PageState :icon="Search" title="Search your local library" description="Enter a track, artist, or album name. Only real local entries will be shown." />
    </section>
    <section v-else class="library-table-wrap" aria-label="Local search results">
      <button v-if="library.searchResults.some((item) => item.availability === 'available')" class="secondary-button library-play-loaded" type="button" @click="player.playQueue(library.searchResults.filter((item) => item.availability === 'available').map(toPlayerTrack))"><Play :size="15" fill="currentColor" aria-hidden="true" />Play results</button>
      <table class="library-table">
        <thead><tr><th scope="col">Track</th><th scope="col">Album</th><th scope="col">Format</th><th scope="col">Duration</th><th scope="col">Size</th><th scope="col">Availability</th></tr></thead>
        <tbody>
          <tr v-for="item in library.searchResults" :key="item.id">
            <th scope="row"><span class="library-track-title"><button class="library-track-play" type="button" :aria-label="`Play ${item.title}`" :disabled="item.availability !== 'available'" @click="playItem(item)"><Play :size="14" fill="currentColor" aria-hidden="true" /></button><FileAudio :size="16" aria-hidden="true" /><span>{{ item.title }}<small>{{ item.subtitle }}</small></span></span></th>
            <td>{{ item.detail || '—' }}</td>
            <td>{{ item.format?.toUpperCase() || '—' }}</td>
            <td>{{ formatDuration(item.durationSeconds) }}</td>
            <td>{{ formatBytes(item.sizeBytes) }}</td>
            <td><span class="availability-badge" :class="`availability-badge--${item.availability || 'unavailable'}`">{{ availabilityLabel(item) }}</span></td>
          </tr>
        </tbody>
      </table>
      <button v-if="library.searchNextOffset !== null" class="secondary-button library-load-more" type="button" @click="library.searchMore">Load more</button>
    </section>
  </div>
</template>