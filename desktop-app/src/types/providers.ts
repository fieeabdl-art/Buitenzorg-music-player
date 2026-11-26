export type ProviderState = 'available' | 'connected' | 'limited' | 'unavailable'

export type YouTubeSearchState =
  | 'ready'
  | 'empty'
  | 'unavailable'
  | 'quotaExceeded'
  | 'invalidKey'
  | 'timeout'
  | 'networkError'
  | 'providerError'

export type DownloadEligibility = 'allowed' | 'denied' | 'unknown'

export type PlaybackResolution =
  | { kind: 'local-html5'; trackId: number }
  | { kind: 'youtube-embed'; videoId: string }
  | { kind: 'external-link'; url: string }

export interface ProviderStatus {
  provider: string
  state: ProviderState
  hasApiKey: boolean
  checkedAt: string | null
  message: string
}

export interface YouTubeVideo {
  videoId: string
  title: string
  artist: string
  description: string
  thumbnailUrl: string | null
  durationSeconds: number | null
  sourceUrl: string
}

export interface YouTubeSearchResponse {
  query: string
  state: YouTubeSearchState
  items: YouTubeVideo[]
  message: string | null
  providerStatus: ProviderStatus
}

export interface ProviderAdapter<T> {
  readonly id: string
  getStatus(): Promise<ProviderStatus>
  search(query: string): Promise<YouTubeSearchResponse>
  resolvePlayback(result: T): PlaybackResolution
  downloadEligibility(result: T): DownloadEligibility
  openSource(result: T): Promise<void>
}