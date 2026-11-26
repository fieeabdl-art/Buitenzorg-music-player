<script setup lang="ts">
import type { Component } from 'vue'
import { RouterLink } from 'vue-router'

defineProps<{
  icon?: Component
  title: string
  description: string
  actionLabel?: string
  actionTo?: string
  state?: 'empty' | 'loading' | 'error'
}>()
</script>

<template>
  <section
    class="page-state"
    :class="`page-state--${state ?? 'empty'}`"
    :role="state === 'error' ? 'alert' : 'status'"
    :aria-live="state === 'error' ? 'assertive' : 'polite'"
    :aria-busy="state === 'loading' ? 'true' : undefined"
  >
    <component
      :is="icon"
      v-if="icon"
      class="page-state__icon"
      :class="{ spin: state === 'loading' }"
      :size="22"
      :stroke-width="1.7"
      aria-hidden="true"
    />
    <div class="page-state__copy">
      <h2>{{ title }}</h2>
      <p>{{ description }}</p>
      <RouterLink v-if="actionLabel && actionTo" class="text-link" :to="actionTo">
        {{ actionLabel }}
      </RouterLink>
    </div>
  </section>
</template>