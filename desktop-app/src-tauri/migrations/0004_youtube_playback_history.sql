CREATE TABLE youtube_playback_history (
  id INTEGER PRIMARY KEY,
  video_id TEXT NOT NULL CHECK (
    length(video_id) = 11 AND video_id NOT GLOB '*[^A-Za-z0-9_-]*'
  ),
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 500),
  artist TEXT NOT NULL CHECK (length(trim(artist)) BETWEEN 1 AND 500),
  artwork_url TEXT CHECK (
    artwork_url IS NULL OR (
      length(artwork_url) <= 2048 AND artwork_url GLOB 'https://i.ytimg.com/*'
    )
  ),
  duration_seconds INTEGER CHECK (duration_seconds IS NULL OR duration_seconds >= 0),
  played_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  position_seconds INTEGER NOT NULL DEFAULT 0 CHECK (position_seconds >= 0),
  listened_seconds INTEGER NOT NULL DEFAULT 0 CHECK (listened_seconds >= 0),
  status TEXT NOT NULL CHECK (
    status IN ('loading', 'playing', 'paused', 'completed', 'stopped', 'failed', 'interrupted')
  )
);

CREATE INDEX youtube_playback_history_played_at_idx
  ON youtube_playback_history(played_at DESC, id DESC);