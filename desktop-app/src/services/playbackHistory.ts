import { invoke, isTauri } from '@tauri-apps/api/core'
import type { PlaybackHistoryEntry } from '../types/history'

function requireDesktop(): void {
  if (!isTauri()) {
    throw new Error('Playback history is available in the desktop app only.')
  }
}

export function getPlaybackHistory(): Promise<PlaybackHistoryEntry[]> {
  requireDesktop()
  return invoke<PlaybackHistoryEntry[]>('get_playback_history')
}

export function deletePlaybackHistory(historyId: number): Promise<void> {
  requireDesktop()
  return invoke<void>('delete_playback_history', { historyId })
}

export function clearPlaybackHistory(): Promise<number> {
  requireDesktop()
  return invoke<number>('clear_playback_history')
}