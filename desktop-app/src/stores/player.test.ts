import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { PlayerTrack, YouTubePlayerController, YouTubePlayerTrack } from '../types/player'
import { usePlayerStore } from './player'

const playbackServices = vi.hoisted(() => ({
  getLastPlayback: vi.fn(),
  resolveLocalTrack: vi.fn(),
  startPlaybackHistory: vi.fn(),
  startYouTubePlaybackHistory: vi.fn(),
  updatePlaybackHistory: vi.fn(),
  youtubeHistorySequence: 500,
}))

vi.mock('../services/audioPlayback', () => playbackServices)

class FakeAudio extends EventTarget {
  currentTime = 0
  duration = 45
  volume = 1
  muted = false
  paused = true
  src = ''
  error: MediaError | null = null
  playFailure: unknown = null
  mediaErrorCode: number | null = null

  get currentSrc(): string {
    return this.src
  }

  load(): void {
    this.error = null
    if (this.src) this.dispatchEvent(new Event('loadedmetadata'))
  }

  async play(): Promise<void> {
    if (this.playFailure) {
      if (this.mediaErrorCode !== null) this.error = { code: this.mediaErrorCode } as MediaError
      throw this.playFailure
    }
    this.paused = false
    this.dispatchEvent(new Event('play'))
  }

  pause(): void {
    if (this.paused) return
    this.paused = true
    this.dispatchEvent(new Event('pause'))
  }

  removeAttribute(name: string): void {
    if (name === 'src') this.src = ''
  }
}

const firstTrack: PlayerTrack = {
  id: 1,
  title: 'First track',
  artist: 'Local artist',
  album: 'Local album',
  format: 'wav',
  durationSeconds: 45,
  availability: 'available',
}

const secondTrack: PlayerTrack = { ...firstTrack, id: 2, title: 'Second track' }
const thirdTrack: PlayerTrack = { ...firstTrack, id: 3, title: 'Third track' }
const youtubeTrack: YouTubePlayerTrack = {
  id: -201,
  title: 'YouTube result',
  artist: 'YouTube channel',
  album: 'YouTube',
  format: 'youtube',
  durationSeconds: 45,
  availability: 'available',
  artworkUrl: 'https://i.ytimg.com/vi/M7lc1UVf-VE/hqdefault.jpg',
  source: 'youtube',
  videoId: 'M7lc1UVf-VE',
}
const secondYouTubeTrack: YouTubePlayerTrack = { ...youtubeTrack, id: -202, videoId: 'dQw4w9WgXcQ', title: 'Next YouTube result' }

class FakeYouTubeController implements YouTubePlayerController {
  currentVideoId = ''
  currentTime = 0
  duration = 45
  volume = 0
  muted = false
  state = -1
  loadVideoById = vi.fn((videoId: string) => { this.currentVideoId = videoId })
  playVideo = vi.fn(() => { this.state = 1 })
  pauseVideo = vi.fn(() => { this.state = 2 })
  stopVideo = vi.fn(() => { this.state = -1 })
  seekTo = vi.fn((seconds: number) => { this.currentTime = seconds })
  setVolume = vi.fn((volume: number) => { this.volume = volume })
  setMuted = vi.fn((muted: boolean) => { this.muted = muted })
  getCurrentTime = vi.fn(() => this.currentTime)
  getDuration = vi.fn(() => this.duration)
  getPlayerState = vi.fn(() => this.state)
  getVideoData = vi.fn(() => ({ video_id: this.currentVideoId, title: 'YouTube result', author: 'YouTube channel' }))
  getIframe = vi.fn(() => null)
  destroy = vi.fn()
}

function makeAudio(): FakeAudio {
  return new FakeAudio()
}

function attach(player: ReturnType<typeof usePlayerStore>, audio: FakeAudio): void {
  player.attachAudio(audio as unknown as HTMLAudioElement)
}

describe('persistent local audio player', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.restoreAllMocks()
    vi.clearAllMocks()
    playbackServices.getLastPlayback.mockResolvedValue(null)
    playbackServices.resolveLocalTrack.mockImplementation(async (trackId: number) => ({
      track: trackId === 2 ? secondTrack : trackId === 3 ? thirdTrack : firstTrack,
      audioPath: `/managed/${trackId}.wav`,
      audioUrl: `asset://managed/${trackId}.wav`,
    }))
    playbackServices.updatePlaybackHistory.mockResolvedValue(undefined)
    playbackServices.startPlaybackHistory.mockImplementation(async (trackId: number) => trackId + 100)
    playbackServices.youtubeHistorySequence = 500
    playbackServices.startYouTubePlaybackHistory.mockImplementation(async () => -++playbackServices.youtubeHistorySequence)
  })

  it('keeps one audio element attached and permits idempotent attachment', () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    attach(player, audio)
    expect(player.audioElement).toBe(audio)
  })

  it('rejects a second audio element', () => {
    const player = usePlayerStore()
    attach(player, makeAudio())
    expect(() => attach(player, makeAudio())).toThrow('Buitenzorg Player supports one persistent audio element.')
  })

  it('resynchronizes playback status with the attached audio host', () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    player.isPlaying = true
    player.attachAudio(audio as unknown as HTMLAudioElement)
    expect(player.isPlaying).toBe(false)

    audio.src = 'http://127.0.0.1:41000/session/track.wav'
    audio.paused = false
    player.attachAudio(audio as unknown as HTMLAudioElement)
    expect(player.isPlaying).toBe(true)
  })

  it('plays, pauses, resumes, seeks, changes volume and advances the same queue', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)

    await player.playQueue([firstTrack, secondTrack])
    expect(audio.src).toBe('asset://managed/1.wav')
    expect(player.currentTrack?.title).toBe('First track')
    expect(player.isPlaying).toBe(true)
    expect(player.duration).toBe(45)

    audio.currentTime = 12
    audio.dispatchEvent(new Event('timeupdate'))
    expect(player.currentTime).toBe(12)
    player.seek(25)
    expect(audio.currentTime).toBe(25)
    player.setVolume(0.35)
    expect(audio.volume).toBe(0.35)
    player.toggleMute()
    expect(audio.muted).toBe(true)

    player.pause()
    expect(player.isPlaying).toBe(false)
    await player.resume()
    expect(player.isPlaying).toBe(true)
    await player.next()
    expect(player.queueIndex).toBe(1)
    expect(player.currentTrack?.title).toBe('Second track')
    expect(audio.src).toBe('asset://managed/2.wav')
    expect(playbackServices.startPlaybackHistory).toHaveBeenCalledWith(1)
    expect(playbackServices.startPlaybackHistory).toHaveBeenCalledWith(2)

    await player.previous()
    expect(player.currentTrack?.title).toBe('First track')
    player.stop()
    expect(audio.paused).toBe(true)
    expect(audio.currentTime).toBe(0)
    expect(player.isPlaying).toBe(false)
  })

  it('starts selected YouTube results in the embedded player and synchronizes its controls and history', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    const youtube = new FakeYouTubeController()
    attach(player, audio)
    player.attachYouTubeController(youtube)

    await player.playTrack(youtubeTrack, [youtubeTrack, secondYouTubeTrack])
    expect(player.isYouTubePlayback).toBe(true)
    expect(player.isLoading).toBe(true)
    expect(player.isPlaying).toBe(false)
    expect(audio.paused).toBe(true)
    expect(audio.src).toBe('')
    expect(youtube.loadVideoById).toHaveBeenLastCalledWith(youtubeTrack.videoId)
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledWith(youtubeTrack))

    youtube.state = 1
    player.handleYouTubeStateChange(1)
    expect(player.isPlaying).toBe(true)
    youtube.currentTime = 8
    player.updateYouTubeTime()
    player.pause()
    expect(youtube.pauseVideo).toHaveBeenCalledOnce()
    expect(player.isPlaying).toBe(false)
    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(-501, 8, 8, 'paused'))

    player.seek(14)
    player.setVolume(0.35)
    player.toggleMute()
    expect(youtube.seekTo).toHaveBeenLastCalledWith(14)
    expect(youtube.setVolume).toHaveBeenLastCalledWith(35)
    expect(youtube.setMuted).toHaveBeenLastCalledWith(true)
    await player.resume()
    expect(youtube.playVideo).toHaveBeenCalledOnce()
    youtube.state = 1
    player.handleYouTubeStateChange(1)
    player.togglePlayback()
    expect(youtube.pauseVideo).toHaveBeenCalledTimes(2)
    expect(player.isPlaying).toBe(false)
  })

  it('advances and reverses the YouTube queue through the same iframe controller', async () => {
    const player = usePlayerStore()
    const youtube = new FakeYouTubeController()
    player.attachYouTubeController(youtube)
    await player.playQueue([youtubeTrack, secondYouTubeTrack])
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledTimes(1))

    youtube.state = 1
    player.handleYouTubeStateChange(1)
    await player.next()
    expect(player.queueIndex).toBe(1)
    expect(player.currentTrack?.title).toBe('Next YouTube result')
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledTimes(2))
    expect(youtube.loadVideoById).toHaveBeenLastCalledWith(secondYouTubeTrack.videoId)

    await player.previous()
    expect(player.queueIndex).toBe(0)
    expect(player.currentYouTubeTrack?.videoId).toBe(youtubeTrack.videoId)
    expect(youtube.loadVideoById).toHaveBeenLastCalledWith(youtubeTrack.videoId)
  })

  it('stops YouTube before local playback and pauses the local host before YouTube playback', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    const youtube = new FakeYouTubeController()
    attach(player, audio)
    player.attachYouTubeController(youtube)

    await player.playQueue([youtubeTrack])
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledOnce())
    youtube.state = 1
    player.handleYouTubeStateChange(1)
    await player.playTrack(firstTrack)
    expect(youtube.stopVideo).toHaveBeenCalled()
    expect(audio.src).toBe('asset://managed/1.wav')
    expect(audio.paused).toBe(false)
    expect(player.isYouTubePlayback).toBe(false)

    await player.playTrack(youtubeTrack)
    expect(audio.paused).toBe(true)
    expect(audio.src).toBe('')
    expect(player.isYouTubePlayback).toBe(true)
    expect(youtube.loadVideoById).toHaveBeenLastCalledWith(youtubeTrack.videoId)
  })

  it('records unavailable YouTube playback as failed without reporting active audio', async () => {
    const player = usePlayerStore()
    const youtube = new FakeYouTubeController()
    player.attachYouTubeController(youtube)
    await player.playQueue([youtubeTrack])
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledOnce())

    player.handleYouTubeError(101)
    expect(player.isPlaying).toBe(false)
    expect(player.error).toContain('does not allow playback')
    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(-501, 0, 0, 'failed'))
  })

  it('lets the app pause a buffering YouTube video without marking it as playing', async () => {
    const player = usePlayerStore()
    const youtube = new FakeYouTubeController()
    player.attachYouTubeController(youtube)
    await player.playQueue([youtubeTrack])
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledOnce())

    player.handleYouTubeStateChange(3)
    expect(player.isLoading).toBe(true)
    expect(player.isPlaying).toBe(false)
    player.togglePlayback()
    expect(youtube.pauseVideo).toHaveBeenCalledOnce()
    expect(player.isLoading).toBe(false)
    expect(player.isPlaying).toBe(false)
  })

  it('retries a failed IFrame API load by mounting a fresh visible player', async () => {
    const player = usePlayerStore()
    await player.playQueue([youtubeTrack])
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledOnce())
    player.handleYouTubeLoadFailure()
    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(-501, 0, 0, 'failed'))
    expect(player.error).toContain('could not load')

    await player.resume()
    expect(player.youtubePlayerRevision).toBe(1)
    expect(player.isLoading).toBe(true)
    expect(player.error).toBe('')
    await vi.waitFor(() => expect(playbackServices.startYouTubePlaybackHistory).toHaveBeenCalledTimes(2))

    const youtube = new FakeYouTubeController()
    player.attachYouTubeController(youtube)
    expect(youtube.loadVideoById).toHaveBeenLastCalledWith(youtubeTrack.videoId)
  })

  it('counts playback time without counting seek jumps', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack])

    audio.currentTime = 10
    audio.dispatchEvent(new Event('timeupdate'))
    player.seek(25)
    audio.currentTime = 26
    audio.dispatchEvent(new Event('timeupdate'))
    player.pause()

    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 26, 11, 'paused'))
  })

  it('saves the final listened interval when the pause event arrives after state changes', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack])

    audio.currentTime = 8
    audio.paused = true
    player.pause()
    audio.dispatchEvent(new Event('pause'))

    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 8, 8, 'paused'))
  })

  it('saves the final listened interval when the audio host fails', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack])

    audio.currentTime = 8
    audio.error = { code: 3 } as MediaError
    audio.dispatchEvent(new Event('error'))

    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 8, 8, 'failed'))
  })

  it('autoplays the next track when the current track ends', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack, secondTrack])

    audio.currentTime = audio.duration
    audio.paused = true
    audio.dispatchEvent(new Event('ended'))
    await vi.waitFor(() => expect(player.isPlaying).toBe(true))

    expect(player.queueIndex).toBe(1)

    expect(player.currentTrack?.title).toBe('Second track')
    expect(player.isPlaying).toBe(true)
    expect(audio.src).toBe('asset://managed/2.wav')
  })

  it('supports shuffle, repeat-all and repeat-one behavior', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack, secondTrack, thirdTrack])

    player.toggleShuffle()
    vi.spyOn(Math, 'random').mockReturnValue(0)
    await player.next()
    expect(player.queueIndex).toBe(1)

    player.toggleShuffle()
    player.queueIndex = 2
    player.cycleRepeat()
    expect(player.repeatMode).toBe('all')
    await player.next()
    expect(player.queueIndex).toBe(0)

    player.cycleRepeat()
    expect(player.repeatMode).toBe('one')
    audio.currentTime = 45
    audio.paused = true
    audio.dispatchEvent(new Event('ended'))
    await Promise.resolve()
    expect(player.currentTime).toBe(0)
    expect(player.isPlaying).toBe(true)
    const historyCallsBeforeRepeat = playbackServices.startPlaybackHistory.mock.calls.length
    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 0, expect.any(Number), 'playing'))
    expect(playbackServices.startPlaybackHistory).toHaveBeenCalledTimes(historyCallsBeforeRepeat)
  })

  it('reports missing files and unsupported media instead of claiming playback', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    playbackServices.resolveLocalTrack.mockRejectedValueOnce('This track\'s managed audio file is missing.')

    await player.playQueue([firstTrack])
    expect(player.isPlaying).toBe(false)
    expect(player.error).toContain('managed audio file is missing')
    expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 0, 0, 'failed')

    audio.playFailure = new Error('Unsupported media')
    playbackServices.resolveLocalTrack.mockResolvedValueOnce({
      track: firstTrack,
      audioPath: '/managed/1.wav',
      audioUrl: 'asset://managed/1.wav',
    })
    audio.mediaErrorCode = 4
    await player.playQueue([firstTrack])
    expect(player.isPlaying).toBe(false)
    expect(player.error).toContain('not supported by the current system WebView')

    audio.mediaErrorCode = null
    audio.playFailure = Object.assign(new Error('Audio endpoint unavailable'), { name: 'NotReadableError' })
    await player.playQueue([firstTrack])
    expect(player.isPlaying).toBe(false)
    expect(player.error).toContain('Check the system audio device')
  })

  it('restores track metadata and saved position without autoplay', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    playbackServices.getLastPlayback.mockResolvedValueOnce({
      track: firstTrack,
      historyId: 77,
      positionSeconds: 16,
      listenedSeconds: 9,
      audioPath: '/managed/1.wav',
      audioUrl: 'asset://managed/1.wav',
    })

    await player.restoreLastSession()

    expect(player.currentTrack?.id).toBe(1)
    expect(audio.src).toBe('asset://managed/1.wav')
    expect(audio.currentTime).toBe(16)
    expect(player.isPlaying).toBe(false)
  })

  it('only detaches the matching audio element and clears playback on detach', () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    player.detachAudio(makeAudio() as unknown as HTMLAudioElement)
    expect(player.audioElement).toBe(audio)

    player.detachAudio(audio as unknown as HTMLAudioElement)
    expect(player.audioElement).toBeNull()
  })

  it('does not start a pending source after the persistent host is detached', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    let resolveSource!: (source: { track: PlayerTrack; audioPath: string; audioUrl: string }) => void
    playbackServices.resolveLocalTrack.mockImplementationOnce(() => new Promise((resolve) => {
      resolveSource = resolve
    }))

    const pendingPlayback = player.playQueue([firstTrack])
    await vi.waitFor(() => expect(typeof resolveSource).toBe('function'))
    player.detachAudio(audio as unknown as HTMLAudioElement)
    resolveSource({ track: firstTrack, audioPath: '/managed/1.wav', audioUrl: 'asset://managed/1.wav' })
    await pendingPlayback

    expect(audio.src).toBe('')
    expect(audio.paused).toBe(true)
    expect(player.isPlaying).toBe(false)
  })

  it('saves the latest position when the persistent host closes', async () => {
    const player = usePlayerStore()
    const audio = makeAudio()
    attach(player, audio)
    await player.playQueue([firstTrack])
    audio.currentTime = 23

    player.detachAudio(audio as unknown as HTMLAudioElement)
    await vi.waitFor(() => expect(playbackServices.updatePlaybackHistory).toHaveBeenLastCalledWith(101, 23, 23, 'paused'))
  })
})