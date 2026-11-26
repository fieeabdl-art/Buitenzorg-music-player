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

export interface YouTubePlayerTrack extends PlayerTrack {
  source: 'youtube'
  videoId: string
}

export type PlaybackTrack = PlayerTrack | YouTubePlayerTrack

export interface YouTubePlayerController {
  loadVideoById(videoId: string): void
  playVideo(): void
  pauseVideo(): void
  stopVideo(): void
  seekTo(seconds: number): void
  setVolume(volume: number): void
  setMuted(muted: boolean): void
  getCurrentTime(): number
  getDuration(): number
  getPlayerState(): number
  getVideoData(): { title?: string; author?: string; video_id?: string }
  getIframe(): HTMLIFrameElement | null
  destroy(): void
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