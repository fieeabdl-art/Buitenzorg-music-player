import { invoke, isTauri } from '@tauri-apps/api/core'
import type { LibraryItem, PlaylistTrack } from '../types/library'
import type { PlaybackResumeSource, PlayerTrack, ResolvedTrackSource } from '../types/player'

interface BackendPlaybackSource {
  track: PlayerTrack
  audioUrl: string
}

interface BackendPlaybackResume extends BackendPlaybackSource {
  historyId: number
  positionSeconds: number
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

export function savePlaybackPosition(historyId: number, positionSeconds: number): Promise<void> {
  requireDesktop()
  return invoke<void>('save_playback_position', { historyId, positionSeconds })
}