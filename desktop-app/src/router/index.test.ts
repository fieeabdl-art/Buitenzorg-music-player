import { beforeEach, describe, expect, it } from 'vitest'
import { createMemoryHistory } from 'vue-router'
import { createAppRouter } from './index'

describe('application navigation', () => {
  let router: ReturnType<typeof createAppRouter>

  beforeEach(() => {
    router = createAppRouter(createMemoryHistory())
  })

  it('resolves every primary and secondary destination', async () => {
    const destinations = [
      ['home', '/'],
      ['discover', '/discover'],
      ['search', '/search'],
      ['library', '/library'],
      ['playlists', '/playlists'],
      ['downloads', '/downloads'],
      ['settings', '/settings'],
      ['profile', '/profile'],
      ['about', '/about'],
    ] as const

    for (const [name, path] of destinations) {
      await router.push({ name })
      expect(router.currentRoute.value.path).toBe(path)
    }
  })

  it('returns unknown paths to the Listen page instead of rendering a blank route', async () => {
    await router.push('/unknown-destination')

    expect(router.currentRoute.value.name).toBe('home')
  })
})