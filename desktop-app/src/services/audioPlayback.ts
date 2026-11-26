import { invoke, isTauri } from '@tauri-apps/api/core'
import type { LibraryItem, PlaylistTrack } from '../types/library'
import type { PlaybackHistoryStatus } from '../types/history'
import type { PlaybackResumeSource, PlayerTrack, ResolvedTrackSource, YouTubePlayerTrack } from '../types/player'
import type { YouTubeVideo } from '../types/providers'

interface BackendPlaybackSource {
  track: PlayerTrack
  audioUrl: string
}

interface BackendPlaybackResume extends BackendPlaybackSource {
  historyId: number
  positionSeconds: number
  listenedSeconds: number
}

function requireDesktop(): void {
  if (!isTauri()) {
    throw new Error('Local audio playback is available in the desktop app only.')
  }
}

export function toPlayerTrack(item: LibraryItem | PlaylistTrack): PlayerTrack {
  if ('artist' in item) {
    return {
      id: item.id,
      title: item.title,
      artist: item.artist,
      album: item.album,
      format: item.format,
      durationSeconds: item.durationSeconds,
      availability: item.availability,
      genre: item.genre,
      year: item.year,
      trackNumber: item.trackNumber,
      artworkUrl: item.artworkUrl,
    }
  }
  return {
    id: item.id,
    title: item.title,
    artist: item.subtitle,
    album: item.detail ?? 'Local files',
    format: item.format ?? 'unknown',
    durationSeconds: item.durationSeconds,
    availability: item.availability,
    genre: item.genre,
    year: item.year,
    trackNumber: item.trackNumber,
    artworkUrl: item.artworkUrl,
  }
}

export function toYouTubePlayerTrack(video: YouTubeVideo): YouTubePlayerTrack {
  let hash = 2166136261
  for (const character of video.videoId) {
    hash = Math.imul(hash ^ character.charCodeAt(0), 16777619)
  }
  return {
    id: -Math.max(1, Math.abs(hash)),
    title: video.title,
    artist: video.artist || 'YouTube channel',
    album: 'YouTube',
    format: 'youtube',
    durationSeconds: video.durationSeconds,
    availability: 'available',
    artworkUrl: video.thumbnailUrl,
    source: 'youtube',
    videoId: video.videoId,
  }
}

export async function resolveLocalTrack(trackId: number): Promise<ResolvedTrackSource> {
  requireDesktop()
  return invoke<BackendPlaybackSource>('resolve_track_for_playback', { trackId })
}

export async function getLastPlayback(): Promise<PlaybackResumeSource | null> {
  requireDesktop()
  return invoke<BackendPlaybackResume | null>('get_last_playback')
}

export function startPlaybackHistory(trackId: number): Promise<number> {
  requireDesktop()
  return invoke<number>('start_playback_history', { trackId })
}

export function startYouTubePlaybackHistory(track: YouTubePlayerTrack): Promise<number> {
  requireDesktop()
  return invoke<number>('start_youtube_playback_history', {
    videoId: track.videoId,
    title: track.title,
    artist: track.artist,
    artworkUrl: track.artworkUrl ?? null,
    durationSeconds: track.durationSeconds,
  })
}

export function updatePlaybackHistory(
  historyId: number,
  positionSeconds: number,
  listenedSeconds: number,
  status: PlaybackHistoryStatus,
): Promise<void> {
  requireDesktop()
  return invoke<void>('update_playback_history', { historyId, positionSeconds, listenedSeconds, status })
}