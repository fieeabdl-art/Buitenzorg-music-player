import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mocks = vi.hoisted(() => ({
  isTauri: true,
  getStatus: vi.fn(),
  saveApiKey: vi.fn(),
  removeApiKey: vi.fn(),
  checkConnection: vi.fn(),
  search: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  isTauri: () => mocks.isTauri,
}))

vi.mock('../services/providers/youtube', () => ({
  youtubeAdapter: {
    getStatus: mocks.getStatus,
    saveApiKey: mocks.saveApiKey,
    removeApiKey: mocks.removeApiKey,
    checkConnection: mocks.checkConnection,
    search: mocks.search,
  },
}))

import { useProvidersStore } from './providers'

const connectedStatus = {
  provider: 'youtube',
  state: 'connected' as const,
  hasApiKey: true,
  checkedAt: '2026-11-26T00:00:00Z',
  message: 'YouTube search is connected.',
}

const video = {
  videoId: 'abcdefghijk',
  title: 'A real result',
  artist: 'A channel',
  description: '',
  thumbnailUrl: null,
  durationSeconds: 180,
  sourceUrl: 'https://www.youtube.com/watch?v=abcdefghijk',
}

function searchResponse(query: string) {
  return {
    query,
    state: 'ready' as const,
    items: [video],
    message: null,
    providerStatus: connectedStatus,
  }
}

describe('music provider store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mocks.isTauri = true
    vi.clearAllMocks()
    mocks.getStatus.mockResolvedValue(connectedStatus)
    mocks.saveApiKey.mockResolvedValue({ ...connectedStatus, state: 'available' })
    mocks.removeApiKey.mockResolvedValue({
      provider: 'youtube', state: 'unavailable', hasApiKey: false, checkedAt: null, message: 'Key removed.',
    })
    mocks.checkConnection.mockResolvedValue(connectedStatus)
    mocks.search.mockImplementation((query: string) => Promise.resolve(searchResponse(query)))
  })

  it('does not call the desktop provider from browser preview', async () => {
    mocks.isTauri = false
    const providers = useProvidersStore()

    await providers.searchYoutube('  song  ')

    expect(providers.youtubeResults?.state).toBe('unavailable')
    expect(providers.youtubeResults?.message).toContain('desktop app only')
    expect(providers.isSearchingYoutube).toBe(false)
    expect(mocks.search).not.toHaveBeenCalled()
  })

  it('trims provider queries and keeps the response status with real results', async () => {
    const providers = useProvidersStore()

    await providers.searchYoutube('  song title  ')

    expect(mocks.search).toHaveBeenCalledWith('song title')
    expect(providers.youtubeResults?.items).toEqual([video])
    expect(providers.youtubeStatus).toEqual(connectedStatus)
    expect(providers.isSearchingYoutube).toBe(false)
  })

  it('ignores an older response after a newer search has finished', async () => {
    let resolveOlder!: (response: ReturnType<typeof searchResponse>) => void
    mocks.search.mockImplementation((query: string) => query === 'older'
      ? new Promise((resolve) => { resolveOlder = resolve })
      : Promise.resolve(searchResponse(query)))
    const providers = useProvidersStore()
    const olderSearch = providers.searchYoutube('older')

    await providers.searchYoutube('newer')
    resolveOlder(searchResponse('older'))
    await olderSearch

    expect(providers.youtubeResults?.query).toBe('newer')
    expect(providers.isSearchingYoutube).toBe(false)
  })

  it('clears provider results when the saved key is removed', async () => {
    const providers = useProvidersStore()
    await providers.searchYoutube('song')

    await expect(providers.removeYoutubeApiKey()).resolves.toBe(true)

    expect(providers.youtubeResults).toBeNull()
    expect(providers.youtubeStatus?.hasApiKey).toBe(false)
  })

  it('does not report a successful connection check as an error', async () => {
    const providers = useProvidersStore()

    await expect(providers.checkYoutubeConnection()).resolves.toBe(true)

    expect(providers.actionError).toBe('')
    expect(providers.youtubeStatus).toEqual(connectedStatus)
  })
})