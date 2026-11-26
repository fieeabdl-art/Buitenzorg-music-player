<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import AppShell from './components/AppShell.vue'
import { useAppStore } from './stores/app'
import { usePlayerStore } from './stores/player'

const appStore = useAppStore()
const playerStore = usePlayerStore()
const audioElement = ref<HTMLAudioElement | null>(null)

onMounted(() => {
  if (audioElement.value) {
    playerStore.attachAudio(audioElement.value)
  }

  void appStore.initialize()
  void playerStore.restoreLastSession()
})

onUnmounted(() => {
  if (audioElement.value) {
    playerStore.detachAudio(audioElement.value)
  }
  appStore.dispose()
})
</script>

<template>
  <audio ref="audioElement" class="audio-engine" preload="none" aria-hidden="true"></audio>
  <AppShell />
</template>
