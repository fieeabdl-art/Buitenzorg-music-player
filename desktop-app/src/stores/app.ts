import { isTauri, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { APP_READY_EVENT } from '../services/events'
import type { DesktopAppInfo } from '../types/app'

export type DesktopBridgeState = 'idle' | 'loading' | 'ready' | 'browser' | 'error'

export const useAppStore = defineStore('app', () => {
  const bridgeState = ref<DesktopBridgeState>('idle')
  const appInfo = ref<DesktopAppInfo | null>(null)
  const bridgeError = ref('')
  let stopListening: UnlistenFn | undefined

  async function initialize() {
    if (bridgeState.value !== 'idle') {
      return
    }

    if (!isTauri()) {
      bridgeState.value = 'browser'
      return
    }

    bridgeState.value = 'loading'
    bridgeError.value = ''

    try {
      stopListening = await listen<DesktopAppInfo>(APP_READY_EVENT, (event) => {
        appInfo.value = event.payload
      })
      appInfo.value = await invoke<DesktopAppInfo>('get_app_info')
      bridgeState.value = 'ready'
    } catch {
      bridgeState.value = 'error'
      bridgeError.value = 'The desktop service did not respond. Retry the connection.'
    }
  }

  async function retry() {
    stopListening?.()
    stopListening = undefined
    bridgeState.value = 'idle'
    await initialize()
  }

  function dispose() {
    stopListening?.()
    stopListening = undefined
  }

  return { appInfo, bridgeError, bridgeState, dispose, initialize, retry }
})