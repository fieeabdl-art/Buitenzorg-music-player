<script setup lang="ts">
import { AlertCircle, ArrowDown, ArrowUp, ListMusic, LoaderCircle, Play, Plus, Save, Trash2 } from '@lucide/vue'
import { computed, ref, watch } from 'vue'
import { RouterLink } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useLibraryStore } from '../stores/library'
import { toPlayerTrack } from '../services/audioPlayback'
import TrackArtwork from '../components/TrackArtwork.vue'
import { trackMetadataSummary } from '../services/trackMetadata'
import { usePlayerStore } from '../stores/player'
import PageState from './PageState.vue'

const appStore = useAppStore()
const library = useLibraryStore()
const player = usePlayerStore()
const newName = ref('')
const newDescription = ref('')
const editName = ref('')
const editDescription = ref('')
const newTrackId = ref<number | ''>('')
const pendingDelete = ref(false)
const busy = computed(() => library.operation === 'playlist')
const candidates = computed(() => library.trackChoices.filter(
  (track) => !library.playlistTracks.some((playlistTrack) => playlistTrack.id === track.id),
))

watch(
  () => appStore.bridgeState,
  (state) => {
    if (state === 'ready') {
      void library.loadPlaylists()
    } else if (state === 'browser') {
      library.playlistStatus = 'unavailable'
    } else if (state === 'error') {
      library.playlistStatus = 'error'
      library.playlistError = appStore.bridgeError
    }
  },
  { immediate: true },
)

watch(
  () => library.selectedPlaylist?.id,
  () => {
    editName.value = library.selectedPlaylist?.name ?? ''
    editDescription.value = library.selectedPlaylist?.description ?? ''
    pendingDelete.value = false
    newTrackId.value = ''
  },
  { immediate: true },
)

async function createPlaylist() {
  if (!newName.value.trim()) return
  await library.createPlaylist(newName.value, newDescription.value)
  if (!library.playlistError) {
    newName.value = ''
    newDescription.value = ''
  }
}

async function savePlaylist() {
  const playlist = library.selectedPlaylist
  if (!playlist) return
  await library.updatePlaylist(playlist.id, editName.value, editDescription.value)
}

async function addTrack() {
  const playlistId = library.selectedPlaylistId
  if (playlistId === null || newTrackId.value === '') return
  await library.addTrackToPlaylist(playlistId, newTrackId.value)
  newTrackId.value = ''
}

function moveTrack(index: number, direction: -1 | 1) {
  const playlistId = library.selectedPlaylistId
  const track = library.playlistTracks[index]
  const newPosition = index + direction
  if (playlistId === null || !track || newPosition < 0 || newPosition >= library.playlistTracks.length) return
  void library.reorderPlaylistTrack(playlistId, track.id, newPosition)
}

function playPlaylist(trackId?: number): void {
  const tracks = library.playlistTracks
    .filter((track) => track.availability === 'available')
    .map(toPlayerTrack)
  if (tracks.length === 0) {
    player.error = 'This playlist has no available local audio files.'
    return
  }
  const startIndex = trackId === undefined ? 0 : tracks.findIndex((track) => track.id === trackId)
  if (startIndex >= 0) void player.playQueue(tracks, startIndex)
}
</script>

<template>
  <div class="page">
    <header class="page-heading">
      <div>
        <h1>Playlists</h1>
        <p>Build listening sets from your local collection.</p>
      </div>
    </header>

    <section v-if="appStore.bridgeState === 'loading' || appStore.bridgeState === 'idle' || library.playlistStatus === 'loading'" class="content-region">
      <PageState :icon="LoaderCircle" title="Loading playlists" description="Reading locally saved listening sets." state="loading" />
    </section>
    <section v-else-if="library.playlistStatus === 'unavailable'" class="content-region">
      <PageState
        :icon="ListMusic"
        title="Open the desktop app to use playlists"
        description="The browser preview cannot access the local SQLite database."
      />
    </section>
    <section v-else-if="library.playlistStatus === 'error'" class="content-region">
      <PageState :icon="AlertCircle" title="Playlists could not be loaded" :description="library.playlistError || 'The local database did not respond.'" state="error" />
    </section>
    <div v-else class="playlist-workspace">
      <aside class="playlist-browser" aria-label="Saved playlists">
        <h2>Your playlists</h2>
        <form class="playlist-create-form" @submit.prevent="createPlaylist">
          <label for="new-playlist-name">New playlist</label>
          <input id="new-playlist-name" v-model="newName" maxlength="80" required placeholder="Playlist name" :disabled="busy" />
          <textarea v-model="newDescription" maxlength="500" rows="2" aria-label="Playlist description" placeholder="Description (optional)" :disabled="busy"></textarea>
          <button class="primary-button" type="submit" :disabled="busy || !newName.trim()"><Plus :size="16" aria-hidden="true" />Create playlist</button>
        </form>
        <p v-if="library.playlists.length === 0" class="playlist-list-empty">Your saved playlists will stay on this device.</p>
        <nav v-else class="playlist-list" aria-label="Playlist list">
          <button
            v-for="playlist in library.playlists"
            :key="playlist.id"
            class="playlist-list__item"
            :class="{ 'playlist-list__item--active': library.selectedPlaylistId === playlist.id }"
            type="button"
            :aria-current="library.selectedPlaylistId === playlist.id ? 'true' : undefined"
            @click="library.selectPlaylist(playlist.id)"
          >
            <span>{{ playlist.name }}<small>{{ playlist.trackCount }} {{ playlist.trackCount === 1 ? 'track' : 'tracks' }}</small></span>
            <ListMusic :size="16" aria-hidden="true" />
          </button>
        </nav>
      </aside>

      <section v-if="library.selectedPlaylist" class="playlist-detail" aria-label="Selected playlist">
        <form class="playlist-edit-form" @submit.prevent="savePlaylist">
          <label for="edit-playlist-name">Playlist details</label>
          <div class="playlist-edit-form__row">
            <input id="edit-playlist-name" v-model="editName" maxlength="80" required :disabled="busy" />
            <button class="secondary-button" type="submit" :disabled="busy || !editName.trim()"><Save :size="15" aria-hidden="true" />Save</button>
            <button v-if="!pendingDelete" class="quiet-button quiet-button--danger" type="button" :disabled="busy" @click="pendingDelete = true"><Trash2 :size="15" aria-hidden="true" />Delete</button>
            <template v-else>
              <span class="delete-confirmation">Delete this playlist?</span>
              <button class="quiet-button quiet-button--danger" type="button" :disabled="busy" @click="library.deletePlaylist(library.selectedPlaylist!.id)">Confirm</button>
              <button class="quiet-button" type="button" :disabled="busy" @click="pendingDelete = false">Keep</button>
            </template>
          </div>
          <textarea v-model="editDescription" maxlength="500" rows="2" aria-label="Edit playlist description" placeholder="Add a description" :disabled="busy"></textarea>
        </form>

        <div class="playlist-track-heading">
          <div><h2>{{ library.selectedPlaylist.name }}</h2><p>{{ library.playlistTracks.length }} tracks · stored locally</p></div>
          <button v-if="library.playlistTracks.some((track) => track.availability === 'available')" class="secondary-button" type="button" @click="playPlaylist()"><Play :size="15" fill="currentColor" aria-hidden="true" />Play playlist</button>
        </div>

        <form class="playlist-add-track" @submit.prevent="addTrack">
          <label for="playlist-track-choice">Add from your library</label>
          <div>
            <select id="playlist-track-choice" v-model.number="newTrackId" :disabled="busy || candidates.length === 0">
              <option value="">{{ candidates.length ? 'Choose a local track' : 'No unlisted local tracks' }}</option>
              <option v-for="track in candidates" :key="track.id" :value="track.id">{{ track.title }} — {{ track.subtitle }}</option>
            </select>
            <button class="secondary-button" type="submit" :disabled="busy || newTrackId === ''"><Plus :size="15" aria-hidden="true" />Add track</button>
          </div>
          <RouterLink v-if="library.trackChoices.length === 0" class="text-link" :to="{ name: 'library' }">Import audio in your Library</RouterLink>
        </form>

        <p v-if="library.playlistError" class="library-error" role="alert"><AlertCircle :size="16" aria-hidden="true" />{{ library.playlistError }}</p>
        <p v-if="library.playlistTracks.some((track) => track.availability !== 'available')" class="library-error" role="status">Some playlist files are missing or unavailable; their metadata remains saved.</p>
        <ol v-if="library.playlistTracks.length" class="playlist-track-list">
          <li v-for="(track, index) in library.playlistTracks" :key="track.id">
            <span class="playlist-track-list__position">{{ String(index + 1).padStart(2, '0') }}</span>
            <TrackArtwork :artwork-url="track.artworkUrl" />
            <span class="playlist-track-list__copy"><strong>{{ track.title }}</strong><small>{{ track.artist }} · {{ track.album }} · {{ track.format.toUpperCase() }}</small><small v-if="trackMetadataSummary(track)">{{ trackMetadataSummary(track) }}</small></span>
            <span class="availability-badge" :class="`availability-badge--${track.availability || 'unavailable'}`">{{ track.availability === 'available' ? 'On device' : track.availability === 'missing' ? 'File missing' : 'Unavailable' }}</span>
            <div class="playlist-track-list__actions">
              <button type="button" :aria-label="`Play ${track.title}`" :disabled="track.availability !== 'available'" @click="playPlaylist(track.id)"><Play :size="14" fill="currentColor" aria-hidden="true" /></button>
              <button type="button" :aria-label="`Move ${track.title} up`" :disabled="busy || index === 0" @click="moveTrack(index, -1)"><ArrowUp :size="15" aria-hidden="true" /></button>
              <button type="button" :aria-label="`Move ${track.title} down`" :disabled="busy || index === library.playlistTracks.length - 1" @click="moveTrack(index, 1)"><ArrowDown :size="15" aria-hidden="true" /></button>
              <button type="button" :aria-label="`Remove ${track.title} from playlist`" :disabled="busy" @click="library.removeTrackFromPlaylist(library.selectedPlaylist!.id, track.id)"><Trash2 :size="15" aria-hidden="true" /></button>
            </div>
          </li>
        </ol>
        <PageState v-else :icon="ListMusic" title="This playlist is empty" description="Add tracks from your local library. No demo tracks are shown." />
      </section>
      <section v-else class="playlist-detail playlist-detail--empty">
        <PageState :icon="ListMusic" title="Create a local playlist" description="Playlists and their order are saved in the local database on this device." />
      </section>
    </div>
  </div>
</template>