import { defineStore } from 'pinia'
import { computed, ref, shallowRef } from 'vue'
import { getLastPlayback, resolveLocalTrack, startPlaybackHistory, startYouTubePlaybackHistory, updatePlaybackHistory } from '../services/audioPlayback'
import type { PlaybackHistoryStatus } from '../types/history'
import type { PlaybackResumeSource, PlaybackTrack, PlayerTrack, RepeatMode, YouTubePlayerController, YouTubePlayerTrack } from '../types/player'

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
  const youtubeController = shallowRef<YouTubePlayerController | null>(null)
  const queue = ref<PlaybackTrack[]>([])
  const queueIndex = ref(-1)
  const currentTrack = computed(() => queue.value[queueIndex.value] ?? null)
  const currentYouTubeTrack = computed(() => isYouTubeTrack(currentTrack.value) ? currentTrack.value : null)
  const isYouTubePlayback = computed(() => currentYouTubeTrack.value !== null)
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
  const playbackNotice = ref('')
  const youtubePlayerRevision = ref(0)
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
  let pendingYoutubeVideoId: string | null = null
  let restoreRequested = false
  let historyWrites = Promise.resolve()

  function isYouTubeTrack(track: PlaybackTrack | null | undefined): track is YouTubePlayerTrack {
    return Boolean(track && 'source' in track && track.source === 'youtube' && track.videoId)
  }

  function updateTimeFromAudio(force = false) {
    const audio = audioElement.value
    if (!audio || (!force && isYouTubeTrack(currentTrack.value))) return
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

  function updateTimeFromYouTube(track: YouTubePlayerTrack | null = isYouTubeTrack(currentTrack.value) ? currentTrack.value : null) {
    const controller = youtubeController.value
    if (!controller || !track) return
    const videoData = controller.getVideoData()
    if (videoData.video_id && videoData.video_id !== track.videoId) return
    const position = controller.getCurrentTime()
    const videoDuration = controller.getDuration()
    if (!Number.isFinite(position) || position < 0) return
    currentTime.value = position
    if (Number.isFinite(videoDuration) && videoDuration > 0) duration.value = videoDuration
    else duration.value = Math.max(0, track.durationSeconds ?? 0)
    if (historyId !== null && isPlaying.value && lastObservedPosition !== null && position >= lastObservedPosition) {
      listenedSeconds += position - lastObservedPosition
    }
    lastObservedPosition = position
    if (historyId !== null && Math.floor(position) - lastSavedSecond >= POSITION_SAVE_INTERVAL_SECONDS) {
      void persistPosition(Math.floor(position))
    }
  }

  function handleLoadedMetadata() {
    const audio = audioElement.value
    if (!audio || isYouTubeTrack(currentTrack.value)) return
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
    if (isYouTubeTrack(currentTrack.value)) return
    isPlaying.value = true
    isLoading.value = false
    error.value = ''
    lastObservedPosition = audioElement.value?.currentTime ?? currentTime.value
    void persistPosition(Math.floor(currentTime.value), 'playing')
  }

  function handlePause() {
    if (isYouTubeTrack(currentTrack.value)) return
    if (!suppressPauseSave && isPlaying.value) updateTimeFromAudio()
    isPlaying.value = false
    isLoading.value = false
    const position = audioElement.value?.currentTime ?? currentTime.value
    if (!suppressPauseSave) void persistPosition(Math.floor(position), 'paused')
  }

  function handleAudioError() {
    const audio = audioElement.value
    if (!audio || isYouTubeTrack(currentTrack.value)) return
    if (isPlaying.value) updateTimeFromAudio()
    error.value = audioErrorMessage(audio)
    isPlaying.value = false
    isLoading.value = false
    void finishCurrentHistory('failed', Math.floor(audio.currentTime))
  }

  async function handleEnded() {
    if (isYouTubeTrack(currentTrack.value)) return
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
    if (isYouTubeTrack(currentTrack.value)) return
    lastObservedPosition = audioElement.value?.currentTime ?? currentTime.value
  }

  const audioListeners: Array<[keyof HTMLMediaElementEventMap, EventListener]> = [
    ['loadedmetadata', handleLoadedMetadata],
    ['durationchange', handleLoadedMetadata],
    ['timeupdate', () => updateTimeFromAudio()],
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

  function resetPlaybackHistoryCounters(): void {
    lastSavedSecond = -1
    lastSavedListenedSecond = -1
    lastSavedHistoryId = null
    lastSavedStatus = null
    listenedSeconds = 0
    lastObservedPosition = 0
    pendingResumePosition = null
  }

  function beginYouTubeHistory(track: YouTubePlayerTrack, requestId: number): void {
    void startYouTubePlaybackHistory(track).then(async (newHistoryId) => {
      if (requestId !== transitionId) {
        await updatePlaybackHistory(newHistoryId, Math.floor(currentTime.value), 0, 'interrupted')
        return
      }
      historyId = newHistoryId
      lastSavedHistoryId = newHistoryId
      lastSavedSecond = Math.floor(currentTime.value)
      lastSavedListenedSecond = 0
      lastSavedStatus = 'loading'
      const status: PlaybackHistoryStatus = error.value ? 'failed' : isPlaying.value ? 'playing' : playbackNotice.value ? 'paused' : 'loading'
      await persistPosition(Math.floor(currentTime.value), status)
      if (status === 'failed' && historyId === newHistoryId) historyId = null
    }).catch(() => {
      historyWarning.value = 'YouTube playback history could not be saved.'
    })
  }

  function beginYouTubePlayback(track: YouTubePlayerTrack, previousTrack: PlaybackTrack | null): void {
    const requestId = ++transitionId
    if (isPlaying.value) {
      if (isYouTubeTrack(previousTrack)) updateTimeFromYouTube(previousTrack)
      else updateTimeFromAudio(true)
    }
    const previousPosition = isYouTubeTrack(previousTrack)
      ? Math.floor(currentTime.value)
      : Math.floor(audioElement.value?.currentTime ?? currentTime.value)
    void finishCurrentHistory('stopped', previousPosition)
    resetPlaybackHistoryCounters()
    const audio = audioElement.value
    if (audio) {
      suppressPauseSave = true
      audio.pause()
      audio.removeAttribute('src')
      audio.load()
      audio.currentTime = 0
      audio.volume = volume.value
      audio.muted = isMuted.value
      suppressPauseSave = false
    }
    youtubeController.value?.stopVideo()
    currentTime.value = 0
    duration.value = Math.max(0, track.durationSeconds ?? 0)
    isPlaying.value = false
    isLoading.value = true
    error.value = ''
    historyWarning.value = ''
    playbackNotice.value = ''
    pendingYoutubeVideoId = track.videoId
    const controller = youtubeController.value
    if (controller) {
      controller.setVolume(Math.round(volume.value * 100))
      controller.setMuted(isMuted.value)
      controller.loadVideoById(track.videoId)
      pendingYoutubeVideoId = null
    }
    beginYouTubeHistory(track, requestId)
  }

  async function beginPlayback(track: PlaybackTrack, previousTrack: PlaybackTrack | null): Promise<void> {
    if (isYouTubeTrack(track)) {
      beginYouTubePlayback(track, previousTrack)
      return
    }
    await beginLocalPlayback(track, previousTrack)
  }

  async function beginLocalPlayback(track: PlayerTrack, previousTrack: PlaybackTrack | null): Promise<void> {
    const audio = audioElement.value
    if (!audio) {
      error.value = 'The persistent audio engine is not ready.'
      return
    }

    const requestId = ++transitionId
    if (isPlaying.value) {
      if (isYouTubeTrack(previousTrack)) updateTimeFromYouTube(previousTrack)
      else updateTimeFromAudio()
    }
    const previousPosition = isYouTubeTrack(previousTrack)
      ? Math.floor(currentTime.value)
      : Math.floor(audio.currentTime)
    await finishCurrentHistory('stopped', previousPosition)
    if (requestId !== transitionId) return
    resetPlaybackHistoryCounters()
    suppressPauseSave = true
    audio.pause()
    audio.removeAttribute('src')
    audio.load()
    suppressPauseSave = false
    audio.currentTime = 0
    audio.volume = volume.value
    audio.muted = isMuted.value
    youtubeController.value?.stopVideo()
    pendingYoutubeVideoId = null
    currentTime.value = 0
    duration.value = Math.max(0, track.durationSeconds ?? 0)
    error.value = ''
    playbackNotice.value = ''
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

  function attachYouTubeController(controller: YouTubePlayerController): void {
    youtubeController.value = controller
    controller.setVolume(Math.round(volume.value * 100))
    controller.setMuted(isMuted.value)
    const track = currentTrack.value
    const videoId = pendingYoutubeVideoId ?? (isYouTubeTrack(track) ? track.videoId : null)
    if (videoId) {
      pendingYoutubeVideoId = null
      controller.loadVideoById(videoId)
    }
  }

  function detachYouTubeController(controller: YouTubePlayerController): void {
    if (youtubeController.value === controller) youtubeController.value = null
  }

  function syncYouTubeMetadata(): void {
    const track = currentTrack.value
    const controller = youtubeController.value
    if (!isYouTubeTrack(track) || !controller) return
    const video = controller.getVideoData()
    if (video.video_id && video.video_id !== track.videoId) return
    const title = video.title?.trim()
    const artist = video.author?.trim()
    if (!title && !artist) return
    queue.value[queueIndex.value] = {
      ...track,
      title: title || track.title,
      artist: artist || track.artist,
    }
  }

  function handleYouTubeReady(controller: YouTubePlayerController): void {
    attachYouTubeController(controller)
    syncYouTubeMetadata()
  }

  function handleYouTubeStateChange(state: number): void {
    const track = currentTrack.value
    const controller = youtubeController.value
    if (!isYouTubeTrack(track) || !controller) return
    const video = controller.getVideoData()
    if (video.video_id && video.video_id !== track.videoId) return

    if (state === 1) {
      isPlaying.value = true
      isLoading.value = false
      playbackNotice.value = ''
      error.value = ''
      syncYouTubeMetadata()
      updateTimeFromYouTube(track)
      void persistPosition(Math.floor(currentTime.value), 'playing')
    } else if (state === 2) {
      updateTimeFromYouTube(track)
      isPlaying.value = false
      isLoading.value = false
      playbackNotice.value = ''
      void persistPosition(Math.floor(currentTime.value), 'paused')
    } else if (state === 3) {
      updateTimeFromYouTube(track)
      isPlaying.value = false
      isLoading.value = true
      playbackNotice.value = 'Buffering video from YouTube.'
      void persistPosition(Math.floor(currentTime.value), 'loading')
    } else if (state === 5) {
      isPlaying.value = false
      isLoading.value = false
      playbackNotice.value = 'Video is ready. Press Play to start.'
      void persistPosition(Math.floor(currentTime.value), 'paused')
    } else if (state === 0) {
      if (!isPlaying.value && isLoading.value) return
      updateTimeFromYouTube(track)
      isPlaying.value = false
      isLoading.value = false
      if (repeatMode.value === 'one') {
        controller.seekTo(0)
        currentTime.value = 0
        lastObservedPosition = 0
        void persistPosition(0, 'playing')
        controller.playVideo()
        return
      }
      void finishCurrentHistory('completed', Math.floor(currentTime.value))
      void next()
    }
  }

  function handleYouTubeError(code: number): void {
    const track = currentTrack.value
    if (!isYouTubeTrack(track)) return
    updateTimeFromYouTube(track)
    const messages: Record<number, string> = {
      2: 'YouTube rejected this video ID. Choose another result.',
      5: 'YouTube could not play this video in the embedded player.',
      100: 'This video was removed, made private, or is unavailable.',
      101: 'The video owner does not allow playback in embedded players.',
      150: 'The video owner does not allow playback in embedded players.',
      153: 'YouTube could not identify this app for embedded playback. Check the app connection and try again.',
    }
    isPlaying.value = false
    isLoading.value = false
    playbackNotice.value = ''
    error.value = messages[code] ?? `YouTube playback failed (error ${code}).`
    void finishCurrentHistory('failed', Math.floor(currentTime.value))
  }

  function handleYouTubeAutoplayBlocked(): void {
    if (!isYouTubeTrack(currentTrack.value)) return
    isPlaying.value = false
    isLoading.value = false
    playbackNotice.value = 'Automatic playback was blocked. Press Play in the YouTube player or use the app controls.'
    void persistPosition(Math.floor(currentTime.value), 'paused')
  }

  function handleYouTubeLoadFailure(): void {
    if (!isYouTubeTrack(currentTrack.value)) return
    isPlaying.value = false
    isLoading.value = false
    error.value = 'The YouTube player could not load. Check the internet connection and try again.'
    void finishCurrentHistory('failed', Math.floor(currentTime.value))
  }

  function updateYouTubeTime(): void {
    updateTimeFromYouTube()
  }

  async function playQueue(tracks: PlaybackTrack[], startIndex = 0): Promise<void> {
    if (tracks.length === 0 || startIndex < 0 || startIndex >= tracks.length) {
      error.value = 'There are no playable tracks in this queue.'
      return
    }
    const previousTrack = currentTrack.value
    queue.value = tracks.slice()
    queueIndex.value = startIndex
    await beginPlayback(queue.value[startIndex], previousTrack)
  }

  async function playTrack(track: PlaybackTrack, tracks: PlaybackTrack[] = [track]): Promise<void> {
    const startIndex = tracks.findIndex((queuedTrack) => isYouTubeTrack(track) && isYouTubeTrack(queuedTrack)
      ? queuedTrack.videoId === track.videoId
      : !isYouTubeTrack(track) && !isYouTubeTrack(queuedTrack) && queuedTrack.id === track.id)
    await playQueue(startIndex < 0 ? [track] : tracks, startIndex < 0 ? 0 : startIndex)
  }

  async function resume(): Promise<void> {
    const track = currentTrack.value
    if (!track) return
    if (isYouTubeTrack(track)) {
      if (error.value) {
        const reloadPlayer = youtubeController.value === null
        beginYouTubePlayback(track, track)
        if (reloadPlayer) youtubePlayerRevision.value += 1
        return
      }
      if (historyId === null) {
        beginYouTubePlayback(track, track)
        return
      }
      const controller = youtubeController.value
      if (!controller) {
        pendingYoutubeVideoId = track.videoId
        isLoading.value = true
        playbackNotice.value = 'Loading YouTube player.'
        return
      }
      playbackNotice.value = ''
      isLoading.value = true
      controller.playVideo()
      return
    }
    const audio = audioElement.value
    if (!audio) return
    if (!audio.src || error.value) {
      await beginPlayback(track, track)
      return
    }
    try {
      await audio.play()
      isPlaying.value = !audio.paused
      isLoading.value = false
      error.value = ''
      if (isPlaying.value && historyId === null) {
        historyId = await startPlaybackHistory(track.id)
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
    if (isYouTubeTrack(currentTrack.value)) {
      updateTimeFromYouTube()
      youtubeController.value?.pauseVideo()
      isPlaying.value = false
      isLoading.value = false
      playbackNotice.value = ''
      void persistPosition(Math.floor(currentTime.value), 'paused')
      return
    }
    const audio = audioElement.value
    if (isPlaying.value) updateTimeFromAudio()
    audio?.pause()
    isPlaying.value = false
    void persistPosition(Math.floor(audio?.currentTime ?? currentTime.value), 'paused')
  }

  function togglePlayback(): Promise<void> | void {
    if (isYouTubeTrack(currentTrack.value)) {
      if (isPlaying.value || isLoading.value) pause()
      else return resume()
      return
    }
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
    const track = currentTrack.value
    if (!track) return
    transitionId += 1
    if (isPlaying.value) {
      if (isYouTubeTrack(track)) updateTimeFromYouTube(track)
      else updateTimeFromAudio()
    }
    void finishCurrentHistory('stopped', 0)
    if (isYouTubeTrack(track)) {
      youtubeController.value?.stopVideo()
      pendingYoutubeVideoId = null
    } else if (audio) {
      suppressPauseSave = true
      audio.pause()
      audio.currentTime = 0
      suppressPauseSave = false
    }
    currentTime.value = 0
    isPlaying.value = false
    isLoading.value = false
    pendingResumePosition = null
    lastObservedPosition = 0
    error.value = ''
  }

  function seek(position: number): void {
    const track = currentTrack.value
    if (!track || !Number.isFinite(position)) return
    const maximum = duration.value > 0 ? duration.value : Number.POSITIVE_INFINITY
    const bounded = Math.max(0, Math.min(position, maximum))
    if (isYouTubeTrack(track)) {
      const controller = youtubeController.value
      if (!controller) return
      controller.seekTo(bounded)
    } else {
      const audio = audioElement.value
      if (!audio) return
      audio.currentTime = bounded
    }
    currentTime.value = bounded
    lastObservedPosition = bounded
    if (isYouTubeTrack(track)) void persistPosition(Math.floor(bounded))
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
    if (isYouTubeTrack(currentTrack.value)) {
      youtubeController.value?.setVolume(Math.round(volume.value * 100))
      if (volume.value > 0) youtubeController.value?.setMuted(false)
    }
  }

  function toggleMute(): void {
    isMuted.value = !isMuted.value
    if (audioElement.value) audioElement.value.muted = isMuted.value
    if (isYouTubeTrack(currentTrack.value)) youtubeController.value?.setMuted(isMuted.value)
  }

  function toggleShuffle(): void {
    shuffle.value = !shuffle.value
  }

  function cycleRepeat(): void {
    repeatMode.value = repeatMode.value === 'off' ? 'all' : repeatMode.value === 'all' ? 'one' : 'off'
  }

  async function next(): Promise<void> {
    if (queue.value.length === 0 || queueIndex.value < 0) return
    const previousTrack = currentTrack.value
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
    await beginPlayback(queue.value[nextIndex], previousTrack)
  }

  async function previous(): Promise<void> {
    if (queue.value.length === 0 || queueIndex.value < 0) return
    if (currentTime.value > 3) {
      seek(0)
      return
    }
    const previousTrack = currentTrack.value
    const previousIndex = queueIndex.value > 0
      ? queueIndex.value - 1
      : repeatMode.value === 'all' ? queue.value.length - 1 : queueIndex.value
    queueIndex.value = previousIndex
    await beginPlayback(queue.value[previousIndex], previousTrack)
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
    isPlaying.value = !isYouTubeTrack(currentTrack.value) && !element.paused && Boolean(element.currentSrc || element.src)
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
      if (isYouTubeTrack(currentTrack.value)) {
        if (isPlaying.value) updateTimeFromYouTube()
        void finishCurrentHistory('interrupted', Math.floor(currentTime.value))
      } else {
        if (isPlaying.value) updateTimeFromAudio()
        void persistPosition(Math.floor(element.currentTime), 'paused')
      }
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
    attachYouTubeController,
    currentTime,
    currentYouTubeTrack,
    currentTrack,
    detachAudio,
    duration,
    error,
    historyWarning,
    historyRevision,
    handleYouTubeAutoplayBlocked,
    handleYouTubeError,
    handleYouTubeLoadFailure,
    handleYouTubeReady,
    handleYouTubeStateChange,
    isLoading,
    isMuted,
    isPlaying,
    isYouTubePlayback,
    next,
    pause,
    playQueue,
    playTrack,
    playbackNotice,
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
    detachYouTubeController,
    updateYouTubeTime,
    youtubePlayerRevision,
    volume,
  }
})