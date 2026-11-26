import { describe, expect, it } from 'vitest'
import { trackMetadataSummary } from './trackMetadata'

describe('track metadata summary', () => {
  it('formats only available metadata in a stable order', () => {
    expect(trackMetadataSummary({ genre: ' Ambient ', year: 2026, trackNumber: 4 })).toBe('Ambient · Year 2026 · Track 4')
  })

  it('does not invent missing or invalid values', () => {
    expect(trackMetadataSummary({ genre: '  ', year: 0, trackNumber: -1 })).toBe('')
    expect(trackMetadataSummary({})).toBe('')
  })
})