import { invoke, isTauri } from '@tauri-apps/api/core'
import type {
  PlaybackResolution,
  ProviderAdapter,
  ProviderStatus,
  YouTubeSearchResponse,
  YouTubeVideo,
} from '../../types/providers'

function requireDesktop(): void {
  if (!isTauri()) {
    throw new Error('YouTube provider connections are available in the desktop app only.')
  }
}

class YouTubeAdapter implements ProviderAdapter<YouTubeVideo> {
  readonly id = 'youtube'

  getStatus(): Promise<ProviderStatus> {
    requireDesktop()
    return invoke<ProviderStatus>('get_youtube_provider_status')
  }

  search(query: string): Promise<YouTubeSearchResponse> {
    requireDesktop()
    return invoke<YouTubeSearchResponse>('search_youtube', { query })
  }

  resolvePlayback(result: YouTubeVideo): PlaybackResolution {
    return { kind: 'youtube-embed', videoId: result.videoId }
  }

  downloadEligibility(_result: YouTubeVideo): 'denied' {
    return 'denied'
  }


  saveApiKey(apiKey: string): Promise<ProviderStatus> {
    requireDesktop()
    return invoke<ProviderStatus>('save_youtube_api_key', { apiKey })
  }

  removeApiKey(): Promise<ProviderStatus> {
    requireDesktop()
    return invoke<ProviderStatus>('remove_youtube_api_key')
  }

  checkConnection(): Promise<ProviderStatus> {
    requireDesktop()
    return invoke<ProviderStatus>('check_youtube_connection')
  }
}

export const youtubeAdapter = new YouTubeAdapter()