<script setup lang="ts">
import { Compass } from '@lucide/vue'
import PageState from './PageState.vue'
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Discover</h1>
        <p>Music from sources that offer supported, official access.</p>
      </div>
    </header>

    <section class="content-region">
      <PageState
        :icon="Compass"
        title="No discovery sources are available"
        description="No official music source is connected in this build, so charts and recommendations are not shown."
        action-label="Review source availability"
        action-to="/settings"
      />
    </section>
  </div>
</template>