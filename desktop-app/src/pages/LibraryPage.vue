<script setup lang="ts">
import { computed, watch } from 'vue'
import { CircleAlert, FolderOpen, HardDrive, LibraryBig, LoaderCircle, Play, Upload } from '@lucide/vue'
import { useAppStore } from '../stores/app'
import { useLibraryStore } from '../stores/library'
import { toPlayerTrack } from '../services/audioPlayback'
import { usePlayerStore } from '../stores/player'
import TrackArtwork from '../components/TrackArtwork.vue'
import { trackMetadataSummary } from '../services/trackMetadata'
import type { LibraryCategory, LibraryItem } from '../types/library'
import PageState from './PageState.vue'

const categories = [
  { id: 'all', label: 'All music', emptyTitle: 'Your library is empty', emptyDescription: 'Import audio you own or have permission to store. The app copies it into managed storage and leaves the original in place.' },
  { id: 'artists', label: 'Artists', emptyTitle: 'No artists to show', emptyDescription: 'Artists are read from imported audio metadata and will appear here when available.' },
  { id: 'albums', label: 'Albums', emptyTitle: 'No albums to show', emptyDescription: 'Albums are read from imported audio metadata and will appear here when available.' },
  { id: 'liked', label: 'Liked songs', emptyTitle: 'No liked songs yet', emptyDescription: 'Liked songs will appear after that local-library action is implemented.' },
  { id: 'imported', label: 'Imported', emptyTitle: 'No imported music yet', emptyDescription: 'Audio copied into this device’s managed library will appear here.' },
] as const

const appStore = useAppStore()
const library = useLibraryStore()
const player = usePlayerStore()
const selectedCategory = computed<LibraryCategory>({
  get: () => library.category,
  set: (category) => void library.loadPage(category),
})
const activeCategory = computed(() => categories.find((category) => category.id === selectedCategory.value) ?? categories[0])
const isWorking = computed(() => library.operation !== null)
const isTrackCategory = computed(() => ['all', 'liked', 'imported'].includes(selectedCategory.value))

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready') {
      void library.loadLibrary()
    } else if (state === 'browser') {
      library.markUnavailable()
    } else if (state === 'error') {
      library.status = 'error'
      library.error = appStore.bridgeError
    }
  },
  { immediate: true },
)

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes <= 0) return '—'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
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
  const minutes = Math.floor(seconds / 60)
  return `${minutes}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`
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
  const tracks = library.items
    .filter((entry) => entry.kind === 'track' && entry.availability === 'available')
    .map(toPlayerTrack)
  const index = tracks.findIndex((track) => track.id === item.id)
  if (index >= 0) void player.playQueue(tracks, index)
}

function playLoadedTracks(): void {
  const tracks = library.items
    .filter((entry) => entry.kind === 'track' && entry.availability === 'available')
    .map(toPlayerTrack)
  if (tracks.length) void player.playQueue(tracks)
}
</script>

<template>
  <div class="page">
    <header class="page-heading library-page-heading">
      <div>
        <h1>Library</h1>
        <p>Music you add is copied to this device. Original files are never moved or deleted.</p>
      </div>
      <div class="library-actions" aria-label="Local library actions">
        <button class="primary-button" type="button" :disabled="appStore.bridgeState !== 'ready' || isWorking" @click="library.importAudioFiles">
          <Upload :size="16" aria-hidden="true" />
          {{ library.operation === 'importing' ? 'Importing…' : 'Add audio' }}
        </button>
        <button class="secondary-button" type="button" :disabled="appStore.bridgeState !== 'ready' || isWorking" @click="library.scanAudioFolder">
          <FolderOpen :size="16" aria-hidden="true" />
          {{ library.operation === 'scanning' ? 'Scanning…' : 'Scan folder' }}
        </button>
        <button class="quiet-button" type="button" :disabled="appStore.bridgeState !== 'ready' || isWorking" @click="library.migrateLegacyLibrary">
          {{ library.operation === 'migrating' ? 'Checking…' : 'Import previous library' }}
        </button>
      </div>
    </header>

    <div class="library-overview">
      <span class="state-label"><FolderOpen :size="15" aria-hidden="true" /> Managed local library</span>
      <span v-if="library.storageInfo" class="state-label"><HardDrive :size="15" aria-hidden="true" /> {{ library.storageInfo.trackCount }} tracks · {{ formatBytes(library.storageInfo.storageBytes) }}</span>
    </div>

    <div class="library-tabs" role="group" aria-label="Library categories">
      <button
        v-for="category in categories"
        :key="category.id"
        class="library-tab"
        type="button"
        :aria-pressed="selectedCategory === category.id"
        @click="selectedCategory = category.id"
      >
        {{ category.label }}
      </button>
    </div>

    <p v-if="library.operation" class="library-feedback" role="status">
      <LoaderCircle :size="15" class="spin" aria-hidden="true" />
      {{ library.operation === 'importing' ? 'Copying and verifying selected audio…' : library.operation === 'scanning' ? 'Scanning selected folder…' : library.operation === 'migrating' ? 'Reading the previous library without changing it…' : 'Saving playlist changes…' }}
    </p>
    <p v-else-if="library.feedback" class="library-feedback" role="status">{{ library.feedback }}</p>
    <p v-if="library.error" class="library-error" role="alert"><CircleAlert :size="16" aria-hidden="true" /> {{ library.error }}</p>
    <p v-if="library.storageError" class="library-error" role="status"><CircleAlert :size="16" aria-hidden="true" /> Storage details could not be loaded.</p>

    <details v-if="library.lastReport?.issues.length" class="library-issues">
      <summary>{{ library.lastReport.issues.length }} item{{ library.lastReport.issues.length === 1 ? '' : 's' }} could not be imported</summary>
      <ul>
        <li v-for="(issue, index) in library.lastReport.issues" :key="`${issue.item}-${index}`"><strong>{{ issue.item }}</strong><span>{{ issue.message }}</span></li>
      </ul>
    </details>

    <section v-if="library.status === 'loading' || appStore.bridgeState === 'loading' || appStore.bridgeState === 'idle'" class="content-region">
      <PageState :icon="LoaderCircle" title="Loading your library" description="Reading local metadata from this device." state="loading" />
    </section>
    <section v-else-if="appStore.bridgeState === 'browser' || library.status === 'unavailable'" class="content-region">
      <PageState :icon="LibraryBig" title="Open the desktop app to use your library" description="The browser preview cannot access local files or the SQLite library." />
    </section>
    <section v-else-if="library.status === 'error'" class="content-region">
      <PageState :icon="CircleAlert" title="The library could not be loaded" :description="library.error || 'The local database did not respond.'" state="error" />
    </section>
    <section v-else-if="library.status === 'empty'" class="content-region">
      <PageState
        :icon="LibraryBig"
        :title="activeCategory.emptyTitle"
        :description="activeCategory.emptyDescription"
      />
    </section>
    <section v-else-if="isTrackCategory" class="library-table-wrap" aria-label="Local audio tracks">
      <button v-if="library.items.some((item) => item.kind === 'track' && item.availability === 'available')" class="secondary-button library-play-loaded" type="button" @click="playLoadedTracks"><Play :size="15" fill="currentColor" aria-hidden="true" />Play loaded tracks</button>
      <table class="library-table">
        <thead><tr><th scope="col">Track</th><th scope="col">Album</th><th scope="col">Format</th><th scope="col">Duration</th><th scope="col">Size</th><th scope="col">Availability</th></tr></thead>
        <tbody>
          <tr v-for="item in library.items" :key="item.id">
            <th scope="row"><span class="library-track-title"><button class="library-track-play" type="button" :aria-label="`Play ${item.title}`" :disabled="item.availability !== 'available'" @click="playItem(item)"><Play :size="14" fill="currentColor" aria-hidden="true" /></button><TrackArtwork :artwork-url="item.artworkUrl" /><span>{{ item.title }}<small>{{ item.subtitle }}</small><small v-if="trackMetadataSummary(item)">{{ trackMetadataSummary(item) }}</small></span></span></th>
            <td>{{ item.detail || '—' }}</td>
            <td>{{ item.format?.toUpperCase() || '—' }}</td>
            <td>{{ formatDuration(item.durationSeconds) }}</td>
            <td>{{ formatBytes(item.sizeBytes) }}</td>
            <td><span class="availability-badge" :class="`availability-badge--${item.availability || 'unavailable'}`">{{ availabilityLabel(item) }}</span></td>
          </tr>
        </tbody>
      </table>
      <button v-if="library.nextOffset !== null" class="secondary-button library-load-more" type="button" @click="library.loadMore">Load more</button>
    </section>
    <section v-else class="library-collection" :aria-label="selectedCategory === 'artists' ? 'Artists' : 'Albums'">
      <article v-for="item in library.items" :key="item.id" class="library-collection__item">
        <TrackArtwork :artwork-url="item.artworkUrl" />
        <span class="library-collection__copy"><strong>{{ item.title }}</strong><small>{{ item.subtitle }}</small><small v-if="item.detail">{{ item.detail }}</small></span>
        <span v-if="item.hasArtwork" class="state-label">Artwork embedded</span>
      </article>
      <button v-if="library.nextOffset !== null" class="secondary-button library-load-more" type="button" @click="library.loadMore">Load more</button>
    </section>
  </div>
</template>