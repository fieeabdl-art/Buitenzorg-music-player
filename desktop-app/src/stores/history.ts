import { defineStore } from 'pinia'
import { ref } from 'vue'
import { clearPlaybackHistory, deletePlaybackHistory, getPlaybackHistory } from '../services/playbackHistory'
import type { PlaybackHistoryEntry } from '../types/history'

export type HistoryLoadState = 'idle' | 'loading' | 'ready' | 'empty' | 'unavailable' | 'error'

function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return 'The playback history request failed.'
}

export const useHistoryStore = defineStore('history', () => {
  const entries = ref<PlaybackHistoryEntry[]>([])
  const status = ref<HistoryLoadState>('idle')
  const error = ref('')
  const feedback = ref('')
  let requestId = 0

  async function load(): Promise<void> {
    const currentRequestId = ++requestId
    status.value = 'loading'
    error.value = ''
    try {
      const result = await getPlaybackHistory()
      if (currentRequestId !== requestId) return
      entries.value = result
      status.value = result.length > 0 ? 'ready' : 'empty'
    } catch (requestError) {
      if (currentRequestId !== requestId) return
      status.value = 'error'
      error.value = errorMessage(requestError)
    }
  }

  async function removeEntry(historyId: number): Promise<void> {
    error.value = ''
    feedback.value = ''
    try {
      await deletePlaybackHistory(historyId)
      entries.value = entries.value.filter((entry) => entry.historyId !== historyId)
      status.value = entries.value.length > 0 ? 'ready' : 'empty'
      feedback.value = 'Playback history entry removed.'
    } catch (requestError) {
      error.value = errorMessage(requestError)
    }
  }

  async function clearEntries(): Promise<void> {
    error.value = ''
    feedback.value = ''
    try {
      const deletedCount = await clearPlaybackHistory()
      await load()
      feedback.value = deletedCount > 0
        ? `${deletedCount} history ${deletedCount === 1 ? 'entry' : 'entries'} removed. Active playback is kept.`
        : 'There are no inactive history entries to clear.'
    } catch (requestError) {
      error.value = errorMessage(requestError)
    }
  }

  function markUnavailable(): void {
    requestId += 1
    entries.value = []
    status.value = 'unavailable'
    error.value = ''
  }

  return { clearEntries, entries, error, feedback, load, markUnavailable, removeEntry, status }
})