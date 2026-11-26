<script setup lang="ts">
import { AlertCircle, Clock3, LoaderCircle, Play, RefreshCw, Trash2 } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { useAppStore } from '../stores/app'
import { useHistoryStore } from '../stores/history'
import { usePlayerStore } from '../stores/player'
import TrackArtwork from '../components/TrackArtwork.vue'
import PageState from './PageState.vue'
import type { PlaybackHistoryEntry, PlaybackHistoryStatus } from '../types/history'

const app = useAppStore()
const history = useHistoryStore()
const player = usePlayerStore()
const confirmClear = ref(false)
const hasClearableEntries = computed(() => history.entries.some((entry) => !['loading', 'playing', 'paused'].includes(entry.status)))

watch(
  () => app.bridgeState,
  (state) => {
    if (state === 'ready') void history.load()
    else if (state === 'browser') history.markUnavailable()
    else if (state === 'error') history.status = 'error'
  },
  { immediate: true },
)

watch(() => [player.currentTrack?.id, player.isPlaying, player.historyRevision], () => {
  if (app.bridgeState === 'ready') void history.load()
})

function formatTime(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds))
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`
}

function formatPlayedAt(value: string): string {
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(date)
}

function statusLabel(status: PlaybackHistoryStatus): string {
  const labels: Record<PlaybackHistoryStatus, string> = {
    loading: 'Loading',
    playing: 'Playing',
    paused: 'Paused',
    completed: 'Completed',
    stopped: 'Stopped',
    failed: 'Failed',
    interrupted: 'Interrupted',
  }
  return labels[status]
}

function play(entry: PlaybackHistoryEntry): void {
  if (entry.track.availability === 'available') void player.playQueue([entry.track])
}

function requestClear(): void {
  if (!confirmClear.value) {
    confirmClear.value = true
    return
  }
  confirmClear.value = false
  void history.clearEntries()
}

function cancelClear(): void {
  confirmClear.value = false
}
</script>

<template>
  <div class="page history-page">
    <header class="page-heading">
      <div>
        <h1>Listening history</h1>
        <p>Recent playback sessions stored on this device. The list keeps up to 100 entries.</p>
      </div>
      <div class="history-heading-actions">
        <button class="secondary-button" type="button" :disabled="history.status === 'loading'" @click="history.load"><RefreshCw :size="15" aria-hidden="true" />Refresh</button>
        <button v-if="hasClearableEntries" class="secondary-button" type="button" @click="requestClear"><Trash2 :size="15" aria-hidden="true" />Clear history</button>
      </div>
    </header>

    <div v-if="confirmClear" class="history-confirm" role="group" aria-label="Confirm clear history">
      <span>Remove finished, stopped, failed, and interrupted sessions? Active playback will remain.</span>
      <button class="quiet-button quiet-button--danger" type="button" @click="requestClear">Remove history</button>
      <button class="quiet-button" type="button" @click="cancelClear">Cancel</button>
    </div>

    <section v-if="app.bridgeState === 'loading' || history.status === 'loading' || history.status === 'idle'" class="content-region">
      <PageState :icon="LoaderCircle" title="Loading playback history" description="Reading recent sessions from your local library." state="loading" />
    </section>
    <section v-else-if="app.bridgeState === 'browser' || history.status === 'unavailable'" class="content-region">
      <PageState :icon="Clock3" title="Open the desktop app to view history" description="Playback history is stored in the local SQLite library and is not available in a browser preview." />
    </section>
    <section v-else-if="app.bridgeState === 'error' || history.status === 'error'" class="content-region">
      <PageState :icon="AlertCircle" title="Playback history could not be loaded" :description="history.error || app.bridgeError || 'The local database did not respond.'" state="error" />
    </section>
    <section v-else-if="history.status === 'empty'" class="content-region">
      <p v-if="history.feedback" class="history-feedback" role="status">{{ history.feedback }}</p>
      <PageState :icon="Clock3" title="No playback history yet" description="Tracks you start playing will appear here with their session time and status." action-label="Browse your library" action-to="/library" />
    </section>
    <section v-else class="history-content" aria-label="Recent playback sessions">
      <p v-if="history.feedback" class="history-feedback" role="status">{{ history.feedback }}</p>
      <p v-if="history.error" class="library-error" role="alert"><AlertCircle :size="16" aria-hidden="true" />{{ history.error }}</p>
      <ol class="history-list">
        <li v-for="entry in history.entries" :key="entry.historyId" class="history-row">
          <TrackArtwork :artwork-url="entry.track.artworkUrl" />
          <span class="history-row__copy">
            <strong>{{ entry.track.title }}</strong>
            <small>{{ entry.track.artist }} · {{ entry.track.album }}</small>
            <small>{{ formatPlayedAt(entry.playedAt) }} · Listened {{ formatTime(entry.listenedSeconds) }}<template v-if="entry.track.durationSeconds !== null"> / {{ formatTime(entry.track.durationSeconds) }} total</template></small>
          </span>
          <span class="history-status" :class="`history-status--${entry.status}`">{{ statusLabel(entry.status) }}</span>
          <span class="history-row__actions">
            <button class="secondary-button" type="button" :disabled="entry.track.availability !== 'available'" :aria-label="`Play ${entry.track.title}`" @click="play(entry)"><Play :size="14" fill="currentColor" aria-hidden="true" />Play</button>
            <button class="quiet-button quiet-button--danger" type="button" :disabled="['loading', 'playing', 'paused'].includes(entry.status)" :aria-label="`Remove ${entry.track.title} from history`" :title="['loading', 'playing', 'paused'].includes(entry.status) ? 'Stop the active session before removing it' : 'Remove from history'" @click="history.removeEntry(entry.historyId)"><Trash2 :size="15" aria-hidden="true" />Remove</button>
          </span>
        </li>
      </ol>
    </section>
  </div>
</template>