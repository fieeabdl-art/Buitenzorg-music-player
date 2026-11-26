export interface YouTubeIframePlayer {
  loadVideoById(videoId: string): void
  playVideo(): void
  pauseVideo(): void
  stopVideo(): void
  seekTo(seconds: number, allowSeekAhead?: boolean): void
  setVolume(volume: number): void
  mute(): void
  unMute(): void
  getCurrentTime(): number
  getDuration(): number
  getPlayerState(): number
  getVideoData(): { title?: string; author?: string; video_id?: string }
  getIframe(): HTMLIFrameElement
  destroy(): void
}

export interface YouTubePlayerEvent<T = void> {
  data: T
  target: YouTubeIframePlayer
}

export interface YouTubeIframeApi {
  Player: new (
    target: HTMLElement,
    options: {
      width: number
      height: number
      videoId: string
      playerVars: Record<string, number | string>
      events: {
        onReady: (event: YouTubePlayerEvent) => void
        onStateChange: (event: YouTubePlayerEvent<number>) => void
        onError: (event: YouTubePlayerEvent<number>) => void
        onAutoplayBlocked: (event: YouTubePlayerEvent) => void
      }
    },
  ) => YouTubeIframePlayer
}

declare global {
  interface Window {
    YT?: YouTubeIframeApi
    onYouTubeIframeAPIReady?: () => void
  }
}

let apiPromise: Promise<YouTubeIframeApi> | null = null

export function loadYouTubeIframeApi(): Promise<YouTubeIframeApi> {
  if (window.YT?.Player) return Promise.resolve(window.YT)
  if (apiPromise) return apiPromise

  apiPromise = new Promise<YouTubeIframeApi>((resolve, reject) => {
    const script = document.querySelector<HTMLScriptElement>('script[data-youtube-iframe-api]')
      ?? document.createElement('script')
    const previousCallback = window.onYouTubeIframeAPIReady
    let timeout: number | undefined
    let settled = false

    const finish = (error?: Error) => {
      if (settled) return
      settled = true
      if (timeout !== undefined) window.clearTimeout(timeout)
      if (error) {
        script.remove()
        apiPromise = null
        reject(error)
      } else if (window.YT?.Player) {
        resolve(window.YT)
      } else {
        apiPromise = null
        reject(new Error('YouTube did not initialize its IFrame Player API.'))
      }
    }

    window.onYouTubeIframeAPIReady = () => {
      previousCallback?.()
      finish()
    }
    script.addEventListener('error', () => finish(new Error('The YouTube IFrame Player API could not be loaded.')), { once: true })
    timeout = window.setTimeout(() => finish(new Error('YouTube took too long to load the player API.')), 15_000)

    if (!script.isConnected) {
      script.dataset.youtubeIframeApi = 'true'
      script.src = 'https://www.youtube.com/iframe_api'
      script.async = true
      document.head.append(script)
    }
  })

  return apiPromise
}