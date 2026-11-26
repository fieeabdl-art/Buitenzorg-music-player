import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { YouTubeVideo } from '../types/providers'
import { startYouTubePlaybackHistory, toYouTubePlayerTrack } from './audioPlayback'
import { youtubeAdapter } from './providers/youtube'

const desktopBridge = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => desktopBridge)

const video: YouTubeVideo = {
  videoId: 'M7lc1UVf-VE',
  title: 'Official player example',
  artist: 'YouTube channel',
  description: 'Video details from YouTube Data API.',
  thumbnailUrl: 'https://i.ytimg.com/vi/M7lc1UVf-VE/hqdefault.jpg',
  durationSeconds: 180,
  sourceUrl: 'https://www.youtube.com/watch?v=M7lc1UVf-VE',
}

describe('YouTube playback adapter', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    desktopBridge.isTauri.mockReturnValue(true)
    desktopBridge.invoke.mockResolvedValue(-1)
  })

  it('maps official search metadata without inventing a stream URL', () => {
    const track = toYouTubePlayerTrack(video)

    expect(track).toMatchObject({
      source: 'youtube',
      videoId: video.videoId,
      title: video.title,
      artist: video.artist,
      artworkUrl: video.thumbnailUrl,
      durationSeconds: video.durationSeconds,
      format: 'youtube',
    })
    expect(track.id).toBeLessThan(0)
    expect(track).not.toHaveProperty('audioUrl')
    expect(toYouTubePlayerTrack(video).id).toBe(track.id)
  })

  it('resolves YouTube results only to the official embedded player and denies downloads', () => {
    expect(youtubeAdapter.resolvePlayback(video)).toEqual({ kind: 'youtube-embed', videoId: video.videoId })
    expect(youtubeAdapter.downloadEligibility(video)).toBe('denied')
    expect('openSource' in youtubeAdapter).toBe(false)
  })

  it('registers a provider playback session through the Tauri command', async () => {
    const track = toYouTubePlayerTrack(video)

    await expect(startYouTubePlaybackHistory(track)).resolves.toBe(-1)
    expect(desktopBridge.invoke).toHaveBeenCalledWith('start_youtube_playback_history', {
      videoId: video.videoId,
      title: video.title,
      artist: video.artist,
      artworkUrl: video.thumbnailUrl,
      durationSeconds: video.durationSeconds,
    })
  })
})