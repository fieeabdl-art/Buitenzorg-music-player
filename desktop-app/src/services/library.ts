import { invoke, isTauri } from '@tauri-apps/api/core'
import type {
  ImportReport,
  LibraryCategory,
  LibraryPage,
  LibraryStorageInfo,
  PlaylistSummary,
  PlaylistTrack,
} from '../types/library'

function callDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri()) {
    return Promise.reject(new Error('Local library actions are available in the desktop app only.'))
  }
  return invoke<T>(command, args)
}

export const libraryService = {
  getPage(category: LibraryCategory, offset = 0) {
    return callDesktop<LibraryPage>('get_library_page', { category, offset })
  },
  searchLocal(query: string, offset = 0) {
    return callDesktop<LibraryPage>('search_local_library', { query, offset })
  },
  importAudioFiles() {
    return callDesktop<ImportReport>('import_audio_files')
  },
  scanAudioFolder() {
    return callDesktop<ImportReport>('scan_audio_folder')
  },
  migrateLegacyLibrary() {
    return callDesktop<ImportReport>('migrate_legacy_library')
  },
  getStorageInfo() {
    return callDesktop<LibraryStorageInfo>('get_library_storage_info')
  },
  listPlaylists() {
    return callDesktop<PlaylistSummary[]>('list_playlists')
  },
  getPlaylistTracks(playlistId: number) {
    return callDesktop<PlaylistTrack[]>('get_playlist_tracks', { playlistId })
  },
  createPlaylist(name: string, description = '') {
    return callDesktop<number>('create_playlist', { name, description })
  },
  updatePlaylist(playlistId: number, name: string, description: string) {
    return callDesktop<void>('update_playlist', { playlistId, name, description })
  },
  deletePlaylist(playlistId: number) {
    return callDesktop<void>('delete_playlist', { playlistId })
  },
  addTrackToPlaylist(playlistId: number, trackId: number) {
    return callDesktop<void>('add_track_to_playlist', { playlistId, trackId })
  },
  removeTrackFromPlaylist(playlistId: number, trackId: number) {
    return callDesktop<void>('remove_track_from_playlist', { playlistId, trackId })
  },
  reorderPlaylistTrack(playlistId: number, trackId: number, position: number) {
    return callDesktop<void>('reorder_playlist_track', { playlistId, trackId, position })
  },
}