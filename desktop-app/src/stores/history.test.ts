import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { PlaybackHistoryEntry } from '../types/history'
import { useHistoryStore } from './history'

const historyServices = vi.hoisted(() => ({
  clearPlaybackHistory: vi.fn(),
  deletePlaybackHistory: vi.fn(),
  getPlaybackHistory: vi.fn(),
}))

vi.mock('../services/playbackHistory', () => historyServices)

const entry = (historyId: number, status: PlaybackHistoryEntry['status'] = 'completed'): PlaybackHistoryEntry => ({
  historyId,
  playedAt: '2026-11-26T09:00:00Z',
  positionSeconds: 30,
  listenedSeconds: 18,
  status,
  track: {
    id: historyId,
    title: `Track ${historyId}`,
    artist: 'Artist',
    album: 'Album',
    format: 'wav',
    durationSeconds: 60,
    availability: 'available',
    genre: null,
    year: null,
    trackNumber: null,
    artworkUrl: null,
  },
})

describe('playback history store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
    historyServices.getPlaybackHistory.mockResolvedValue([])
    historyServices.deletePlaybackHistory.mockResolvedValue(undefined)
    historyServices.clearPlaybackHistory.mockResolvedValue(0)
  })

  it('loads records and exposes an informative empty state', async () => {
    historyServices.getPlaybackHistory.mockResolvedValueOnce([entry(1)])
    const history = useHistoryStore()

    await history.load()
    expect(history.entries).toHaveLength(1)
    expect(history.status).toBe('ready')

    await history.load()
    expect(history.entries).toHaveLength(0)
    expect(history.status).toBe('empty')
  })

  it('removes one completed entry and keeps state synchronized', async () => {
    historyServices.getPlaybackHistory.mockResolvedValueOnce([entry(1), entry(2)])
    const history = useHistoryStore()
    await history.load()

    await history.removeEntry(1)
    expect(historyServices.deletePlaybackHistory).toHaveBeenCalledWith(1)
    expect(history.entries.map((item) => item.historyId)).toEqual([2])
  })

  it('clears finished entries but preserves active or paused playback', async () => {
    historyServices.getPlaybackHistory.mockResolvedValueOnce([entry(1), entry(2, 'playing'), entry(3, 'paused')])
    historyServices.getPlaybackHistory.mockResolvedValueOnce([entry(2, 'playing'), entry(3, 'paused')])
    historyServices.clearPlaybackHistory.mockResolvedValueOnce(1)
    const history = useHistoryStore()
    await history.load()

    await history.clearEntries()
    expect(history.entries.map((item) => item.historyId)).toEqual([2, 3])
    expect(history.feedback).toContain('Active playback is kept')
  })

  it('surfaces provider errors without discarding existing records', async () => {
    historyServices.getPlaybackHistory.mockResolvedValueOnce([entry(1)])
    const history = useHistoryStore()
    await history.load()
    historyServices.deletePlaybackHistory.mockRejectedValueOnce(new Error('Active playback cannot be removed.'))

    await history.removeEntry(1)
    expect(history.error).toBe('Active playback cannot be removed.')
    expect(history.entries).toHaveLength(1)
  })
})