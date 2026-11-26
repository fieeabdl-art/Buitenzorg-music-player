import { isTauri } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { libraryService } from '../services/library'
import type {
  ImportReport,
  LibraryCategory,
  LibraryItem,
  LibraryStorageInfo,
  PlaylistSummary,
  PlaylistTrack,
} from '../types/library'

export type LibraryLoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'unavailable' | 'error'
export type LibraryOperation = 'importing' | 'scanning' | 'migrating' | 'playlist' | null

function errorMessage(error: unknown): string {
  if (typeof error === 'string') {
    return error
  }
  if (error instanceof Error) {
    return error.message
  }
  return 'The local library request failed.'
}

function describeReport(report: ImportReport): string {
  const parts = [`${report.imported} imported`, `${report.duplicates} duplicate${report.duplicates === 1 ? '' : 's'}`]
  if (report.skipped > 0) {
    parts.push(`${report.skipped} skipped`)
  }
  return parts.join(' · ')
}

export const useLibraryStore = defineStore('library', () => {
  const category = ref<LibraryCategory>('all')
  const items = ref<LibraryItem[]>([])
  const totalCount = ref(0)
  const storageInfo = ref<LibraryStorageInfo | null>(null)
  const nextOffset = ref<number | null>(null)
  const status = ref<LibraryLoadState>('idle')
  const storageError = ref('')
  const error = ref('')
  const feedback = ref('')
  const lastReport = ref<ImportReport | null>(null)
  const operation = ref<LibraryOperation>(null)
  const searchQuery = ref('')
  const searchResults = ref<LibraryItem[]>([])
  const searchStatus = ref<LibraryLoadState>('idle')
  const searchError = ref('')
  const searchTotalCount = ref(0)
  const searchNextOffset = ref<number | null>(null)
  let searchRequestId = 0

  const playlists = ref<PlaylistSummary[]>([])
  const playlistTracks = ref<PlaylistTrack[]>([])
  const trackChoices = ref<LibraryItem[]>([])
  const selectedPlaylistId = ref<number | null>(null)
  const playlistStatus = ref<LibraryLoadState>('idle')
  const playlistError = ref('')
  const selectedPlaylist = computed(
    () => playlists.value.find((playlist) => playlist.id === selectedPlaylistId.value) ?? null,
  )

  async function loadPage(nextCategory: LibraryCategory = category.value, offset = 0, append = false) {
    category.value = nextCategory
    if (!isTauri()) {
      status.value = 'unavailable'
      items.value = []
      totalCount.value = 0
      return
    }
    if (!append) {
      status.value = 'loading'
      error.value = ''
    }

    try {
      const page = await libraryService.getPage(nextCategory, offset)
      items.value = append ? [...items.value, ...page.items] : page.items
      totalCount.value = page.totalCount
      nextOffset.value = page.nextOffset
      status.value = items.value.length > 0 ? 'ready' : 'empty'
    } catch (requestError) {
      status.value = 'error'
      error.value = errorMessage(requestError)
    }
  }

  async function loadStorageInfo() {
    if (!isTauri()) {
      storageInfo.value = null
      storageError.value = ''
      return
    }
    try {
      storageInfo.value = await libraryService.getStorageInfo()
      storageError.value = ''
    } catch (requestError) {
      storageError.value = errorMessage(requestError)
    }
  }

  async function loadLibrary() {
    await Promise.all([loadPage(category.value), loadStorageInfo()])
  }

  async function loadMore() {
    if (nextOffset.value !== null && status.value !== 'loading') {
      await loadPage(category.value, nextOffset.value, true)
    }
  }

  async function searchLocal(query: string, offset = 0, append = false) {
    const normalizedQuery = query.trim()
    searchQuery.value = normalizedQuery
    if (!normalizedQuery) {
      searchRequestId += 1
      searchResults.value = []
      searchStatus.value = 'idle'
      searchError.value = ''
      searchTotalCount.value = 0
      searchNextOffset.value = null
      return
    }
    if (!isTauri()) {
      searchRequestId += 1
      searchResults.value = []
      searchStatus.value = 'unavailable'
      searchTotalCount.value = 0
      searchNextOffset.value = null
      return
    }
    const requestId = ++searchRequestId
    if (!append) {
      searchStatus.value = 'loading'
      searchError.value = ''
    }
    try {
      const page = await libraryService.searchLocal(normalizedQuery, offset)
      if (requestId !== searchRequestId || normalizedQuery !== searchQuery.value) return
      searchResults.value = append ? [...searchResults.value, ...page.items] : page.items
      searchTotalCount.value = page.totalCount
      searchNextOffset.value = page.nextOffset
      searchStatus.value = searchResults.value.length > 0 ? 'ready' : 'empty'
    } catch (requestError) {
      if (requestId !== searchRequestId) return
      searchStatus.value = 'error'
      searchError.value = errorMessage(requestError)
    }
  }

  async function searchMore() {
    if (searchNextOffset.value !== null && searchStatus.value !== 'loading') {
      await searchLocal(searchQuery.value, searchNextOffset.value, true)
    }
  }

  function markUnavailable() {
    status.value = 'unavailable'
    items.value = []
    totalCount.value = 0
    storageInfo.value = null
  }

  async function runImportAction(
    nextOperation: Exclude<LibraryOperation, 'playlist' | null>,
    action: () => Promise<ImportReport>,
  ) {
    if (!isTauri()) {
      status.value = 'unavailable'
      return
    }
    operation.value = nextOperation
    feedback.value = ''
    error.value = ''
    lastReport.value = null
    try {
      const report = await action()
      if (report.cancelled) {
        return
      }
      lastReport.value = report
      feedback.value = describeReport(report)
      await Promise.all([loadPage(category.value), loadStorageInfo()])
    } catch (requestError) {
      error.value = errorMessage(requestError)
      status.value = 'error'
    } finally {
      operation.value = null
    }
  }

  function importAudioFiles() {
    return runImportAction('importing', () => libraryService.importAudioFiles())
  }

  function scanAudioFolder() {
    return runImportAction('scanning', () => libraryService.scanAudioFolder())
  }

  function migrateLegacyLibrary() {
    return runImportAction('migrating', () => libraryService.migrateLegacyLibrary())
  }

  async function loadPlaylists() {
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      playlists.value = []
      playlistTracks.value = []
      return
    }
    playlistStatus.value = 'loading'
    playlistError.value = ''
    try {
      playlists.value = await libraryService.listPlaylists()
      playlistStatus.value = playlists.value.length > 0 ? 'ready' : 'empty'
      if (selectedPlaylistId.value && !playlists.value.some((playlist) => playlist.id === selectedPlaylistId.value)) {
        selectedPlaylistId.value = null
        playlistTracks.value = []
      }
      if (selectedPlaylistId.value === null && playlists.value.length > 0) {
        selectedPlaylistId.value = playlists.value[0].id
      }
      if (selectedPlaylistId.value !== null) {
        await selectPlaylist(selectedPlaylistId.value)
      }
    } catch (requestError) {
      playlistStatus.value = 'error'
      playlistError.value = errorMessage(requestError)
    }
  }

  async function selectPlaylist(playlistId: number) {
    selectedPlaylistId.value = playlistId
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      return
    }
    try {
      const [tracks, page] = await Promise.all([
        libraryService.getPlaylistTracks(playlistId),
        libraryService.getPage('all', 0),
      ])
      playlistTracks.value = tracks
      trackChoices.value = page.items
      playlistError.value = ''
    } catch (requestError) {
      playlistError.value = errorMessage(requestError)
    }
  }

  async function createPlaylist(name: string, description: string) {
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      return
    }
    operation.value = 'playlist'
    playlistError.value = ''
    try {
      const playlistId = await libraryService.createPlaylist(name, description)
      selectedPlaylistId.value = playlistId
      await loadPlaylists()
    } catch (requestError) {
      playlistError.value = errorMessage(requestError)
    } finally {
      operation.value = null
    }
  }

  async function updatePlaylist(playlistId: number, name: string, description: string) {
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      return
    }
    operation.value = 'playlist'
    playlistError.value = ''
    try {
      await libraryService.updatePlaylist(playlistId, name, description)
      await loadPlaylists()
    } catch (requestError) {
      playlistError.value = errorMessage(requestError)
    } finally {
      operation.value = null
    }
  }

  async function deletePlaylist(playlistId: number) {
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      return
    }
    operation.value = 'playlist'
    playlistError.value = ''
    try {
      await libraryService.deletePlaylist(playlistId)
      if (selectedPlaylistId.value === playlistId) {
        selectedPlaylistId.value = null
        playlistTracks.value = []
      }
      await loadPlaylists()
    } catch (requestError) {
      playlistError.value = errorMessage(requestError)
    } finally {
      operation.value = null
    }
  }

  async function addTrackToPlaylist(playlistId: number, trackId: number) {
    await mutatePlaylist(() => libraryService.addTrackToPlaylist(playlistId, trackId), playlistId)
  }

  async function removeTrackFromPlaylist(playlistId: number, trackId: number) {
    await mutatePlaylist(() => libraryService.removeTrackFromPlaylist(playlistId, trackId), playlistId)
  }

  async function reorderPlaylistTrack(playlistId: number, trackId: number, position: number) {
    await mutatePlaylist(() => libraryService.reorderPlaylistTrack(playlistId, trackId, position), playlistId)
  }

  async function mutatePlaylist(action: () => Promise<void>, playlistId: number) {
    if (!isTauri()) {
      playlistStatus.value = 'unavailable'
      return
    }
    operation.value = 'playlist'
    playlistError.value = ''
    try {
      await action()
      await loadPlaylists()
      await selectPlaylist(playlistId)
    } catch (requestError) {
      playlistError.value = errorMessage(requestError)
    } finally {
      operation.value = null
    }
  }

  return {
    addTrackToPlaylist,
    category,
    createPlaylist,
    deletePlaylist,
    error,
    feedback,
    importAudioFiles,
    items,
    lastReport,
    loadLibrary,
    loadMore,
    loadPage,
    loadPlaylists,
    loadStorageInfo,
    markUnavailable,
    migrateLegacyLibrary,
    nextOffset,
    operation,
    playlistError,
    playlistStatus,
    playlistTracks,
    playlists,
    reorderPlaylistTrack,
    removeTrackFromPlaylist,
    scanAudioFolder,
    searchError,
    searchLocal,
    searchMore,
    searchNextOffset,
    searchQuery,
    searchResults,
    searchStatus,
    searchTotalCount,
    selectPlaylist,
    selectedPlaylist,
    selectedPlaylistId,
    status,
    storageError,
    storageInfo,
    totalCount,
    trackChoices,
    updatePlaylist,
  }
})