import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { PlayerTrack } from '../types/player'
import { usePlayerStore } from './player'

const playbackServices = vi.hoisted(() => ({
  getLastPlayback: vi.fn(),
  resolveLocalTrack: vi.fn(),
  startPlaybackHistory: vi.fn(),
  updatePlaybackHistory: vi.fn(),
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