import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import { getLastPlayback, resolveLocalTrack, startPlaybackHistory, updatePlaybackHistory } from '../services/audioPlayback'
import type { PlaybackHistoryStatus } from '../types/history'
import type { PlaybackResumeSource, PlayerTrack, RepeatMode } from '../types/player'

const POSITION_SAVE_INTERVAL_SECONDS = 10

function audioErrorMessage(audio: HTMLAudioElement): string {
  switch (audio.error?.code) {
    case 1:
      return 'Playback was interrupted.'
    case 2:
      return 'The managed audio file could not be read. It may have been removed or access was denied.'
    case 3:
      return 'The audio decoder could not read this file. Its format may not be supported by this system.'
    case 4:
      return 'This audio format is not supported by the current system WebView.'
    default:
      return 'Audio output could not start. Check the system audio device and try again.'
  }
}

function playbackErrorMessage(error: unknown, audio: HTMLAudioElement): string {
  if (typeof error === 'string') return error
  if (audio.error) return audioErrorMessage(audio)
  if (error instanceof Error && error.name === 'NotAllowedError') {
    return 'Playback was blocked by the system. Press Play again to retry.'
  }
  if (error instanceof Error && ['AbortError', 'NotReadableError'].includes(error.name)) {
    return 'Audio output could not start. Check the system audio device and try again.'
  }
  if (error instanceof Error && error.message) return error.message
  return 'Audio output could not start. Check the system audio device and try again.'
}

export const usePlayerStore = defineStore('player', () => {
  const audioElement = shallowRef<HTMLAudioElement | null>(null)
  const queue = ref<PlayerTrack[]>([])
  const queueIndex = ref(-1)
  const currentTrack = computed(() => queue.value[queueIndex.value] ?? null)
  const isPlaying = ref(false)
  const isLoading = ref(false)
  const currentTime = ref(0)
  const duration = ref(0)
  const volume = ref(0.8)
  const isMuted = ref(false)
  const shuffle = ref(false)
  const repeatMode = ref<RepeatMode>('off')
  const error = ref('')
  const historyWarning = ref('')
  const historyRevision = ref(0)
  const queueOpen = ref(false)
  let historyId: number | null = null
  let lastSavedSecond = -1
  let lastSavedListenedSecond = -1
  let lastSavedHistoryId: number | null = null
  let lastSavedStatus: PlaybackHistoryStatus | null = null
  let listenedSeconds = 0
  let lastObservedPosition: number | null = null
  let pendingResumePosition: number | null = null
  let transitionId = 0
  let suppressPauseSave = false
  let restoreRequested = false
  let historyWrites = Promise.resolve()

  function updateTimeFromAudio() {
    const audio = audioElement.value
    if (!audio) return
    currentTime.value = Number.isFinite(audio.currentTime) ? Math.max(0, audio.currentTime) : 0
    if (historyId !== null && isPlaying.value && lastObservedPosition !== null && currentTime.value >= lastObservedPosition) {
      const difference = currentTime.value - lastObservedPosition
      listenedSeconds += difference
    }
    lastObservedPosition = currentTime.value
    if (Number.isFinite(audio.duration) && audio.duration > 0) {
      duration.value = audio.duration
    }
    if (historyId !== null && Math.floor(currentTime.value) - lastSavedSecond >= POSITION_SAVE_INTERVAL_SECONDS) {
      void persistPosition(Math.floor(currentTime.value))
    }
  }

  function handleLoadedMetadata() {
    const audio = audioElement.value
    if (!audio) return
    duration.value = Number.isFinite(audio.duration) && audio.duration > 0
      ? audio.duration
      : Math.max(0, currentTrack.value?.durationSeconds ?? 0)
    if (pendingResumePosition !== null) {
      const maximum = duration.value > 0 ? duration.value : pendingResumePosition
      audio.currentTime = Math.min(pendingResumePosition, maximum)
      currentTime.value = audio.currentTime
      pendingResumePosition = null
    }
  }

  function handlePlay() {
    isPlaying.value = true
    isLoading.value = false
    error.value = ''
    lastObservedPosition = audioElement.value?.currentTime ?? currentTime.value
    void persistPosition(Math.floor(currentTime.value), 'playing')
  }

  function handlePause() {
    if (!suppressPauseSave && isPlaying.value) updateTimeFromAudio()
    isPlaying.value = false
    isLoading.value = false
    const position = audioElement.value?.currentTime ?? currentTime.value
    if (!suppressPauseSave) void persistPosition(Math.floor(position), 'paused')
  }

  function handleAudioError() {
    const audio = audioElement.value
    if (!audio) return
    error.value = audioErrorMessage(audio)
    isPlaying.value = false
    isLoading.value = false
    void finishCurrentHistory('failed', Math.floor(audio.currentTime))
  }

  async function handleEnded() {
    if (isPlaying.value) updateTimeFromAudio()
    isPlaying.value = false
    if (repeatMode.value === 'one') {
      const audio = audioElement.value
      if (!audio) return
      audio.currentTime = 0
      currentTime.value = 0
      lastObservedPosition = 0
      void persistPosition(0, 'playing')
      void resume()
      return
    }
    await finishCurrentHistory('completed', Math.floor(currentTime.value))
    await next()
  }

  function handleSeeking() {
    lastObservedPosition = audioElement.value?.currentTime ?? currentTime.value
  }

  const audioListeners: Array<[keyof HTMLMediaElementEventMap, EventListener]> = [
    ['loadedmetadata', handleLoadedMetadata],
    ['durationchange', handleLoadedMetadata],
    ['timeupdate', updateTimeFromAudio],
    ['play', handlePlay],
    ['pause', handlePause],
    ['ended', handleEnded],
    ['seeking', handleSeeking],
    ['error', handleAudioError],
  ]

  async function persistPosition(
    position: number,
    status: PlaybackHistoryStatus = isPlaying.value ? 'playing' : 'paused',
    targetHistoryId = historyId,
  ): Promise<void> {
    const currentHistoryId = targetHistoryId
    if (currentHistoryId === null || !Number.isFinite(position)) return
    const safePosition = Math.max(0, Math.floor(position))
    const safeListenedSeconds = Math.max(0, Math.floor(listenedSeconds))
    if (
      currentHistoryId === lastSavedHistoryId
      && safePosition === lastSavedSecond
      && safeListenedSeconds === lastSavedListenedSecond
      && status === lastSavedStatus
    ) return
    if (currentHistoryId === historyId) {
      lastSavedHistoryId = currentHistoryId
      lastSavedSecond = safePosition
      lastSavedListenedSecond = safeListenedSeconds
      lastSavedStatus = status
    }
    historyWrites = historyWrites
      .then(async () => {
        await updatePlaybackHistory(currentHistoryId, safePosition, safeListenedSeconds, status)
        historyWarning.value = ''
      })
      .catch(() => {
        historyWarning.value = 'Playback history could not be saved.'
      })
    await historyWrites
    historyRevision.value += 1
  }

  async function finishCurrentHistory(status: PlaybackHistoryStatus, position: number): Promise<void> {
    const currentHistoryId = historyId
    if (currentHistoryId === null) return
    historyId = null
    await persistPosition(position, status, currentHistoryId)
  }

  async function beginPlayback(track: PlayerTrack): Promise<void> {
    const audio = audioElement.value
    if (!audio) {
      error.value = 'The persistent audio engine is not ready.'
      return
    }

    const requestId = ++transitionId
    if (isPlaying.value) updateTimeFromAudio()
    await finishCurrentHistory('stopped', Math.floor(audio.currentTime))
    if (requestId !== transitionId) return
    lastSavedSecond = -1
    lastSavedListenedSecond = -1
    lastSavedHistoryId = null
    lastSavedStatus = null
    listenedSeconds = 0
    lastObservedPosition = 0
    pendingResumePosition = null
    suppressPauseSave = true
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
    suppressPauseSave = false
    audio.currentTime = 0
    audio.volume = volume.value
    audio.muted = isMuted.value
    currentTime.value = 0
    duration.value = Math.max(0, track.durationSeconds ?? 0)
    error.value = ''
    historyWarning.value = ''
    isLoading.value = true

    try {
      const newHistoryId = await startPlaybackHistory(track.id)
      if (requestId !== transitionId) {
        await updatePlaybackHistory(newHistoryId, 0, 0, 'interrupted')
        return
      }
      historyId = newHistoryId
      lastSavedHistoryId = newHistoryId
      lastSavedSecond = 0
      lastSavedListenedSecond = 0
      lastSavedStatus = 'playing'
      const source = await resolveLocalTrack(track.id)
      if (requestId !== transitionId) {
        await updatePlaybackHistory(newHistoryId, Math.floor(currentTime.value), Math.floor(listenedSeconds), 'interrupted')
        return
      }
      audio.src = source.audioUrl
      audio.load()
      await audio.play()
      if (requestId !== transitionId) {
        await updatePlaybackHistory(newHistoryId, Math.floor(currentTime.value), Math.floor(listenedSeconds), 'interrupted')
        return
      }
      isLoading.value = false
      isPlaying.value = !audio.paused
      if (isPlaying.value) {
        try {
          await persistPosition(Math.floor(currentTime.value), 'playing')
        } catch {
          historyWarning.value = 'Playback history could not be saved.'
        }
      }
    } catch (playbackError) {
      if (requestId !== transitionId) return
      const message = playbackErrorMessage(playbackError, audio)
      suppressPauseSave = true
      audio.pause()
      audio.removeAttribute('src')
      audio.load()
      suppressPauseSave = false
      isPlaying.value = false
      isLoading.value = false
      error.value = message
      await finishCurrentHistory('failed', Math.floor(audio.currentTime))
    }
  }

  async function playQueue(tracks: PlayerTrack[], startIndex = 0): Promise<void> {
    if (tracks.length === 0 || startIndex < 0 || startIndex >= tracks.length) {
      error.value = 'There are no playable tracks in this queue.'
      return
    }
    queue.value = tracks.slice()
    queueIndex.value = startIndex
    await beginPlayback(queue.value[startIndex])
  }

  async function playTrack(track: PlayerTrack, tracks: PlayerTrack[] = [track]): Promise<void> {
    const startIndex = tracks.findIndex((queuedTrack) => queuedTrack.id === track.id)
    await playQueue(startIndex < 0 ? [track] : tracks, startIndex < 0 ? 0 : startIndex)
  }

  async function resume(): Promise<void> {
    const audio = audioElement.value
    if (!audio || !currentTrack.value) return
    if (!audio.src || error.value) {
      await beginPlayback(currentTrack.value)
      return
    }
    try {
      await audio.play()
      isPlaying.value = !audio.paused
      isLoading.value = false
      error.value = ''
      if (isPlaying.value && historyId === null && currentTrack.value) {
        historyId = await startPlaybackHistory(currentTrack.value.id)
        listenedSeconds = 0
        lastObservedPosition = audio.currentTime
        lastSavedHistoryId = historyId
        lastSavedSecond = Math.floor(audio.currentTime)
        lastSavedListenedSecond = 0
        lastSavedStatus = 'playing'
      }
      if (isPlaying.value) await persistPosition(Math.floor(audio.currentTime), 'playing')
    } catch (playbackError) {
      error.value = playbackErrorMessage(playbackError, audio)
      isPlaying.value = false
      isLoading.value = false
      await finishCurrentHistory('failed', Math.floor(audio.currentTime))
    }
  }

  function pause(): void {
    const audio = audioElement.value
    audio?.pause()
    isPlaying.value = false
    void persistPosition(Math.floor(audio?.currentTime ?? currentTime.value), 'paused')
  }

  function togglePlayback(): Promise<void> | void {
    const audio = audioElement.value
    if (audio) {
      isPlaying.value = !audio.paused && Boolean(audio.currentSrc || audio.src)
    }
    if (isPlaying.value) {
      pause()
      return
    }
    return resume()
  }

  function stop(): void {
    const audio = audioElement.value
    if (!audio) return
    transitionId += 1
    if (isPlaying.value) updateTimeFromAudio()
    void finishCurrentHistory('stopped', 0)
    suppressPauseSave = true
    audio.pause()
    audio.currentTime = 0
    suppressPauseSave = false
    currentTime.value = 0
    isPlaying.value = false
    isLoading.value = false
    pendingResumePosition = null
    lastObservedPosition = 0
    error.value = ''
  }

  function seek(position: number): void {
    const audio = audioElement.value
    if (!audio || !Number.isFinite(position)) return
    const maximum = duration.value > 0 ? duration.value : Number.POSITIVE_INFINITY
    const bounded = Math.max(0, Math.min(position, maximum))
    audio.currentTime = bounded
    currentTime.value = bounded
    lastObservedPosition = bounded
  }

  function setVolume(value: number): void {
    if (!Number.isFinite(value)) return
    volume.value = Math.max(0, Math.min(value, 1))
    const audio = audioElement.value
    if (audio) {
      audio.volume = volume.value
      if (volume.value > 0) {
        audio.muted = false
        isMuted.value = false
      }
    }
  }

  function toggleMute(): void {
    isMuted.value = !isMuted.value
    if (audioElement.value) audioElement.value.muted = isMuted.value
  }

  function toggleShuffle(): void {
    shuffle.value = !shuffle.value
  }

  function cycleRepeat(): void {
    repeatMode.value = repeatMode.value === 'off' ? 'all' : repeatMode.value === 'all' ? 'one' : 'off'
  }

  async function next(): Promise<void> {
    if (queue.value.length === 0 || queueIndex.value < 0) return
    let nextIndex = -1
    if (shuffle.value && queue.value.length > 1) {
      const choices = queue.value.map((_, index) => index).filter((index) => index !== queueIndex.value)
      nextIndex = choices[Math.floor(Math.random() * choices.length)]
    } else if (queueIndex.value < queue.value.length - 1) {
      nextIndex = queueIndex.value + 1
    } else if (repeatMode.value === 'all') {
      nextIndex = 0
    }
    if (nextIndex < 0) {
      stop()
      return
    }
    queueIndex.value = nextIndex
    await beginPlayback(queue.value[nextIndex])
  }

  async function previous(): Promise<void> {
    if (queue.value.length === 0 || queueIndex.value < 0) return
    if (currentTime.value > 3) {
      seek(0)
      return
    }
    const previousIndex = queueIndex.value > 0
      ? queueIndex.value - 1
      : repeatMode.value === 'all' ? queue.value.length - 1 : queueIndex.value
    queueIndex.value = previousIndex
    await beginPlayback(queue.value[previousIndex])
  }

  function toggleQueue(): void {
    queueOpen.value = !queueOpen.value
  }

  async function restoreLastSession(): Promise<void> {
    if (restoreRequested) return
    restoreRequested = true
    const audio = audioElement.value
    if (!audio) return
    try {
      const saved: PlaybackResumeSource | null = await getLastPlayback()
      if (!saved || audioElement.value !== audio || currentTrack.value) return
      queue.value = [saved.track]
      queueIndex.value = 0
      historyId = saved.historyId
      listenedSeconds = saved.listenedSeconds
      lastSavedSecond = saved.positionSeconds
      lastSavedListenedSecond = saved.listenedSeconds
      lastSavedHistoryId = saved.historyId
      lastSavedStatus = 'interrupted'
      lastObservedPosition = saved.positionSeconds
      duration.value = Math.max(0, saved.track.durationSeconds ?? 0)
      pendingResumePosition = Math.max(0, saved.positionSeconds)
      audio.src = saved.audioUrl
      audio.load()
      error.value = ''
      void persistPosition(saved.positionSeconds, 'paused')
    } catch (restoreError) {
      error.value = restoreError instanceof Error ? restoreError.message : 'The last local track could not be restored.'
    }
  }

  function handleKeyboardShortcut(event: KeyboardEvent): void {
    const target = event.target
    if (target instanceof HTMLElement && (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))) return
    if (event.key === ' ' && !event.repeat) {
      event.preventDefault()
      void togglePlayback()
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault()
      seek(currentTime.value - 5)
    } else if (event.key === 'ArrowRight') {
      event.preventDefault()
      seek(currentTime.value + 5)
    } else if (event.key === 'ArrowUp') {
      event.preventDefault()
      setVolume(volume.value + 0.05)
    } else if (event.key === 'ArrowDown') {
      event.preventDefault()
      setVolume(volume.value - 0.05)
    } else if (event.key.toLowerCase() === 'n') {
      void next()
    } else if (event.key.toLowerCase() === 'p') {
      void previous()
    }
  }

  function attachAudio(element: HTMLAudioElement) {
    if (audioElement.value && audioElement.value !== element) {
      throw new Error('Buitenzorg Player supports one persistent audio element.')
    }

    audioElement.value = element
    element.volume = volume.value
    element.muted = isMuted.value
    isPlaying.value = !element.paused && Boolean(element.currentSrc || element.src)
    isLoading.value = false
    currentTime.value = Number.isFinite(element.currentTime) ? Math.max(0, element.currentTime) : 0
    if (Number.isFinite(element.duration) && element.duration > 0) {
      duration.value = element.duration
    } else if (currentTrack.value) {
      duration.value = Math.max(0, currentTrack.value.durationSeconds ?? 0)
    }
    for (const [event, listener] of audioListeners) element.addEventListener(event, listener)
    if (typeof window !== 'undefined') window.addEventListener('keydown', handleKeyboardShortcut)
  }

  function detachAudio(element: HTMLAudioElement) {
    if (audioElement.value === element) {
      transitionId += 1
      if (isPlaying.value) updateTimeFromAudio()
      void persistPosition(Math.floor(element.currentTime), 'paused')
      for (const [event, listener] of audioListeners) element.removeEventListener(event, listener)
      if (typeof window !== 'undefined') window.removeEventListener('keydown', handleKeyboardShortcut)
      suppressPauseSave = true
      element.pause()
      element.removeAttribute('src')
      element.load()
      suppressPauseSave = false
      isPlaying.value = false
      isLoading.value = false
      audioElement.value = null
    }
  }

  return {
    audioElement,
    attachAudio,
    currentTime,
    currentTrack,
    detachAudio,
    duration,
    error,
    historyWarning,
    historyRevision,
    isLoading,
    isMuted,
    isPlaying,
    next,
    pause,
    playQueue,
    playTrack,
    previous,
    queue,
    queueIndex,
    queueOpen,
    repeatMode,
    restoreLastSession,
    resume,
    seek,
    setVolume,
    shuffle,
    stop,
    toggleMute,
    togglePlayback,
    toggleQueue,
    toggleShuffle,
    cycleRepeat,
    volume,
  }
})