<script setup lang="ts">
import { ref, watch } from 'vue'
import BrandMark from './BrandMark.vue'

const props = withDefaults(defineProps<{
  artworkUrl?: string | null
  size?: 'small' | 'player' | 'hero'
}>(), {
  artworkUrl: null,
  size: 'small',
})

const failed = ref(false)

watch(() => props.artworkUrl, () => {
  failed.value = false
})
</script>

<template>
  <span class="track-artwork" :class="`track-artwork--${size}`" aria-hidden="true">
    <img v-if="artworkUrl && !failed" :src="artworkUrl" alt="" loading="lazy" decoding="async" @error="failed = true" />
    <BrandMark v-else />
  </span>
</template>