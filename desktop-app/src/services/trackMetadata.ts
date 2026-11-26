export interface TrackMetadataFields {
  genre?: string | null
  year?: number | null
  trackNumber?: number | null
}

export function trackMetadataSummary(track: TrackMetadataFields): string {
  const genre = track.genre?.trim()
  const year = Number.isInteger(track.year) && track.year! >= 1 && track.year! <= 9999 ? `Year ${track.year}` : null
  const trackNumber = Number.isInteger(track.trackNumber) && track.trackNumber! > 0 ? `Track ${track.trackNumber}` : null
  return [genre || null, year, trackNumber].filter((value): value is string => value !== null).join(' · ')
}