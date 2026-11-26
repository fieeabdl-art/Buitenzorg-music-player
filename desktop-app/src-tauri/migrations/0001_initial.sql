CREATE TABLE local_profile (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  display_name TEXT NOT NULL CHECK (length(trim(display_name)) > 0),
  username TEXT,
  avatar_path TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE artists (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  name_key TEXT NOT NULL UNIQUE,
  artwork_name TEXT
);

CREATE TABLE albums (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  title_key TEXT NOT NULL,
  artist_id INTEGER REFERENCES artists(id) ON DELETE SET NULL,
  artwork_name TEXT,
  UNIQUE (artist_id, title_key)
);

CREATE TABLE tracks (
  id INTEGER PRIMARY KEY,
  title TEXT NOT NULL,
  artist_id INTEGER REFERENCES artists(id) ON DELETE SET NULL,
  album_id INTEGER REFERENCES albums(id) ON DELETE SET NULL,
  source_name TEXT NOT NULL,
  stored_name TEXT NOT NULL UNIQUE,
  content_sha256 TEXT NOT NULL UNIQUE CHECK (
    length(content_sha256) = 64 AND content_sha256 NOT GLOB '*[^0-9a-f]*'
  ),
  format TEXT NOT NULL,
  duration_seconds INTEGER,
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  artwork_name TEXT,
  added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX tracks_added_at_idx ON tracks(added_at DESC, id DESC);
CREATE INDEX tracks_artist_id_idx ON tracks(artist_id);
CREATE INDEX tracks_album_id_idx ON tracks(album_id);

CREATE TABLE playlists (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 80),
  description TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 500),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE playlist_tracks (
  playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position >= 0),
  added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (playlist_id, track_id),
  UNIQUE (playlist_id, position)
);

CREATE TABLE liked_tracks (
  track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
  liked_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE downloads (
  id INTEGER PRIMARY KEY,
  track_id INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
  provider TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('queued', 'downloading', 'completed', 'failed', 'cancelled')),
  progress REAL NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
  output_name TEXT,
  size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE playback_history (
  id INTEGER PRIMARY KEY,
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  played_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  position_seconds INTEGER NOT NULL DEFAULT 0 CHECK (position_seconds >= 0)
);

CREATE INDEX playback_history_played_at_idx ON playback_history(played_at DESC);

CREATE TABLE provider_status (
  provider TEXT PRIMARY KEY,
  status TEXT NOT NULL CHECK (status IN ('available', 'connected', 'limited', 'unavailable')),
  checked_at TEXT
);

CREATE TABLE application_settings (
  key TEXT PRIMARY KEY CHECK (
    key IN ('theme', 'storage_limit_bytes', 'library_sort_order', 'legacy_import_checked_at')
  ),
  value TEXT NOT NULL
);