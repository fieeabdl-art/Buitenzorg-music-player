<script setup lang="ts">
import { ArrowLeft, UserRound } from '@lucide/vue'
import { RouterLink } from 'vue-router'
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Local profile</h1>
        <p>No online account or sign-in is required.</p>
      </div>
    </header>

    <section class="profile-sheet" aria-labelledby="profile-state-title">
      <div class="profile-avatar" aria-hidden="true"><UserRound :size="25" :stroke-width="1.5" /></div>
      <div class="profile-sheet__content">
        <span class="state-label">Profile not created</span>
        <h2 id="profile-state-title">Set up a private profile</h2>
        <p>A display name will be required. Username and profile photo are optional; no online account is needed.</p>
        <dl class="profile-fields">
          <div><dt>Display name</dt><dd>Required</dd></div>
          <div><dt>Username</dt><dd>Optional</dd></div>
          <div><dt>Profile photo</dt><dd>Optional</dd></div>
        </dl>
        <button class="primary-button" type="button" title="Local profile storage is not available in this build" disabled>
          Create local profile
        </button>
        <p class="profile-note">Profile setup is unavailable until local storage is connected. Nothing is being saved yet.</p>
        <RouterLink class="quiet-link profile-settings-link" :to="{ name: 'settings' }">
          <ArrowLeft :size="15" aria-hidden="true" />
          Back to settings
        </RouterLink>
      </div>
    </section>
  </div>
</template>