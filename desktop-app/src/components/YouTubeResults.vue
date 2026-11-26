<script setup lang="ts">
import { Play } from '@lucide/vue'
import TrackArtwork from './TrackArtwork.vue'
import type { YouTubeVideo } from '../types/providers'

defineProps<{
  videos: YouTubeVideo[]
}>()

const emit = defineEmits<{
  play: [video: YouTubeVideo]
}>()

function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds)) return 'Duration unavailable'
  const whole = Math.max(0, Math.floor(seconds))
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`
}
</script>

<template>
  <ol class="youtube-results" aria-label="YouTube video results">
    <li v-for="video in videos" :key="video.videoId" class="youtube-result">
      <TrackArtwork :artwork-url="video.thumbnailUrl" />
      <div class="youtube-result__copy">
        <strong>{{ video.title }}</strong>
        <span>{{ video.artist || 'YouTube channel' }}</span>
        <small>{{ formatDuration(video.durationSeconds) }}</small>
      </div>
      <button class="secondary-button" type="button" :aria-label="`Play ${video.title} on YouTube`" @click="emit('play', video)">
        <Play :size="14" fill="currentColor" aria-hidden="true" />Play
      </button>
    </li>
  </ol>
</template>