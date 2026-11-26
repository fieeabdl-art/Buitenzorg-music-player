import { isTauri } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { youtubeAdapter } from '../services/providers/youtube'
import type { ProviderStatus, YouTubeSearchResponse, YouTubeVideo } from '../types/providers'

function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error && error.message) return error.message
  return 'The YouTube provider request failed. Try again.'
}

export const useProvidersStore = defineStore('providers', () => {
  const youtubeStatus = ref<ProviderStatus | null>(null)
  const youtubeResults = ref<YouTubeSearchResponse | null>(null)
  const selectedVideo = ref<YouTubeVideo | null>(null)
  const isLoadingStatus = ref(false)
  const isSavingKey = ref(false)
  const isCheckingConnection = ref(false)
  const isSearchingYoutube = ref(false)
  const actionError = ref('')
  let searchRequestId = 0

  async function loadYoutubeStatus(): Promise<void> {
    if (!isTauri()) {
      youtubeStatus.value = {
        provider: 'youtube',
        state: 'unavailable',
        hasApiKey: false,
        checkedAt: null,
        message: 'Open Buitenzorg Player desktop to configure the secure provider connection.',
      }
      return
    }
    isLoadingStatus.value = true
    try {
      youtubeStatus.value = await youtubeAdapter.getStatus()
      actionError.value = ''
    } catch (error) {
      youtubeStatus.value = {
        provider: 'youtube',
        state: 'unavailable',
        hasApiKey: false,
        checkedAt: null,
        message: errorMessage(error),
      }
    } finally {
      isLoadingStatus.value = false
    }
  }

  async function saveYoutubeApiKey(apiKey: string): Promise<boolean> {
    isSavingKey.value = true
    actionError.value = ''
    try {
      youtubeStatus.value = await youtubeAdapter.saveApiKey(apiKey)
      return true
    } catch (error) {
      actionError.value = errorMessage(error)
      return false
    } finally {
      isSavingKey.value = false
    }
  }

  async function removeYoutubeApiKey(): Promise<boolean> {
    isSavingKey.value = true
    actionError.value = ''
    try {
      youtubeStatus.value = await youtubeAdapter.removeApiKey()
      youtubeResults.value = null
      selectedVideo.value = null
      return true
    } catch (error) {
      actionError.value = errorMessage(error)
      return false
    } finally {
      isSavingKey.value = false
    }
  }

  async function checkYoutubeConnection(): Promise<boolean> {
    isCheckingConnection.value = true
    actionError.value = ''
    try {
      youtubeStatus.value = await youtubeAdapter.checkConnection()
      actionError.value = youtubeStatus.value.message
      return youtubeStatus.value.state === 'connected'
    } catch (error) {
      actionError.value = errorMessage(error)
      return false
    } finally {
      isCheckingConnection.value = false
    }
  }

  async function searchYoutube(query: string): Promise<void> {
    const normalizedQuery = query.trim()
    const requestId = ++searchRequestId
    selectedVideo.value = null
    actionError.value = ''
    if (!normalizedQuery) {
      youtubeResults.value = null
      isSearchingYoutube.value = false
      return
    }
    if (!isTauri()) {
      youtubeResults.value = {
        query: normalizedQuery,
        state: 'unavailable',
        items: [],
        message: 'YouTube search is available in the desktop app only.',
        providerStatus: youtubeStatus.value ?? {
          provider: 'youtube', state: 'unavailable', hasApiKey: false, checkedAt: null,
          message: 'Open the desktop app to configure YouTube search.',
        },
      }
      return
    }

    isSearchingYoutube.value = true
    youtubeResults.value = null
    try {
      const response = await youtubeAdapter.search(normalizedQuery)
      if (requestId !== searchRequestId) return
      youtubeResults.value = response
      youtubeStatus.value = response.providerStatus
    } catch (error) {
      if (requestId !== searchRequestId) return
      actionError.value = errorMessage(error)
      youtubeResults.value = {
        query: normalizedQuery,
        state: 'providerError',
        items: [],
        message: actionError.value,
        providerStatus: youtubeStatus.value ?? {
          provider: 'youtube', state: 'unavailable', hasApiKey: false, checkedAt: null,
          message: 'The provider could not be reached.',
        },
      }
    } finally {
      if (requestId === searchRequestId) isSearchingYoutube.value = false
    }
  }

  function selectYoutubeVideo(video: YouTubeVideo): void {
    const resolution = youtubeAdapter.resolvePlayback(video)
    selectedVideo.value = resolution.kind === 'youtube-embed' ? video : null
  }

  async function openYoutubeSource(video: YouTubeVideo): Promise<boolean> {
    try {
      await youtubeAdapter.openSource(video)
      return true
    } catch (error) {
      actionError.value = errorMessage(error)
      return false
    }
  }

  function clearSelectedVideo(): void {
    selectedVideo.value = null
  }

  return {
    actionError,
    checkYoutubeConnection,
    clearSelectedVideo,
    isCheckingConnection,
    isLoadingStatus,
    isSavingKey,
    isSearchingYoutube,
    loadYoutubeStatus,
    openYoutubeSource,
    removeYoutubeApiKey,
    saveYoutubeApiKey,
    searchYoutube,
    selectYoutubeVideo,
    selectedVideo,
    youtubeResults,
    youtubeStatus,
  }
})