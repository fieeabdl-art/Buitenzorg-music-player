<script setup lang="ts">
import { useAppStore } from '../stores/app'

const appStore = useAppStore()
</script>

<template>
  <div class="page about-page">
    <header class="page-heading">
      <div>
        <h1>Buitenzorg Player</h1>
        <p>Music, Redefined.</p>
      </div>
      <span v-if="appStore.appInfo" class="state-label">Version {{ appStore.appInfo.version }}</span>
    </header>

    <section class="philosophy" aria-labelledby="philosophy-title">
      <h2 id="philosophy-title">Our Philosophy</h2>
      <p>Music is more than something you play. It's something you experience.</p>
      <p>Buitenzorg is built around that belief, combining thoughtful design, reliable technology, and a personal listening experience.</p>
      <p>Every detail has a purpose. Every sound has its place.</p>
      <p class="philosophy-signature">Buitenzorg — Music, Redefined.</p>
    </section>

    <section class="project-owner" aria-labelledby="owner-title">
      <h2 id="owner-title">Project owner</h2>
      <p>Luthfie Abdul Hakim</p>
      <dl class="owner-links">
        <div><dt>Instagram</dt><dd>@luthfhkm_</dd></div>
        <div><dt>GitHub</dt><dd>@fieeabdl-art</dd></div>
      </dl>
    </section>
  </div>
</template>