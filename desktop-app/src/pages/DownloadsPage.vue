<script setup lang="ts">
import { CircleAlert, Download } from '@lucide/vue'
import PageState from './PageState.vue'
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Downloads</h1>
        <p>Only sources with explicit download permission can enter a queue.</p>
      </div>
    </header>

    <section class="content-region">
      <PageState
        :icon="Download"
        title="Downloads are not available yet"
        description="No authorized download worker is enabled, so no task can be queued. Sources with unclear or denied permissions must not be downloaded."
      />
      <p class="security-note"><CircleAlert :size="16" aria-hidden="true" /> YouTube content is not available for offline download here.</p>
    </section>
  </div>
</template>