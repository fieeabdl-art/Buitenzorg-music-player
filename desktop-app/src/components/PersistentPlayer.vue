<script setup lang="ts">
import { ListMusic, LoaderCircle, Pause, Play, Repeat, Repeat1, Shuffle, SkipBack, SkipForward, Square, Volume2, VolumeX, X } from '@lucide/vue'
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import TrackArtwork from './TrackArtwork.vue'
import YouTubePlayerFrame from './YouTubePlayerFrame.vue'
import { usePlayerStore } from '../stores/player'
import { trackMetadataSummary } from '../services/trackMetadata'

const player = usePlayerStore()
const canControl = computed(() => player.currentTrack !== null)

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '0:00'
  const wholeSeconds = Math.floor(seconds)
  return `${Math.floor(wholeSeconds / 60)}:${String(wholeSeconds % 60).padStart(2, '0')}`
}

function seek(event: Event) {
  player.seek(Number((event.target as HTMLInputElement).value))
}

function changeVolume(event: Event) {
  player.setVolume(Number((event.target as HTMLInputElement).value))
}
</script>

<template>
  <footer class="persistent-player" :class="{ 'persistent-player--youtube': player.isYouTubePlayback }" aria-label="Music player">
    <section v-if="player.queueOpen" class="player-queue" aria-label="Playback queue">
      <header><h2>Up next</h2><button type="button" aria-label="Close queue" @click="player.toggleQueue"><X :size="16" aria-hidden="true" /></button></header>
      <p v-if="player.queue.length === 0" class="player-queue__empty">The queue is empty. Play something from your library.</p>
      <ol v-else>
        <li v-for="(track, index) in player.queue" :key="`${track.id}-${index}`" :class="{ 'player-queue__item--current': index === player.queueIndex }">
          <button type="button" :aria-current="index === player.queueIndex ? 'true' : undefined" @click="player.playQueue(player.queue, index)">
            <span><strong>{{ track.title }}</strong><small>{{ track.artist }} · {{ track.album }}</small><small v-if="trackMetadataSummary(track)">{{ trackMetadataSummary(track) }}</small></span>
            <span>{{ index === player.queueIndex ? (player.isPlaying ? 'Playing' : 'Current') : '' }}</span>
          </button>
        </li>
      </ol>
    </section>

    <YouTubePlayerFrame
      v-if="player.currentYouTubeTrack"
      :key="player.youtubePlayerRevision"
      :video-id="player.currentYouTubeTrack.videoId"
      :title="player.currentYouTubeTrack.title"
    />

    <div class="player-track">
      <TrackArtwork size="player" :artwork-url="player.currentTrack?.artworkUrl" />
      <RouterLink v-if="player.currentTrack" class="player-track__copy" :to="{ name: 'now-playing' }">
        <strong>{{ player.currentTrack?.title ?? 'No track selected' }}</strong>
        <span>{{ player.currentTrack ? `${player.currentTrack.artist} · ${player.currentTrack.album}` : 'Choose a track from your local library' }}</span>
        <span v-if="player.currentTrack && trackMetadataSummary(player.currentTrack)">{{ trackMetadataSummary(player.currentTrack) }}</span>
        <span v-if="player.isYouTubePlayback">YouTube video · Internet required</span>
        <span v-if="player.error" class="player-error" role="alert">{{ player.error }}</span>
        <span v-else-if="player.playbackNotice" role="status">{{ player.playbackNotice }}</span>
        <span v-else-if="player.historyWarning" class="player-error" role="status">{{ player.historyWarning }}</span>
      </RouterLink>
      <div v-else class="player-track__copy">
        <strong>No track selected</strong>
        <span>Choose a track from your local library</span>
      </div>
    </div>

    <div class="player-controls">
      <div class="player-controls__buttons">
        <button type="button" :aria-pressed="player.shuffle" aria-label="Shuffle" :title="player.shuffle ? 'Turn shuffle off' : 'Turn shuffle on'" :disabled="player.queue.length < 2" @click="player.toggleShuffle"><Shuffle :size="16" aria-hidden="true" /></button>
        <button type="button" aria-label="Previous track" title="Previous track" :disabled="player.queue.length === 0" @click="player.previous"><SkipBack :size="17" aria-hidden="true" /></button>
      <button class="player-play" type="button" :aria-label="player.isPlaying || player.isLoading ? 'Pause' : 'Play'" :title="player.isLoading ? 'Loading playback' : player.isPlaying ? 'Pause' : 'Play'" :disabled="!canControl" @click="player.togglePlayback">
          <LoaderCircle v-if="player.isLoading" :size="17" class="spin" aria-hidden="true" />
          <Pause v-else-if="player.isPlaying" :size="17" fill="currentColor" aria-hidden="true" />
          <Play v-else :size="17" fill="currentColor" aria-hidden="true" />
        </button>
        <button type="button" aria-label="Stop playback" title="Stop playback" :disabled="!player.currentTrack" @click="player.stop"><Square :size="15" fill="currentColor" aria-hidden="true" /></button>
        <button type="button" aria-label="Next track" title="Next track" :disabled="player.queue.length === 0" @click="player.next"><SkipForward :size="17" aria-hidden="true" /></button>
        <button type="button" :aria-label="`Repeat ${player.repeatMode}`" :aria-pressed="player.repeatMode !== 'off'" :title="`Repeat: ${player.repeatMode}`" :disabled="player.queue.length === 0" @click="player.cycleRepeat">
          <Repeat1 v-if="player.repeatMode === 'one'" :size="16" aria-hidden="true" />
          <Repeat v-else :size="16" aria-hidden="true" />
        </button>
      </div>
      <div class="player-progress">
        <span>{{ formatTime(player.currentTime) }}</span>
        <input
          class="player-seek"
          type="range"
          min="0"
          :max="player.duration || 0"
          step="0.1"
          :value="player.currentTime"
          :disabled="!player.currentTrack || player.duration <= 0"
          :aria-label="player.currentTrack ? `Seek ${player.currentTrack.title}` : 'Playback position'"
          :aria-valuetext="`${formatTime(player.currentTime)} of ${formatTime(player.duration)}`"
          @input="seek"
        />
        <span>{{ formatTime(player.duration) }}</span>
      </div>
    </div>

    <div class="player-tools">
      <button type="button" aria-label="Queue" title="Playback queue" :aria-expanded="player.queueOpen" @click="player.toggleQueue"><ListMusic :size="17" aria-hidden="true" /></button>
      <button type="button" :aria-label="player.isMuted ? 'Unmute' : 'Mute'" :title="player.isMuted ? 'Unmute' : 'Mute'" :disabled="!player.currentTrack" @click="player.toggleMute">
        <VolumeX v-if="player.isMuted" :size="17" :stroke-width="1.8" aria-hidden="true" />
        <Volume2 v-else :size="17" :stroke-width="1.8" aria-hidden="true" />
      </button>
      <input class="player-volume" type="range" min="0" max="1" step="0.01" :value="player.volume" :disabled="!player.currentTrack" aria-label="Volume" @input="changeVolume" />
    </div>
  </footer>
</template>