use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::Serialize;

const SCHEMA_VERSION: i64 = 1;
const PAGE_SIZE: usize = 50;
const INITIAL_SCHEMA: &str = include_str!("../migrations/0001_initial.sql");

#[derive(Clone)]
pub struct Database {
  connection: Arc<Mutex<Connection>>,
}

#[derive(Clone, Debug)]
pub struct NewTrack {
  pub title: String,
  pub artist: String,
  pub album: String,
  pub source_name: String,
  pub stored_name: String,
  pub content_sha256: String,
  pub format: String,
  pub duration_seconds: Option<i64>,
  pub size_bytes: i64,
  pub artwork_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExistingTrack {
  pub stored_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
  pub id: i64,
  pub kind: String,
  pub title: String,
  pub subtitle: String,
  pub detail: Option<String>,
  pub format: Option<String>,
  pub size_bytes: Option<i64>,
  pub duration_seconds: Option<i64>,
  pub added_at: Option<String>,
  pub availability: Option<String>,
  pub has_artwork: bool,
  pub track_count: Option<i64>,
  #[serde(skip_serializing)]
  pub(crate) stored_name: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
  pub category: String,
  pub offset: usize,
  pub total_count: usize,
  pub storage_bytes: u64,
  pub next_offset: Option<usize>,
  pub items: Vec<LibraryItem>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
  pub id: i64,
  pub name: String,
  pub description: String,
  pub track_count: i64,
  pub created_at: String,
  pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistTrack {
  pub id: i64,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub format: String,
  pub duration_seconds: Option<i64>,
  pub position: i64,
  pub availability: Option<String>,
  #[serde(skip_serializing)]
  pub(crate) stored_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackTrack {
  pub id: i64,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub format: String,
  pub duration_seconds: Option<i64>,
  #[serde(skip_serializing)]
  pub(crate) stored_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackHistoryEntry {
  pub history_id: i64,
  pub position_seconds: i64,
  pub track: PlaybackTrack,
}

#[derive(Clone, Debug)]
pub struct StoredProviderStatus {
  pub state: String,
  pub checked_at: Option<String>,
}

impl Database {
  pub fn open(path: &Path) -> Result<Self, String> {
    let mut connection = Connection::open(path).map_err(|error| error.to_string())?;
    Self::configure_and_migrate(&mut connection)?;
    Ok(Self { connection: Arc::new(Mutex::new(connection)) })
  }

  #[cfg(test)]
  fn open_in_memory() -> Result<Self, String> {
    let mut connection = Connection::open_in_memory().map_err(|error| error.to_string())?;
    Self::configure_and_migrate(&mut connection)?;
    Ok(Self { connection: Arc::new(Mutex::new(connection)) })
  }

  fn configure_and_migrate(connection: &mut Connection) -> Result<(), String> {
    connection
      .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")
      .map_err(|error| error.to_string())?;

    let mut current_version: i64 = connection
      .pragma_query_value(None, "user_version", |row| row.get(0))
      .map_err(|error| error.to_string())?;

    if current_version > SCHEMA_VERSION {
      return Err(format!(
        "Database schema version {current_version} is newer than this application supports ({SCHEMA_VERSION})."
      ));
    }

    if current_version < 1 {
      let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
      transaction.execute_batch(INITIAL_SCHEMA).map_err(|error| error.to_string())?;
      transaction
        .pragma_update(None, "user_version", 1_i64)
        .map_err(|error| error.to_string())?;
      transaction.commit().map_err(|error| error.to_string())?;
      current_version = 1;
    }

    if current_version != SCHEMA_VERSION {
      return Err(format!("Database migration stopped at unexpected schema version {current_version}."));
    }

    Ok(())
  }

  fn lock(&self) -> Result<MutexGuard<'_, Connection>, String> {
    self.connection.lock().map_err(|_| "The local database is unavailable.".to_owned())
  }

  #[cfg(test)]
  pub fn schema_version(&self) -> Result<i64, String> {
    self
      .lock()?
      .pragma_query_value(None, "user_version", |row| row.get(0))
      .map_err(|error| error.to_string())
  }

  pub fn set_provider_status(&self, provider: &str, state: &str) -> Result<(), String> {
    if provider != "youtube" || !["available", "connected", "limited", "unavailable"].contains(&state) {
      return Err("The provider status is invalid.".to_owned());
    }
    self
      .lock()?
      .execute(
        "INSERT INTO provider_status (provider, status, checked_at) VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ON CONFLICT(provider) DO UPDATE SET status = excluded.status, checked_at = excluded.checked_at",
        params![provider, state],
      )
      .map_err(|error| error.to_string())?;
    Ok(())
  }

  pub fn get_provider_status(&self, provider: &str) -> Result<Option<StoredProviderStatus>, String> {
    if provider != "youtube" {
      return Err("The provider is not supported.".to_owned());
    }
    self
      .lock()?
      .query_row(
        "SELECT status, checked_at FROM provider_status WHERE provider = ?1",
        [provider],
        |row| Ok(StoredProviderStatus { state: row.get(0)?, checked_at: row.get(1)? }),
      )
      .optional()
      .map_err(|error| error.to_string())
  }

  pub fn find_track_by_hash(&self, content_sha256: &str) -> Result<Option<ExistingTrack>, String> {
    self
      .lock()?
      .query_row(
        "SELECT stored_name FROM tracks WHERE content_sha256 = ?1",
        [content_sha256],
        |row| Ok(ExistingTrack { stored_name: row.get(0)? }),
      )
      .optional()
      .map_err(|error| error.to_string())
  }

  pub fn playback_track(&self, track_id: i64) -> Result<Option<PlaybackTrack>, String> {
    self
      .lock()?
      .query_row(
        "SELECT t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, t.stored_name FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id WHERE t.id = ?1",
        [track_id],
        |row| {
          Ok(PlaybackTrack {
            id: row.get(0)?,
            title: row.get(1)?,
            artist: row.get(2)?,
            album: row.get(3)?,
            format: row.get(4)?,
            duration_seconds: row.get(5)?,
            stored_name: row.get(6)?,
          })
        },
      )
      .optional()
      .map_err(|error| error.to_string())
  }

  pub fn start_playback_history(&self, track_id: i64) -> Result<i64, String> {
    let connection = self.lock()?;
    let inserted = connection
      .execute(
        "INSERT INTO playback_history (track_id, position_seconds) SELECT id, 0 FROM tracks WHERE id = ?1",
        [track_id],
      )
      .map_err(|error| error.to_string())?;
    if inserted == 0 {
      return Err("Track not found in the local library.".to_owned());
    }
    Ok(connection.last_insert_rowid())
  }

  pub fn save_playback_position(&self, history_id: i64, position_seconds: i64) -> Result<(), String> {
    if history_id <= 0 || position_seconds < 0 {
      return Err("Playback position is invalid.".to_owned());
    }
    let connection = self.lock()?;
    let updated = connection
      .execute(
        "UPDATE playback_history SET position_seconds = ?1 WHERE id = ?2",
        params![position_seconds, history_id],
      )
      .map_err(|error| error.to_string())?;
    if updated == 0 {
      return Err("Playback history entry not found.".to_owned());
    }
    Ok(())
  }

  pub fn latest_playback_history(&self) -> Result<Option<PlaybackHistoryEntry>, String> {
    self
      .lock()?
      .query_row(
        "SELECT h.id, h.position_seconds, t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, t.stored_name FROM playback_history h JOIN tracks t ON t.id = h.track_id LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id ORDER BY h.played_at DESC, h.id DESC LIMIT 1",
        [],
        |row| {
          Ok(PlaybackHistoryEntry {
            history_id: row.get(0)?,
            position_seconds: row.get(1)?,
            track: PlaybackTrack {
              id: row.get(2)?,
              title: row.get(3)?,
              artist: row.get(4)?,
              album: row.get(5)?,
              format: row.get(6)?,
              duration_seconds: row.get(7)?,
              stored_name: row.get(8)?,
            },
          })
        },
      )
      .optional()
      .map_err(|error| error.to_string())
  }

  pub fn insert_track(&self, track: &NewTrack) -> Result<i64, String> {
    let mut connection = self.lock()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(|error| error.to_string())?;

    let artist_key = track.artist.to_lowercase();
    transaction
      .execute(
        "INSERT INTO artists (name, name_key) VALUES (?1, ?2) ON CONFLICT(name_key) DO NOTHING",
        params![track.artist, artist_key],
      )
      .map_err(|error| error.to_string())?;
    let artist_id: i64 = transaction
      .query_row(
        "SELECT id FROM artists WHERE name_key = ?1",
        [track.artist.to_lowercase()],
        |row| row.get(0),
      )
      .map_err(|error| error.to_string())?;

    let album_key = track.album.to_lowercase();
    transaction
      .execute(
        "INSERT INTO albums (title, title_key, artist_id, artwork_name) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(artist_id, title_key) DO NOTHING",
        params![track.album, album_key, artist_id, track.artwork_name],
      )
      .map_err(|error| error.to_string())?;
    let album_id: i64 = transaction
      .query_row(
        "SELECT id FROM albums WHERE artist_id = ?1 AND title_key = ?2",
        params![artist_id, track.album.to_lowercase()],
        |row| row.get(0),
      )
      .map_err(|error| error.to_string())?;

    transaction
      .execute(
        "INSERT INTO tracks (title, artist_id, album_id, source_name, stored_name, content_sha256, format, duration_seconds, size_bytes, artwork_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
          track.title,
          artist_id,
          album_id,
          track.source_name,
          track.stored_name,
          track.content_sha256,
          track.format,
          track.duration_seconds,
          track.size_bytes,
          track.artwork_name
        ],
      )
      .map_err(|error| error.to_string())?;
    let id = transaction.last_insert_rowid();
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(id)
  }

  pub fn library_page(&self, category: &str, offset: usize) -> Result<LibraryPage, String> {
    if !matches!(category, "all" | "imported" | "artists" | "albums" | "liked") {
      return Err("Unknown library category.".to_owned());
    }

    let connection = self.lock()?;
    let (total_count, storage_bytes): (i64, i64) = connection
      .query_row(
        "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0) FROM tracks",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
      )
      .map_err(|error| error.to_string())?;
    let category_count = match category {
      "artists" => connection.query_row("SELECT COUNT(*) FROM artists", [], |row| row.get::<_, i64>(0)),
      "albums" => connection.query_row("SELECT COUNT(*) FROM albums", [], |row| row.get::<_, i64>(0)),
      "liked" => connection.query_row("SELECT COUNT(*) FROM liked_tracks", [], |row| row.get::<_, i64>(0)),
      _ => Ok(total_count),
    }
    .map_err(|error| error.to_string())?;
    let offset_i64 = i64::try_from(offset).unwrap_or(i64::MAX);
    let items = match category {
      "artists" => {
        let mut statement = connection
          .prepare(
            "SELECT a.id, a.name, COUNT(t.id) FROM artists a LEFT JOIN tracks t ON t.artist_id = a.id GROUP BY a.id ORDER BY a.name COLLATE NOCASE LIMIT ?1 OFFSET ?2",
          )
          .map_err(|error| error.to_string())?;
        let rows = statement
          .query_map(params![PAGE_SIZE as i64, offset_i64], |row| {
            let count: i64 = row.get(2)?;
            Ok(LibraryItem {
              id: row.get(0)?,
              kind: "artist".to_owned(),
              title: row.get(1)?,
              subtitle: format!("{count} {}", if count == 1 { "track" } else { "tracks" }),
              detail: None,
              format: None,
              size_bytes: None,
              duration_seconds: None,
              added_at: None,
              availability: None,
              has_artwork: false,
              track_count: Some(count),
              stored_name: None,
            })
          })
          .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?
      }
      "albums" => {
        let mut statement = connection
          .prepare(
            "SELECT al.id, al.title, COALESCE(a.name, 'Unknown artist'), COUNT(t.id), al.artwork_name FROM albums al LEFT JOIN artists a ON a.id = al.artist_id LEFT JOIN tracks t ON t.album_id = al.id GROUP BY al.id ORDER BY al.title COLLATE NOCASE LIMIT ?1 OFFSET ?2",
          )
          .map_err(|error| error.to_string())?;
        let rows = statement
          .query_map(params![PAGE_SIZE as i64, offset_i64], |row| {
            let count: i64 = row.get(3)?;
            Ok(LibraryItem {
              id: row.get(0)?,
              kind: "album".to_owned(),
              title: row.get(1)?,
              subtitle: row.get(2)?,
              detail: Some(format!("{count} {}", if count == 1 { "track" } else { "tracks" })),
              format: None,
              size_bytes: None,
              duration_seconds: None,
              added_at: None,
              availability: None,
              has_artwork: row.get::<_, Option<String>>(4)?.is_some(),
              track_count: Some(count),
              stored_name: None,
            })
          })
          .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?
      }
      _ => {
        let query = if category == "liked" {
          "SELECT t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, t.size_bytes, t.added_at, t.stored_name, t.artwork_name FROM liked_tracks l JOIN tracks t ON t.id = l.track_id LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id ORDER BY l.liked_at DESC LIMIT ?1 OFFSET ?2"
        } else {
          "SELECT t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, t.size_bytes, t.added_at, t.stored_name, t.artwork_name FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id ORDER BY t.added_at DESC, t.id DESC LIMIT ?1 OFFSET ?2"
        };
        let mut statement = connection.prepare(query).map_err(|error| error.to_string())?;
        let rows = statement
          .query_map(params![PAGE_SIZE as i64, offset_i64], |row| {
            Ok(LibraryItem {
              id: row.get(0)?,
              kind: "track".to_owned(),
              title: row.get(1)?,
              subtitle: row.get(2)?,
              detail: row.get(3)?,
              format: Some(row.get(4)?),
              size_bytes: Some(row.get(6)?),
              duration_seconds: row.get(5)?,
              added_at: Some(row.get(7)?),
              availability: None,
              has_artwork: row.get::<_, Option<String>>(9)?.is_some(),
              track_count: None,
              stored_name: Some(row.get(8)?),
            })
          })
          .map_err(|error| error.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?
      }
    };

    let next_offset = if offset.saturating_add(items.len()) < category_count as usize {
      Some(offset.saturating_add(items.len()))
    } else {
      None
    };

    Ok(LibraryPage {
      category: category.to_owned(),
      offset,
      total_count: category_count as usize,
      storage_bytes: storage_bytes.max(0) as u64,
      next_offset,
      items,
    })
  }

  pub fn search_library(&self, query: &str, offset: usize) -> Result<LibraryPage, String> {
    let query = query.trim();
    if query.is_empty() {
      return Ok(LibraryPage {
        category: "search".to_owned(),
        offset,
        total_count: 0,
        storage_bytes: 0,
        next_offset: None,
        items: Vec::new(),
      });
    }
    if query.chars().count() > 120 {
      return Err("Search terms must be 120 characters or fewer.".to_owned());
    }

    let connection = self.lock()?;
    let total_count: i64 = connection
      .query_row(
        "SELECT COUNT(*) FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id WHERE instr(lower(t.title), lower(?1)) > 0 OR instr(lower(a.name), lower(?1)) > 0 OR instr(lower(al.title), lower(?1)) > 0 OR instr(lower(t.source_name), lower(?1)) > 0",
        [query],
        |row| row.get(0),
      )
      .map_err(|error| error.to_string())?;
    let storage_bytes: i64 = connection
      .query_row("SELECT COALESCE(SUM(size_bytes), 0) FROM tracks", [], |row| row.get(0))
      .map_err(|error| error.to_string())?;
    let offset_i64 = i64::try_from(offset).unwrap_or(i64::MAX);
    let mut statement = connection
      .prepare(
        "SELECT t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, t.size_bytes, t.added_at, t.stored_name, t.artwork_name FROM tracks t LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id WHERE instr(lower(t.title), lower(?1)) > 0 OR instr(lower(a.name), lower(?1)) > 0 OR instr(lower(al.title), lower(?1)) > 0 OR instr(lower(t.source_name), lower(?1)) > 0 ORDER BY t.added_at DESC, t.id DESC LIMIT ?2 OFFSET ?3",
      )
      .map_err(|error| error.to_string())?;
    let rows = statement
      .query_map(params![query, PAGE_SIZE as i64, offset_i64], |row| {
        Ok(LibraryItem {
          id: row.get(0)?,
          kind: "track".to_owned(),
          title: row.get(1)?,
          subtitle: row.get(2)?,
          detail: row.get(3)?,
          format: Some(row.get(4)?),
          size_bytes: Some(row.get(6)?),
          duration_seconds: row.get(5)?,
          added_at: Some(row.get(7)?),
          availability: None,
          has_artwork: row.get::<_, Option<String>>(9)?.is_some(),
          track_count: None,
          stored_name: Some(row.get(8)?),
        })
      })
      .map_err(|error| error.to_string())?;
    let items = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    let next_offset = if offset.saturating_add(items.len()) < total_count as usize {
      Some(offset.saturating_add(items.len()))
    } else {
      None
    };

    Ok(LibraryPage {
      category: "search".to_owned(),
      offset,
      total_count: total_count as usize,
      storage_bytes: storage_bytes.max(0) as u64,
      next_offset,
      items,
    })
  }

  pub fn list_playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
    let connection = self.lock()?;
    let mut statement = connection
      .prepare(
        "SELECT p.id, p.name, p.description, COUNT(pt.track_id), p.created_at, p.updated_at FROM playlists p LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id GROUP BY p.id ORDER BY p.updated_at DESC, p.id DESC",
      )
      .map_err(|error| error.to_string())?;
    let rows = statement
      .query_map([], |row| {
        Ok(PlaylistSummary {
          id: row.get(0)?,
          name: row.get(1)?,
          description: row.get(2)?,
          track_count: row.get(3)?,
          created_at: row.get(4)?,
          updated_at: row.get(5)?,
        })
      })
      .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
  }

  pub fn playlist_tracks(&self, playlist_id: i64) -> Result<Vec<PlaylistTrack>, String> {
    let connection = self.lock()?;
    let mut statement = connection
      .prepare(
        "SELECT t.id, t.title, COALESCE(a.name, 'Unknown artist'), COALESCE(al.title, 'Unknown album'), t.format, t.duration_seconds, pt.position, t.stored_name FROM playlist_tracks pt JOIN tracks t ON t.id = pt.track_id LEFT JOIN artists a ON a.id = t.artist_id LEFT JOIN albums al ON al.id = t.album_id WHERE pt.playlist_id = ?1 ORDER BY pt.position",
      )
      .map_err(|error| error.to_string())?;
    let rows = statement
      .query_map([playlist_id], |row| {
        Ok(PlaylistTrack {
          id: row.get(0)?,
          title: row.get(1)?,
          artist: row.get(2)?,
          album: row.get(3)?,
          format: row.get(4)?,
          duration_seconds: row.get(5)?,
          position: row.get(6)?,
          availability: None,
          stored_name: row.get(7)?,
        })
      })
      .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())
  }

  pub fn create_playlist(&self, name: &str, description: &str) -> Result<i64, String> {
    let (name, description) = validate_playlist_fields(name, description)?;
    let connection = self.lock()?;
    connection
      .execute("INSERT INTO playlists (name, description) VALUES (?1, ?2)", params![name, description])
      .map_err(|error| error.to_string())?;
    Ok(connection.last_insert_rowid())
  }

  pub fn update_playlist(&self, id: i64, name: &str, description: &str) -> Result<(), String> {
    let (name, description) = validate_playlist_fields(name, description)?;
    let connection = self.lock()?;
    let changed = connection
      .execute(
        "UPDATE playlists SET name = ?1, description = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3",
        params![name, description, id],
      )
      .map_err(|error| error.to_string())?;
    if changed == 0 {
      return Err("Playlist not found.".to_owned());
    }
    Ok(())
  }

  pub fn delete_playlist(&self, id: i64) -> Result<(), String> {
    let connection = self.lock()?;
    let changed = connection
      .execute("DELETE FROM playlists WHERE id = ?1", [id])
      .map_err(|error| error.to_string())?;
    if changed == 0 {
      return Err("Playlist not found.".to_owned());
    }
    Ok(())
  }

  pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<(), String> {
    let mut connection = self.lock()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(|error| error.to_string())?;
    let next_position: i64 = transaction
      .query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
        |row| row.get(0),
      )
      .map_err(|error| error.to_string())?;
    transaction
      .execute(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?1, ?2, ?3)",
        params![playlist_id, track_id, next_position],
      )
      .map_err(|error| match error {
        rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::ConstraintViolation => {
          "The track is already in this playlist, or the playlist or track no longer exists.".to_owned()
        }
        _ => error.to_string(),
      })?;
    transaction
      .execute(
        "UPDATE playlists SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [playlist_id],
      )
      .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
  }

  pub fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<(), String> {
    let mut connection = self.lock()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(|error| error.to_string())?;
    let removed = transaction
      .execute(
        "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
        params![playlist_id, track_id],
      )
      .map_err(|error| error.to_string())?;
    if removed == 0 {
      return Err("Track not found in this playlist.".to_owned());
    }
    Self::reindex_playlist(&transaction, playlist_id)?;
    transaction
      .execute(
        "UPDATE playlists SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [playlist_id],
      )
      .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
  }

  pub fn reorder_playlist_track(&self, playlist_id: i64, track_id: i64, new_position: usize) -> Result<(), String> {
    let mut connection = self.lock()?;
    let transaction = connection
      .transaction_with_behavior(TransactionBehavior::Immediate)
      .map_err(|error| error.to_string())?;
    let mut tracks = {
      let mut statement = transaction
        .prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
        .map_err(|error| error.to_string())?;
      let rows = statement
        .query_map([playlist_id], |row| row.get::<_, i64>(0))
        .map_err(|error| error.to_string())?;
      rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?
    };
    let Some(current_position) = tracks.iter().position(|id| *id == track_id) else {
      return Err("Track not found in this playlist.".to_owned());
    };
    let track = tracks.remove(current_position);
    let position = new_position.min(tracks.len());
    tracks.insert(position, track);
    Self::write_playlist_order(&transaction, playlist_id, &tracks)?;
    transaction
      .execute(
        "UPDATE playlists SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        [playlist_id],
      )
      .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())
  }

  fn reindex_playlist(transaction: &rusqlite::Transaction<'_>, playlist_id: i64) -> Result<(), String> {
    let mut statement = transaction
      .prepare("SELECT track_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position")
      .map_err(|error| error.to_string())?;
    let rows = statement
      .query_map([playlist_id], |row| row.get::<_, i64>(0))
      .map_err(|error| error.to_string())?;
    let tracks = rows.collect::<Result<Vec<_>, _>>().map_err(|error| error.to_string())?;
    drop(statement);
    Self::write_playlist_order(transaction, playlist_id, &tracks)
  }

  fn write_playlist_order(
    transaction: &rusqlite::Transaction<'_>,
    playlist_id: i64,
    tracks: &[i64],
  ) -> Result<(), String> {
    let max_position: i64 = transaction
      .query_row(
        "SELECT COALESCE(MAX(position), -1) FROM playlist_tracks WHERE playlist_id = ?1",
        [playlist_id],
        |row| row.get(0),
      )
      .map_err(|error| error.to_string())?;
    let shift = max_position.saturating_add(tracks.len() as i64).saturating_add(1);
    transaction
      .execute(
        "UPDATE playlist_tracks SET position = position + ?1 WHERE playlist_id = ?2",
        params![shift, playlist_id],
      )
      .map_err(|error| error.to_string())?;
    for (position, track_id) in tracks.iter().enumerate() {
      transaction
        .execute(
          "UPDATE playlist_tracks SET position = ?1 WHERE playlist_id = ?2 AND track_id = ?3",
          params![position as i64, playlist_id, track_id],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
  }
}

fn validate_playlist_fields(name: &str, description: &str) -> Result<(String, String), String> {
  let name = name.trim();
  let description = description.trim();
  if name.is_empty() || name.chars().count() > 80 {
    return Err("Playlist name must contain 1–80 characters.".to_owned());
  }
  if description.chars().count() > 500 {
    return Err("Playlist description must be 500 characters or fewer.".to_owned());
  }
  Ok((name.to_owned(), description.to_owned()))
}

#[cfg(test)]
mod tests {
  use super::{Database, NewTrack};
  use rusqlite::Connection;
  use std::fs;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicU64, Ordering};

  static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

  fn test_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("buitenzorg-db-{}-{id}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
  }

  fn sample_track(hash: &str, stored_name: &str, title: &str) -> NewTrack {
    NewTrack {
      title: title.to_owned(),
      artist: "Test Artist".to_owned(),
      album: "Test Album".to_owned(),
      source_name: format!("{title}.wav"),
      stored_name: stored_name.to_owned(),
      content_sha256: hash.to_owned(),
      format: "wav".to_owned(),
      duration_seconds: Some(1),
      size_bytes: 44,
      artwork_name: None,
    }
  }

  #[test]
  fn empty_database_migrates_once_and_preserves_latest_schema_on_reopen() {
    let directory = test_directory();
    let path = directory.join("library.sqlite3");

    let database = Database::open(&path).unwrap();
    assert_eq!(database.schema_version().unwrap(), 1);
    drop(database);

    let reopened = Database::open(&path).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 1);
    assert_eq!(reopened.library_page("all", 0).unwrap().total_count, 0);

    drop(reopened);
    fs::remove_dir_all(directory).unwrap();
  }

  #[test]
  fn version_zero_upgrade_preserves_preexisting_user_data() {
    let directory = test_directory();
    let path = directory.join("library.sqlite3");
    {
      let connection = Connection::open(&path).unwrap();
      connection.execute_batch("CREATE TABLE preserved_user_data (value TEXT NOT NULL); INSERT INTO preserved_user_data VALUES ('keep');").unwrap();
      connection.pragma_update(None, "user_version", 0_i64).unwrap();
    }

    let database = Database::open(&path).unwrap();
    let value: String = database
      .lock()
      .unwrap()
      .query_row("SELECT value FROM preserved_user_data", [], |row| row.get(0))
      .unwrap();
    assert_eq!(database.schema_version().unwrap(), 1);
    assert_eq!(value, "keep");

    drop(database);
    fs::remove_dir_all(directory).unwrap();
  }

  #[test]
  fn migration_creates_expected_local_tables_without_a_credentials_column() {
    let database = Database::open_in_memory().unwrap();
    let connection = database.lock().unwrap();
    let mut statement = connection
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
      .unwrap();
    let tables = statement
      .query_map([], |row| row.get::<_, String>(0))
      .unwrap()
      .collect::<Result<Vec<_>, _>>()
      .unwrap();

    for required in [
      "local_profile",
      "tracks",
      "artists",
      "albums",
      "playlists",
      "playlist_tracks",
      "liked_tracks",
      "downloads",
      "playback_history",
      "provider_status",
      "application_settings",
    ] {
      assert!(tables.iter().any(|table| table == required), "missing table {required}");
    }
    assert!(!tables.iter().any(|table| table.contains("credential")));
  }

  #[test]
  fn playlist_crud_and_reordering_keep_contiguous_unique_positions() {
    let database = Database::open_in_memory().unwrap();
    let playlist_id = database.create_playlist(" Focus ", "  Local set  ").unwrap();
    let track_a = database.insert_track(&sample_track(&"a".repeat(64), &format!("{}.wav", "a".repeat(64)), "A")).unwrap();
    let track_b = database.insert_track(&sample_track(&"b".repeat(64), &format!("{}.wav", "b".repeat(64)), "B")).unwrap();
    let track_c = database.insert_track(&sample_track(&"c".repeat(64), &format!("{}.wav", "c".repeat(64)), "C")).unwrap();

    database.add_track_to_playlist(playlist_id, track_a).unwrap();
    database.add_track_to_playlist(playlist_id, track_b).unwrap();
    database.add_track_to_playlist(playlist_id, track_c).unwrap();
    database.reorder_playlist_track(playlist_id, track_c, 0).unwrap();
    let order = database.playlist_tracks(playlist_id).unwrap();
    assert_eq!(order.iter().map(|track| track.id).collect::<Vec<_>>(), vec![track_c, track_a, track_b]);
    assert_eq!(order.iter().map(|track| track.position).collect::<Vec<_>>(), vec![0, 1, 2]);

    database.remove_track_from_playlist(playlist_id, track_a).unwrap();
    database.update_playlist(playlist_id, "Focus edited", "Updated").unwrap();
    assert_eq!(database.list_playlists().unwrap()[0].name, "Focus edited");
    assert_eq!(database.playlist_tracks(playlist_id).unwrap()[1].position, 1);
    database.delete_playlist(playlist_id).unwrap();
    assert!(database.list_playlists().unwrap().is_empty());
  }

  #[test]
  fn invalid_library_categories_and_playlist_names_are_rejected() {
    let database = Database::open_in_memory().unwrap();
    assert!(database.library_page("invalid", 0).is_err());
    assert!(database.create_playlist("  ", "").is_err());
    assert!(database.create_playlist(&"x".repeat(81), "").is_err());
  }

  #[test]
  fn local_search_matches_track_artist_and_album_without_treating_input_as_sql() {
    let database = Database::open_in_memory().unwrap();
    database.insert_track(&sample_track(&"d".repeat(64), &format!("{}.wav", "d".repeat(64)), "Blue Hour")).unwrap();

    let title_match = database.search_library("blue", 0).unwrap();
    assert_eq!(title_match.total_count, 1);
    assert_eq!(title_match.items[0].title, "Blue Hour");
    assert_eq!(database.search_library("test artist", 0).unwrap().total_count, 1);
    assert_eq!(database.search_library("%' OR 1=1 --", 0).unwrap().total_count, 0);
    assert_eq!(database.search_library("", 0).unwrap().total_count, 0);
    assert!(database.search_library(&"x".repeat(121), 0).is_err());
  }

  #[test]
  fn playback_history_records_and_updates_position_for_existing_tracks_only() {
    let database = Database::open_in_memory().unwrap();
    let track_id = database
      .insert_track(&sample_track(&"e".repeat(64), &format!("{}.wav", "e".repeat(64)), "Resume point"))
      .unwrap();

    let history_id = database.start_playback_history(track_id).unwrap();
    database.save_playback_position(history_id, 17).unwrap();
    let latest = database.latest_playback_history().unwrap().unwrap();
    assert_eq!(latest.history_id, history_id);
    assert_eq!(latest.position_seconds, 17);
    assert_eq!(latest.track.id, track_id);
    assert_eq!(latest.track.title, "Resume point");
    assert!(database.save_playback_position(history_id, -1).is_err());
    assert!(database.start_playback_history(i64::MAX).is_err());
  }
}