<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { usePlayerStore } from '../stores/player'
import type { YouTubePlayerController } from '../types/player'
import { loadYouTubeIframeApi, type YouTubeIframePlayer } from '../services/youtubeIframeApi'

const props = defineProps<{
  videoId: string
  title: string
}>()

const player = usePlayerStore()
const playerMount = ref<HTMLDivElement | null>(null)
const errorMessage = ref('')
let youtubePlayer: YouTubeIframePlayer | null = null
let controller: YouTubePlayerController | null = null
let positionTimer: number | undefined

function stopPositionTimer(): void {
  if (positionTimer !== undefined) {
    window.clearInterval(positionTimer)
    positionTimer = undefined
  }
}

function makeController(instance: YouTubeIframePlayer): YouTubePlayerController {
  return {
    loadVideoById: (videoId) => instance.loadVideoById(videoId),
    playVideo: () => instance.playVideo(),
    pauseVideo: () => instance.pauseVideo(),
    stopVideo: () => instance.stopVideo(),
    seekTo: (seconds) => instance.seekTo(seconds, true),
    setVolume: (volume) => instance.setVolume(volume),
    setMuted: (muted) => muted ? instance.mute() : instance.unMute(),
    getCurrentTime: () => instance.getCurrentTime(),
    getDuration: () => instance.getDuration(),
    getPlayerState: () => instance.getPlayerState(),
    getVideoData: () => instance.getVideoData(),
    getIframe: () => instance.getIframe(),
    destroy: () => instance.destroy(),
  }
}

onMounted(async () => {
  try {
    const api = await loadYouTubeIframeApi()
    if (!playerMount.value) return
    youtubePlayer = new api.Player(playerMount.value, {
      width: 240,
      height: 200,
      videoId: props.videoId,
      playerVars: {
        autoplay: 0,
        controls: 1,
        enablejsapi: 1,
        fs: 1,
        origin: window.location.origin,
        playsinline: 1,
        rel: 0,
      },
      events: {
        onReady: (event) => {
          controller = makeController(event.target)
          const iframe = controller.getIframe()
          if (iframe) {
            iframe.title = `YouTube player: ${props.title}`
            iframe.referrerPolicy = 'strict-origin-when-cross-origin'
          }
          player.handleYouTubeReady(controller)
          errorMessage.value = ''
        },
        onStateChange: (event) => {
          if (event.data === 1) {
            stopPositionTimer()
            positionTimer = window.setInterval(player.updateYouTubeTime, 500)
          } else {
            stopPositionTimer()
            player.updateYouTubeTime()
          }
          player.handleYouTubeStateChange(event.data)
        },
        onError: (event) => player.handleYouTubeError(event.data),
        onAutoplayBlocked: () => player.handleYouTubeAutoplayBlocked(),
      },
    })
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : 'The YouTube player could not be loaded.'
    player.handleYouTubeLoadFailure()
  }
})

onUnmounted(() => {
  stopPositionTimer()
  if (controller) {
    player.detachYouTubeController(controller)
    controller.destroy()
  } else {
    youtubePlayer?.destroy()
  }
})
</script>

<template>
  <section class="youtube-player-frame" aria-label="YouTube video player">
    <div ref="playerMount" class="youtube-player-frame__mount" />
    <p v-if="errorMessage" class="youtube-player-frame__error" role="alert">{{ errorMessage }}</p>
  </section>
</template>