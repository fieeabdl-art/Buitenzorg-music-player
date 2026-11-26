export interface PlayerTrack {
  id: number
  title: string
  artist: string
  album: string
  format: string
  durationSeconds: number | null
  availability?: 'available' | 'missing' | 'unreadable' | null
  genre?: string | null
  year?: number | null
  trackNumber?: number | null
  artworkUrl?: string | null
}

export type RepeatMode = 'off' | 'all' | 'one'

export interface ResolvedTrackSource {
  track: PlayerTrack
  audioUrl: string
}

export interface PlaybackResumeSource extends ResolvedTrackSource {
  historyId: number
  positionSeconds: number
  listenedSeconds: number
}