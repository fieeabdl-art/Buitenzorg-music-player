export type LibraryCategory = 'all' | 'artists' | 'albums' | 'liked' | 'imported'

export interface LibraryItem {
  id: number
  kind: 'track' | 'artist' | 'album'
  title: string
  subtitle: string
  detail: string | null
  format: string | null
  sizeBytes: number | null
  durationSeconds: number | null
  addedAt: string | null
  availability: 'available' | 'missing' | 'unreadable' | null
  hasArtwork: boolean
  trackCount: number | null
  genre: string | null
  year: number | null
  trackNumber: number | null
  artworkUrl: string | null
}

export interface LibraryPage {
  category: LibraryCategory | 'search'
  offset: number
  totalCount: number
  storageBytes: number
  nextOffset: number | null
  items: LibraryItem[]
}

export interface ImportIssue {
  item: string
  message: string
}

export interface ImportReport {
  cancelled: boolean
  scanned: number
  imported: number
  duplicates: number
  skipped: number
  issues: ImportIssue[]
}

export interface LibraryStorageInfo {
  storageBytes: number
  trackCount: number
}

export interface PlaylistSummary {
  id: number
  name: string
  description: string
  trackCount: number
  createdAt: string
  updatedAt: string
}

export interface PlaylistTrack {
  id: number
  title: string
  artist: string
  album: string
  format: string
  durationSeconds: number | null
  position: number
  availability: 'available' | 'missing' | 'unreadable' | null
  genre: string | null
  year: number | null
  trackNumber: number | null
  artworkUrl: string | null
}