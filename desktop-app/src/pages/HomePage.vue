<script setup lang="ts">
import { AlertCircle, ArrowRight, FileAudio, Headphones, LibraryBig, ListMusic, LoaderCircle, Pause, Play } from '@lucide/vue'
import { computed, watch } from 'vue'
import { RouterLink } from 'vue-router'
import BrandMark from '../components/BrandMark.vue'
import { useAppStore } from '../stores/app'
import { useLibraryStore } from '../stores/library'
import { toPlayerTrack } from '../services/audioPlayback'
import { usePlayerStore } from '../stores/player'
import PageState from './PageState.vue'

const appStore = useAppStore()
const library = useLibraryStore()
const player = usePlayerStore()
const recentTracks = computed(() => library.items.filter((item) => item.kind === 'track').slice(0, 4))

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready') {
      void Promise.all([library.loadPage('all'), library.loadPlaylists()])
    } else if (state === 'browser') {
      library.markUnavailable()
      library.playlistStatus = 'unavailable'
    } else if (state === 'error') {
      library.status = 'error'
      library.error = appStore.bridgeError
      library.playlistStatus = 'error'
      library.playlistError = appStore.bridgeError
    }
  },
  { immediate: true },
)

function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return '—'
  return `${Math.floor(seconds / 60)}:${String(Math.floor(seconds % 60)).padStart(2, '0')}`
}

function playRecent(trackId: number): void {
  const tracks = recentTracks.value
    .filter((track) => track.availability === 'available')
    .map(toPlayerTrack)
  const index = tracks.findIndex((track) => track.id === trackId)
  if (index >= 0) void player.playQueue(tracks, index)
}
</script>

<template>
  <div class="page home-page">
    <header class="page-heading">
      <div>
        <h1>Listen</h1>
        <p>A quiet place for the music you keep close.</p>
      </div>
      <RouterLink class="quiet-link" :to="{ name: 'library' }">
        Open library <ArrowRight :size="16" aria-hidden="true" />
      </RouterLink>
    </header>

    <section class="listening-stage" aria-labelledby="listening-title">
      <div class="stage-artwork" aria-hidden="true">
        <BrandMark />
        <span>BUITENZORG</span>
      </div>
      <div class="stage-copy">
        <Headphones class="stage-icon" :size="20" :stroke-width="1.7" aria-hidden="true" />
        <h2 id="listening-title">{{ player.currentTrack?.title ?? 'A little space for your music.' }}</h2>
        <p>{{ player.currentTrack ? `${player.currentTrack.artist} · ${player.currentTrack.album}` : 'Your local collection, ready when you are.' }}</p>
        <p v-if="player.error" class="player-error" role="alert">{{ player.error }}</p>
        <button v-if="player.currentTrack" class="primary-link" type="button" :disabled="player.isLoading" @click="player.togglePlayback">
          <Pause v-if="player.isPlaying" :size="17" aria-hidden="true" />
          <Play v-else :size="17" fill="currentColor" aria-hidden="true" />
          {{ player.isPlaying ? 'Pause' : 'Resume listening' }}
        </button>
        <RouterLink v-else class="primary-link" :to="{ name: 'library' }">
          <LibraryBig :size="17" aria-hidden="true" />
          Browse local library
        </RouterLink>
        <span class="stage-note">One persistent local audio player · codec support depends on this system.</span>
      </div>
    </section>

    <div class="home-shelves">
      <section class="home-shelf" aria-labelledby="recent-title">
        <div class="shelf-heading">
          <h2 id="recent-title">Recently added</h2>
          <RouterLink :to="{ name: 'library' }">Library</RouterLink>
        </div>
        <PageState v-if="appStore.bridgeState === 'loading' || appStore.bridgeState === 'idle' || library.status === 'loading'" :icon="LoaderCircle" title="Loading recent additions" description="Reading local track metadata." state="loading" />
        <PageState v-else-if="library.status === 'unavailable'" :icon="LibraryBig" title="Open the desktop app to see local music" description="Browser preview does not have access to the local library." />
        <PageState v-else-if="library.status === 'error'" :icon="AlertCircle" title="Recent additions could not be loaded" :description="library.error || 'The local database did not respond.'" state="error" />
        <PageState v-else-if="library.status === 'empty'" :icon="LibraryBig" title="Your library is empty" description="Import local audio from Library to see real additions here." />
        <ol v-else class="home-track-list" aria-label="Recently added local tracks">
          <li v-for="track in recentTracks" :key="track.id">
            <button class="library-track-play" type="button" :aria-label="`Play ${track.title}`" :disabled="track.availability !== 'available'" @click="playRecent(track.id)"><Play :size="14" fill="currentColor" aria-hidden="true" /></button>
            <FileAudio :size="16" aria-hidden="true" />
            <span><strong>{{ track.title }}</strong><small>{{ track.subtitle }} · {{ track.detail || 'Local files' }}</small></span>
            <span class="home-track-list__meta">{{ formatDuration(track.durationSeconds) }}</span>
          </li>
        </ol>
      </section>

      <section class="home-shelf" aria-labelledby="playlist-title">
        <div class="shelf-heading">
          <h2 id="playlist-title">Playlists</h2>
          <RouterLink :to="{ name: 'playlists' }">View playlists</RouterLink>
        </div>
        <PageState v-if="appStore.bridgeState === 'loading' || appStore.bridgeState === 'idle' || library.playlistStatus === 'loading'" :icon="LoaderCircle" title="Loading playlists" description="Reading locally saved listening sets." state="loading" />
        <PageState v-else-if="library.playlistStatus === 'unavailable'" :icon="ListMusic" title="Playlists are available in the desktop app" description="Browser preview cannot read the local database." />
        <PageState v-else-if="library.playlistStatus === 'error'" :icon="AlertCircle" title="Playlists could not be loaded" :description="library.playlistError || 'The local database did not respond.'" state="error" />
        <PageState v-else-if="library.playlists.length === 0" :icon="ListMusic" title="No playlists yet" description="Create a listening set in Playlists to see it here." />
        <div v-else class="home-playlist-list">
          <RouterLink v-for="playlist in library.playlists.slice(0, 4)" :key="playlist.id" :to="{ name: 'playlists' }" class="home-playlist-list__item" @click="library.selectPlaylist(playlist.id)">
            <ListMusic :size="17" aria-hidden="true" />
            <span><strong>{{ playlist.name }}</strong><small>{{ playlist.trackCount }} {{ playlist.trackCount === 1 ? 'track' : 'tracks' }}</small></span>
            <ArrowRight :size="15" aria-hidden="true" />
          </RouterLink>
        </div>
      </section>
    </div>
  </div>
</template>