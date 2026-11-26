<script setup lang="ts">
import {
  Compass,
  Download,
  Headphones,
  History,
  Info,
  LibraryBig,
  ListMusic,
  MoreHorizontal,
  Search,
  Settings2,
  UserRound,
} from '@lucide/vue'
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import BrandMark from './BrandMark.vue'

const router = useRouter()
const searchQuery = ref('')
const moreMenu = ref<HTMLDetailsElement | null>(null)

const primaryLinks = [
  { label: 'Listen', name: 'home', icon: Headphones },
  { label: 'Library', name: 'library', icon: LibraryBig },
  { label: 'Discover', name: 'discover', icon: Compass },
  { label: 'Playlists', name: 'playlists', icon: ListMusic },
]

function submitSearch() {
  void router.push({ name: 'search', query: searchQuery.value.trim() ? { q: searchQuery.value.trim() } : {} })
}

function closeMoreMenu() {
  moreMenu.value?.removeAttribute('open')
}
</script>

<template>
  <header class="top-navigation">
    <RouterLink class="brand-link" :to="{ name: 'home' }" aria-label="Buitenzorg Player home" @click="closeMoreMenu">
      <BrandMark />
      <span class="brand-wordmark">BUITENZORG</span>
    </RouterLink>

    <nav class="primary-navigation" aria-label="Primary navigation">
      <RouterLink
        v-for="link in primaryLinks"
        :key="link.name"
        class="navigation-link"
        :to="{ name: link.name }"
        @click="closeMoreMenu"
        active-class="navigation-link--active"
        exact-active-class="navigation-link--active"
      >
        <component :is="link.icon" :size="17" :stroke-width="1.8" aria-hidden="true" />
        <span>{{ link.label }}</span>
      </RouterLink>
    </nav>

    <form class="global-search" role="search" @submit.prevent="submitSearch">
      <Search :size="17" :stroke-width="1.8" aria-hidden="true" />
      <label class="sr-only" for="global-search-input">Search music</label>
      <input
        id="global-search-input"
        v-model="searchQuery"
        type="search"
        placeholder="Search music"
        autocomplete="off"
      />
      <kbd>Enter</kbd>
    </form>

    <details ref="moreMenu" class="more-menu" @keydown.esc="closeMoreMenu">
      <summary aria-label="More destinations">
        <MoreHorizontal :size="19" :stroke-width="1.8" aria-hidden="true" />
        <span>More</span>
      </summary>
      <nav class="more-menu__panel" aria-label="More destinations">
        <RouterLink :to="{ name: 'downloads' }" @click="closeMoreMenu"><Download :size="16" aria-hidden="true" />Downloads</RouterLink>
        <RouterLink :to="{ name: 'history' }" @click="closeMoreMenu"><History :size="16" aria-hidden="true" />History</RouterLink>
        <RouterLink :to="{ name: 'settings' }" @click="closeMoreMenu"><Settings2 :size="16" aria-hidden="true" />Settings</RouterLink>
        <RouterLink :to="{ name: 'profile' }" @click="closeMoreMenu"><UserRound :size="16" aria-hidden="true" />Profile</RouterLink>
        <RouterLink :to="{ name: 'about' }" @click="closeMoreMenu"><Info :size="16" aria-hidden="true" />About</RouterLink>
      </nav>
    </details>
  </header>
</template>