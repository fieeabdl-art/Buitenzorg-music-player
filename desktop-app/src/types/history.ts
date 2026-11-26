import type { PlayerTrack } from './player'

export type PlaybackHistoryStatus = 'playing' | 'paused' | 'completed' | 'stopped' | 'failed' | 'interrupted'

export interface PlaybackHistoryEntry {
  historyId: number
  playedAt: string
  positionSeconds: number
  listenedSeconds: number
  status: PlaybackHistoryStatus
  track: PlayerTrack
}