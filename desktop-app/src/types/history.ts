import type { PlaybackTrack } from './player'

export type PlaybackHistoryStatus = 'loading' | 'playing' | 'paused' | 'completed' | 'stopped' | 'failed' | 'interrupted'

export interface PlaybackHistoryEntry {
  historyId: number
  playedAt: string
  positionSeconds: number
  listenedSeconds: number
  status: PlaybackHistoryStatus
  track: PlaybackTrack
}