import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mocks = vi.hoisted(() => ({
  isTauri: true,
  getPage: vi.fn(),
  searchLocal: vi.fn(),
  importAudioFiles: vi.fn(),
  scanAudioFolder: vi.fn(),
  migrateLegacyLibrary: vi.fn(),
  getStorageInfo: vi.fn(),
  listPlaylists: vi.fn(),
  getPlaylistTracks: vi.fn(),
  createPlaylist: vi.fn(),
  updatePlaylist: vi.fn(),
  deletePlaylist: vi.fn(),
  addTrackToPlaylist: vi.fn(),
  removeTrackFromPlaylist: vi.fn(),
  reorderPlaylistTrack: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => mocks.isTauri,
}))

vi.mock('../services/library', () => ({
  libraryService: {
    getPage: mocks.getPage,
    searchLocal: mocks.searchLocal,
    importAudioFiles: mocks.importAudioFiles,
    scanAudioFolder: mocks.scanAudioFolder,
    migrateLegacyLibrary: mocks.migrateLegacyLibrary,
    getStorageInfo: mocks.getStorageInfo,
    listPlaylists: mocks.listPlaylists,
    getPlaylistTracks: mocks.getPlaylistTracks,
    createPlaylist: mocks.createPlaylist,
    updatePlaylist: mocks.updatePlaylist,
    deletePlaylist: mocks.deletePlaylist,
    addTrackToPlaylist: mocks.addTrackToPlaylist,
    removeTrackFromPlaylist: mocks.removeTrackFromPlaylist,
    reorderPlaylistTrack: mocks.reorderPlaylistTrack,
  },
}))

import { useLibraryStore } from './library'

describe('local library store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mocks.isTauri = true
    vi.clearAllMocks()
  })

  it('uses an honest unavailable state in browser preview without calling desktop commands', async () => {
    mocks.isTauri = false
    const library = useLibraryStore()

    await library.loadPage('all')

    expect(library.status).toBe('unavailable')
    expect(library.items).toEqual([])
    expect(mocks.getPage).not.toHaveBeenCalled()
  })

  it('loads real library page metadata and keeps pagination state', async () => {
    mocks.getPage.mockResolvedValue({
      category: 'all',
      offset: 0,
      totalCount: 2,
      storageBytes: 16,
      nextOffset: 1,
      items: [{
        id: 8,
        kind: 'track',
        title: 'Local song',
        subtitle: 'Local artist',
        detail: 'Local album',
        format: 'wav',
        sizeBytes: 16,
        durationSeconds: 1,
        addedAt: '2026-11-25T00:00:00Z',
        availability: 'available',
        hasArtwork: false,
        trackCount: null,
        genre: null,
        year: null,
        trackNumber: null,
        artworkUrl: null,
      }],
    })
    const library = useLibraryStore()

    await library.loadPage('all')

    expect(library.status).toBe('ready')
    expect(library.items[0]?.title).toBe('Local song')
    expect(library.totalCount).toBe(2)
    expect(library.nextOffset).toBe(1)
    expect(mocks.getPage).toHaveBeenCalledWith('all', 0)
  })

  it('refreshes library and storage after an import and surfaces the real report', async () => {
    mocks.importAudioFiles.mockResolvedValue({
      cancelled: false,
      scanned: 2,
      imported: 1,
      duplicates: 1,
      skipped: 0,
      issues: [],
    })
    mocks.getPage.mockResolvedValue({
      category: 'all', offset: 0, totalCount: 1, storageBytes: 24, nextOffset: null, items: [],
    })
    mocks.getStorageInfo.mockResolvedValue({
      storageBytes: 24, trackCount: 1,
    })
    const library = useLibraryStore()

    await library.importAudioFiles()

    expect(library.feedback).toBe('1 imported · 1 duplicate')
    expect(library.lastReport?.scanned).toBe(2)
    expect(library.storageInfo?.trackCount).toBe(1)
    expect(mocks.getPage).toHaveBeenCalledWith('all', 0)
    expect(mocks.getStorageInfo).toHaveBeenCalledOnce()
    expect(library.operation).toBeNull()
  })

  it('retains backend errors as visible state instead of manufacturing an empty result', async () => {
    mocks.getPage.mockRejectedValue('Database is unavailable.')
    const library = useLibraryStore()

    await library.loadPage('all')

    expect(library.status).toBe('error')
    expect(library.error).toBe('Database is unavailable.')
    expect(library.items).toEqual([])
  })

  it('searches local metadata and represents no matches as an honest empty state', async () => {
    mocks.searchLocal.mockResolvedValue({
      category: 'search',
      offset: 0,
      totalCount: 0,
      storageBytes: 0,
      nextOffset: null,
      items: [],
    })
    const library = useLibraryStore()

    await library.searchLocal('  not in this library  ')

    expect(mocks.searchLocal).toHaveBeenCalledWith('not in this library', 0)
    expect(library.searchQuery).toBe('not in this library')
    expect(library.searchStatus).toBe('empty')
    expect(library.searchResults).toEqual([])
  })
})