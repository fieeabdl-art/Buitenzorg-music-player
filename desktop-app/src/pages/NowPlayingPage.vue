<script setup lang="ts">
import { computed } from 'vue'
import { ListMusic, LoaderCircle, Pause, Play, SkipBack, SkipForward, Square } from '@lucide/vue'
import { usePlayerStore } from '../stores/player'
import TrackArtwork from '../components/TrackArtwork.vue'
import PageState from './PageState.vue'
import { trackMetadataSummary } from '../services/trackMetadata'

const player = usePlayerStore()
const track = computed(() => player.currentTrack)

function formatTime(seconds: number): string {
  const whole = Math.max(0, Math.floor(Number.isFinite(seconds) ? seconds : 0))
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`
}

function seek(event: Event): void {
  player.seek(Number((event.target as HTMLInputElement).value))
}
</script>

<template>
  <div class="page now-playing-page">
    <header class="page-heading">
      <div>
        <h1>Now playing</h1>
        <p>Playback stays in the persistent player while you move around the app.</p>
      </div>
      <button class="secondary-button" type="button" :aria-pressed="player.queueOpen" @click="player.toggleQueue">
        <ListMusic :size="15" aria-hidden="true" />{{ player.queueOpen ? 'Close queue' : `Queue · ${player.queue.length}` }}
      </button>
    </header>

    <section v-if="track" class="now-playing-stage" aria-label="Current track">
      <TrackArtwork size="hero" :artwork-url="track.artworkUrl" />
      <div class="now-playing-stage__copy">
        <span class="now-playing-source">{{ player.isYouTubePlayback ? 'YouTube video · Online' : 'Local library' }}</span>
        <h2>{{ track.title }}</h2>
        <p>{{ track.artist }} · {{ track.album }}</p>
        <p v-if="trackMetadataSummary(track)" class="now-playing-metadata">{{ trackMetadataSummary(track) }}</p>
        <p v-if="player.error" class="player-error" role="alert">{{ player.error }}</p>
        <p v-else-if="player.playbackNotice" class="player-notice" role="status">{{ player.playbackNotice }}</p>
        <div class="now-playing-controls">
          <button type="button" aria-label="Previous track" :disabled="player.queue.length === 0" @click="player.previous"><SkipBack :size="18" aria-hidden="true" /></button>
          <button class="now-playing-play" type="button" :aria-label="player.isPlaying || player.isLoading ? 'Pause' : 'Play'" @click="player.togglePlayback">
            <LoaderCircle v-if="player.isLoading" :size="18" class="spin" aria-hidden="true" />
            <Pause v-if="player.isPlaying" :size="18" fill="currentColor" aria-hidden="true" />
            <Play v-else-if="!player.isLoading" :size="18" fill="currentColor" aria-hidden="true" />
          </button>
          <button type="button" aria-label="Stop playback" @click="player.stop"><Square :size="16" fill="currentColor" aria-hidden="true" /></button>
          <button type="button" aria-label="Next track" :disabled="player.queue.length === 0" @click="player.next"><SkipForward :size="18" aria-hidden="true" /></button>
        </div>
        <div class="now-playing-progress">
          <span>{{ formatTime(player.currentTime) }}</span>
          <input type="range" min="0" :max="player.duration || 0" step="0.1" :value="player.currentTime" :disabled="player.duration <= 0" :aria-label="`Seek ${track.title}`" @input="seek" />
          <span>{{ formatTime(player.duration) }}</span>
        </div>
      </div>
    </section>

    <section v-else class="content-region">
      <PageState :icon="Play" title="Nothing is playing" description="Choose a local track or a YouTube search result to start playback." action-label="Search music" action-to="/search" />
    </section>
  </div>
</template>