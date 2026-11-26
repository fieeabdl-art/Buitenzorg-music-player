use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{Database, LibraryPage, NewTrack, PlaybackTrack, PlaylistSummary, PlaylistTrack};
use crate::media_server::LocalMediaServer;

const SUPPORTED_EXTENSIONS: &[&str] = &["aac", "aif", "aiff", "flac", "m4a", "mp3", "mp4", "oga", "ogg", "opus", "wav"];
const LEGACY_STORED_EXTENSIONS: &[&str] = &["webm"];
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARTWORK_BYTES: usize = 20 * 1024 * 1024;
const MAX_DIRECTORY_ENTRIES: usize = 100_000;
const MAX_LEGACY_INDEX_BYTES: u64 = 16 * 1024 * 1024;
const COPY_BUFFER_SIZE: usize = 1024 * 1024;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone)]
pub struct LibraryService {
  database: Arc<Database>,
  audio_directory: PathBuf,
  artwork_directory: PathBuf,
  media_server: Arc<LocalMediaServer>,
  operation_lock: Arc<Mutex<()>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportIssue {
  pub item: String,
  pub message: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
  pub cancelled: bool,
  pub scanned: usize,
  pub imported: usize,
  pub duplicates: usize,
  pub skipped: usize,
  pub issues: Vec<ImportIssue>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStorageInfo {
  pub storage_bytes: u64,
  pub track_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPlaybackTrack {
  pub track: PlaybackTrack,
  pub audio_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumablePlayback {
  pub history_id: i64,
  pub position_seconds: i64,
  pub track: PlaybackTrack,
  pub audio_url: String,
}

#[derive(Clone, Debug)]
struct AudioMetadata {
  title: String,
  artist: String,
  album: String,
  genre: Option<String>,
  year: Option<i64>,
  track_number: Option<i64>,
  duration_seconds: Option<i64>,
  artwork: Option<Vec<u8>>,
}

#[derive(Debug)]
struct TemporaryCopy {
  path: PathBuf,
  sha256: String,
  size_bytes: u64,
}

impl Drop for TemporaryCopy {
  fn drop(&mut self) {
    let _ = fs::remove_file(&self.path);
  }
}

#[derive(Debug, Deserialize)]
struct LegacyTrack {
  id: Option<String>,
  title: Option<String>,
  artist: Option<String>,
  album: Option<String>,
  file_name: Option<String>,
  stored_name: Option<String>,
}

impl LibraryService {
  pub fn new(database: Database, root: PathBuf) -> Result<Self, String> {
    create_managed_directory(&root)?;
    let root = root.canonicalize().map_err(|error| io_message(&error))?;
    let audio_directory = root.join("audio");
    let artwork_directory = root.join("artwork");
    create_managed_directory(&audio_directory)?;
    create_managed_directory(&artwork_directory)?;
    let media_server = Arc::new(LocalMediaServer::start(&audio_directory, &artwork_directory)?);

    Ok(Self {
      database: Arc::new(database),
      audio_directory,
      artwork_directory,
      media_server,
      operation_lock: Arc::new(Mutex::new(())),
    })
  }

  pub fn list_library_page(&self, category: &str, offset: usize) -> Result<LibraryPage, String> {
    let mut page = self.database.library_page(category, offset)?;
    for item in &mut page.items {
      if let Some(stored_name) = &item.stored_name {
        item.availability = Some(self.file_availability(stored_name));
      }
      item.artwork_url = item.artwork_name.as_deref().and_then(|name| self.media_server.url_for_artwork(name).ok());
    }
    Ok(page)
  }

  pub fn search_local_library(&self, query: &str, offset: usize) -> Result<LibraryPage, String> {
    let mut page = self.database.search_library(query, offset)?;
    for item in &mut page.items {
      if let Some(stored_name) = &item.stored_name {
        item.availability = Some(self.file_availability(stored_name));
      }
      item.artwork_url = item.artwork_name.as_deref().and_then(|name| self.media_server.url_for_artwork(name).ok());
    }
    Ok(page)
  }

  pub fn import_selected_files(&self, paths: Vec<PathBuf>) -> Result<ImportReport, String> {
    if paths.is_empty() {
      return Ok(ImportReport { cancelled: true, ..ImportReport::default() });
    }
    let _operation = self.operation_lock.lock().map_err(|_| "The library is busy.".to_owned())?;
    let mut report = ImportReport::default();
    for path in paths {
      report.scanned += 1;
      let item = safe_file_name(&path);
      let result = self.import_file(&path, None, None);
      self.add_import_result(&mut report, item, result);
    }
    Ok(report)
  }

  pub fn scan_folder(&self, folder: &Path) -> Result<ImportReport, String> {
    let _operation = self.operation_lock.lock().map_err(|_| "The library is busy.".to_owned())?;
    let root_metadata = fs::symlink_metadata(folder).map_err(|error| io_message(&error))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
      return Err("The selected folder is unavailable or is not a regular directory.".to_owned());
    }
    let root = folder.canonicalize().map_err(|error| io_message(&error))?;
    let mut report = ImportReport::default();
    let mut directories = vec![root];
    let mut visited = 0_usize;

    while let Some(directory) = directories.pop() {
      let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) => {
          report.skipped += 1;
          report.issues.push(ImportIssue {
            item: safe_file_name(&directory),
            message: io_message(&error),
          });
          continue;
        }
      };

      for entry in entries {
        visited += 1;
        if visited > MAX_DIRECTORY_ENTRIES {
          report.skipped += 1;
          report.issues.push(ImportIssue {
            item: safe_file_name(&directory),
            message: "Folder scan stopped at the safety limit of 100,000 entries.".to_owned(),
          });
          return Ok(report);
        }

        let entry = match entry {
          Ok(entry) => entry,
          Err(error) => {
            report.skipped += 1;
            report.issues.push(ImportIssue {
              item: safe_file_name(&directory),
              message: io_message(&error),
            });
            continue;
          }
        };
        let path = entry.path();
        let metadata = match fs::symlink_metadata(&path) {
          Ok(metadata) => metadata,
          Err(error) => {
            report.skipped += 1;
            report.issues.push(ImportIssue {
              item: safe_file_name(&path),
              message: io_message(&error),
            });
            continue;
          }
        };
        if metadata.file_type().is_symlink() {
          continue;
        }
        if metadata.is_dir() {
          directories.push(path);
          continue;
        }
        if !metadata.is_file() || !is_supported_audio(&path) {
          continue;
        }

        report.scanned += 1;
        let item = safe_file_name(&path);
        let result = self.import_file(&path, None, None);
        self.add_import_result(&mut report, item, result);
      }
    }
    Ok(report)
  }

  pub fn migrate_legacy_library(&self) -> Result<ImportReport, String> {
    let root = legacy_library_root()?;
    self.migrate_legacy_from(&root)
  }

  fn migrate_legacy_from(&self, legacy_root: &Path) -> Result<ImportReport, String> {
    let _operation = self.operation_lock.lock().map_err(|_| "The library is busy.".to_owned())?;
    let index_path = legacy_root.join("library.json");
    let index_metadata = fs::symlink_metadata(&index_path).map_err(|error| {
      if error.kind() == io::ErrorKind::NotFound {
        "No previous local library index was found.".to_owned()
      } else {
        io_message(&error)
      }
    })?;
    if index_metadata.file_type().is_symlink() || !index_metadata.is_file() {
      return Err("The previous library index is not a regular file.".to_owned());
    }
    if index_metadata.len() > MAX_LEGACY_INDEX_BYTES {
      return Err("The previous library index exceeds the safe import size limit.".to_owned());
    }
    let mut index_bytes = Vec::new();
    File::open(&index_path)
      .map_err(|error| io_message(&error))?
      .take(MAX_LEGACY_INDEX_BYTES + 1)
      .read_to_end(&mut index_bytes)
      .map_err(|error| io_message(&error))?;
    if index_bytes.len() as u64 > MAX_LEGACY_INDEX_BYTES {
      return Err("The previous library index exceeds the safe import size limit.".to_owned());
    }
    let entries: Vec<LegacyTrack> = serde_json::from_slice(&index_bytes)
      .map_err(|_| "The previous library index could not be read.".to_owned())?;
    let legacy_audio = legacy_root.join("audio");
    let audio_metadata = fs::symlink_metadata(&legacy_audio).map_err(|error| io_message(&error))?;
    if audio_metadata.file_type().is_symlink() || !audio_metadata.is_dir() {
      return Err("The previous library audio folder is unavailable.".to_owned());
    }
    let legacy_audio = legacy_audio.canonicalize().map_err(|error| io_message(&error))?;
    let mut report = ImportReport::default();

    for entry in entries {
      report.scanned += 1;
      let validation = validate_legacy_entry(&entry, &legacy_audio);
      let (source, expected_hash) = match validation {
        Ok(value) => value,
        Err((item, message)) => {
          report.skipped += 1;
          report.issues.push(ImportIssue { item, message });
          continue;
        }
      };
      let item = entry
        .file_name
        .as_deref()
        .map(|name| clean_text(name, "Previous audio", 255))
        .unwrap_or_else(|| "Previous audio".to_owned());
      let fallbacks = LegacyFallbacks {
        title: entry.title.as_deref(),
        artist: entry.artist.as_deref(),
        album: entry.album.as_deref(),
      };
      self.add_import_result(
        &mut report,
        item,
        self.import_file(&source, Some(&expected_hash), Some(fallbacks)),
      );
    }

    Ok(report)
  }

  pub fn storage_info(&self) -> Result<LibraryStorageInfo, String> {
    let page = self.database.library_page("all", 0)?;
    Ok(LibraryStorageInfo {
      storage_bytes: page.storage_bytes,
      track_count: page.total_count,
    })
  }

  pub fn resolve_playback_track(&self, track_id: i64) -> Result<ResolvedPlaybackTrack, String> {
    let mut track = self.database.playback_track(track_id)?
      .ok_or_else(|| "Track not found in the local library.".to_owned())?;
    self.playable_audio_path(&track.stored_name)?;
    let audio_url = self.media_server.url_for(&track.stored_name)?;
    track.artwork_url = track.artwork_name.as_deref().and_then(|name| self.media_server.url_for_artwork(name).ok());
    Ok(ResolvedPlaybackTrack {
      track,
      audio_url,
    })
  }

  pub fn start_playback_history(&self, track_id: i64) -> Result<i64, String> {
    self.database.start_playback_history(track_id)
  }

  pub fn save_playback_position(&self, history_id: i64, position_seconds: i64) -> Result<(), String> {
    self.database.save_playback_position(history_id, position_seconds)
  }

  pub fn resumable_playback(&self) -> Result<Option<ResumablePlayback>, String> {
    let Some(mut entry) = self.database.latest_playback_history()? else {
      return Ok(None);
    };
    self.playable_audio_path(&entry.track.stored_name)?;
    let audio_url = self.media_server.url_for(&entry.track.stored_name)?;
    entry.track.artwork_url = entry
      .track
      .artwork_name
      .as_deref()
      .and_then(|name| self.media_server.url_for_artwork(name).ok());
    Ok(Some(ResumablePlayback {
      history_id: entry.history_id,
      position_seconds: entry.position_seconds,
      track: entry.track,
      audio_url,
    }))
  }

  pub fn list_playlists(&self) -> Result<Vec<PlaylistSummary>, String> {
    self.database.list_playlists()
  }

  pub fn playlist_tracks(&self, playlist_id: i64) -> Result<Vec<PlaylistTrack>, String> {
    let mut tracks = self.database.playlist_tracks(playlist_id)?;
    for track in &mut tracks {
      track.availability = Some(self.file_availability(&track.stored_name));
      track.artwork_url = track.artwork_name.as_deref().and_then(|name| self.media_server.url_for_artwork(name).ok());
    }
    Ok(tracks)
  }

  pub fn create_playlist(&self, name: &str, description: &str) -> Result<i64, String> {
    self.database.create_playlist(name, description)
  }

  pub fn update_playlist(&self, id: i64, name: &str, description: &str) -> Result<(), String> {
    self.database.update_playlist(id, name, description)
  }

  pub fn delete_playlist(&self, id: i64) -> Result<(), String> {
    self.database.delete_playlist(id)
  }

  pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<(), String> {
    self.database.add_track_to_playlist(playlist_id, track_id)
  }

  pub fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<(), String> {
    self.database.remove_track_from_playlist(playlist_id, track_id)
  }

  pub fn reorder_playlist_track(&self, playlist_id: i64, track_id: i64, position: usize) -> Result<(), String> {
    self.database.reorder_playlist_track(playlist_id, track_id, position)
  }

  fn add_import_result(&self, report: &mut ImportReport, item: String, result: Result<bool, String>) {
    match result {
      Ok(true) => report.duplicates += 1,
      Ok(false) => report.imported += 1,
      Err(message) => {
        report.skipped += 1;
        report.issues.push(ImportIssue { item, message });
      }
    }
  }

  fn import_file(
    &self,
    source: &Path,
    expected_hash: Option<&str>,
    fallbacks: Option<LegacyFallbacks<'_>>,
  ) -> Result<bool, String> {
    let original_metadata = fs::symlink_metadata(source).map_err(|error| io_message(&error))?;
    if original_metadata.file_type().is_symlink() || !original_metadata.is_file() {
      return Err("The selected source is not a regular audio file.".to_owned());
    }
    if !is_supported_audio(source) {
      return Err("This audio format is not supported for import.".to_owned());
    }
    if original_metadata.len() > MAX_FILE_BYTES {
      return Err("The audio file exceeds the 2 GiB import limit.".to_owned());
    }
    let source = source.canonicalize().map_err(|error| io_message(&error))?;
    let mut metadata = read_audio_metadata(&source, fallbacks)?;
    let extension = audio_extension(&source).ok_or_else(|| "This audio format is not supported for import.".to_owned())?;
    let temporary = self.copy_to_temporary(&source)?;

    if expected_hash.is_some_and(|expected| expected != temporary.sha256) {
      let _ = fs::remove_file(&temporary.path);
      return Err("The previous library checksum does not match the audio file.".to_owned());
    }

    if let Some(existing) = self.database.find_track_by_hash(&temporary.sha256)? {
      let existing_path = self.managed_audio_path(&existing.stored_name)?;
      match path_availability(&existing_path) {
        "available" => {
          if hash_file(&existing_path)? == temporary.sha256 {
            return Ok(true);
          }
          return Err("The existing managed copy failed its checksum verification.".to_owned());
        }
        "missing" => {
          fs::rename(&temporary.path, existing_path).map_err(|error| io_message(&error))?;
          return Ok(true);
        }
        _ => {
          return Err("The managed copy exists but cannot be safely replaced.".to_owned());
        }
      }
    }

    let stored_name = format!("{}.{}", temporary.sha256, extension);
    let destination = self.managed_audio_path(&stored_name)?;
    let mut created_audio = false;
    match path_availability(&destination) {
      "missing" => {
        fs::rename(&temporary.path, &destination).map_err(|error| io_message(&error))?;
        created_audio = true;
      }
      "available" => {
        if hash_file(&destination)? != temporary.sha256 {
          return Err("A conflicting managed file already exists; it was left unchanged.".to_owned());
        }
      }
      _ => return Err("A conflicting managed file cannot be safely replaced.".to_owned()),
    }

    let stored_artwork = match metadata.artwork.take() {
      Some(bytes) => match self.store_artwork(&bytes) {
        Ok(artwork) => artwork,
        Err(error) => {
          if created_audio {
            let _ = fs::remove_file(&destination);
          }
          return Err(error);
        }
      },
      None => None,
    };
    let artwork_name = stored_artwork.as_ref().map(|(name, _)| name.clone());
    let source_name = clean_text(
      source.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
      "Local audio",
      255,
    );
    let track = NewTrack {
      title: clean_text(&metadata.title, "Untitled audio", 200),
      artist: clean_text(&metadata.artist, "Unknown artist", 200),
      album: clean_text(&metadata.album, "Local files", 200),
      source_name,
      stored_name,
      content_sha256: temporary.sha256.clone(),
      format: extension.to_owned(),
      duration_seconds: metadata.duration_seconds,
      size_bytes: i64::try_from(temporary.size_bytes).unwrap_or(i64::MAX),
      artwork_name,
      genre: metadata.genre,
      year: metadata.year,
      track_number: metadata.track_number,
    };

    match self.database.insert_track(&track) {
      Ok(_) => Ok(false),
      Err(error) => {
        if created_audio {
          let _ = fs::remove_file(&destination);
        }
        if let Some((artwork_name, true)) = stored_artwork {
          let _ = fs::remove_file(self.artwork_directory.join(artwork_name));
        }
        Err(error)
      }
    }
  }

  fn copy_to_temporary(&self, source: &Path) -> Result<TemporaryCopy, String> {
    let mut input = File::open(source).map_err(|error| io_message(&error))?;
    let (temporary_path, mut output) = self.create_temporary_file(&self.audio_directory)?;
    let result = (|| {
      let mut hasher = Sha256::new();
      let mut size_bytes = 0_u64;
      let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];
      loop {
        let read = input.read(&mut buffer).map_err(|error| io_message(&error))?;
        if read == 0 {
          break;
        }
        size_bytes = size_bytes.saturating_add(read as u64);
        if size_bytes > MAX_FILE_BYTES {
          return Err("The audio file exceeds the 2 GiB import limit.".to_owned());
        }
        hasher.update(&buffer[..read]);
        output.write_all(&buffer[..read]).map_err(|error| io_message(&error))?;
      }
      output.flush().map_err(|error| io_message(&error))?;
      output.sync_all().map_err(|error| io_message(&error))?;
      Ok(TemporaryCopy {
        path: temporary_path.clone(),
        sha256: hex_digest(&hasher.finalize()),
        size_bytes,
      })
    })();
    drop(output);
    if result.is_err() {
      let _ = fs::remove_file(&temporary_path);
    }
    result
  }

  fn create_temporary_file(&self, directory: &Path) -> Result<(PathBuf, File), String> {
    for _ in 0..10 {
      let id = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
      let path = directory.join(format!(".import-{}-{id}.tmp", std::process::id()));
      match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => return Ok((path, file)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
        Err(error) => return Err(io_message(&error)),
      }
    }
    Err("A safe temporary file could not be created.".to_owned())
  }

  fn store_artwork(&self, bytes: &[u8]) -> Result<Option<(String, bool)>, String> {
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
      return Ok(None);
    }
    let Some(extension) = image_extension(bytes) else {
      return Ok(None);
    };
    let hash = sha256_bytes(bytes);
    let file_name = format!("{hash}.{extension}");
    let destination = self.artwork_directory.join(&file_name);
    if path_availability(&destination) == "available" {
      if hash_file(&destination)? == hash {
        return Ok(Some((file_name, false)));
      }
      return Err("An existing artwork file failed its checksum verification.".to_owned());
    }
    if path_availability(&destination) != "missing" {
      return Err("An artwork path cannot be safely replaced.".to_owned());
    }
    let (temporary_path, mut output) = self.create_temporary_file(&self.artwork_directory)?;
    let result = (|| {
      output.write_all(bytes).map_err(|error| io_message(&error))?;
      output.flush().map_err(|error| io_message(&error))?;
      output.sync_all().map_err(|error| io_message(&error))?;
      Ok(())
    })();
    drop(output);
    if let Err(error) = result {
      let _ = fs::remove_file(&temporary_path);
      return Err(error);
    }
    if let Err(error) = fs::rename(&temporary_path, &destination) {
      let _ = fs::remove_file(&temporary_path);
      if path_availability(&destination) == "available" && hash_file(&destination)? == hash {
        return Ok(Some((file_name, false)));
      }
      return Err(io_message(&error));
    }
    Ok(Some((file_name, true)))
  }

  fn managed_audio_path(&self, stored_name: &str) -> Result<PathBuf, String> {
    if !is_safe_stored_name(stored_name) {
      return Err("The managed library contains an invalid audio reference.".to_owned());
    }
    Ok(self.audio_directory.join(stored_name))
  }

  fn playable_audio_path(&self, stored_name: &str) -> Result<PathBuf, String> {
    let path = self.managed_audio_path(stored_name)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
      if error.kind() == io::ErrorKind::NotFound {
        "This track's managed audio file is missing.".to_owned()
      } else {
        io_message(&error)
      }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
      return Err("The managed audio file is unavailable.".to_owned());
    }
    let canonical = path.canonicalize().map_err(|error| io_message(&error))?;
    if canonical.parent() != Some(self.audio_directory.as_path()) {
      return Err("The managed audio file resolves outside the library folder.".to_owned());
    }
    Ok(canonical)
  }

  fn file_availability(&self, stored_name: &str) -> String {
    match self.managed_audio_path(stored_name) {
      Ok(path) => path_availability(&path).to_owned(),
      Err(_) => "unreadable".to_owned(),
    }
  }
}

struct LegacyFallbacks<'a> {
  title: Option<&'a str>,
  artist: Option<&'a str>,
  album: Option<&'a str>,
}

fn metadata_from_tag(tag: Option<&lofty::tag::Tag>) -> (Option<String>, Option<i64>, Option<i64>) {
  let genre = tag
    .and_then(Accessor::genre)
    .map(|value| value.into_owned())
    .map(|value| clean_text(&value, "", 100))
    .filter(|value| !value.is_empty());
  let year = tag
    .and_then(Accessor::date)
    .map(|date| i64::from(date.year))
    .filter(|year| (1..=9999).contains(year));
  let track_number = tag.and_then(Accessor::track).map(i64::from).filter(|number| *number > 0);
  (genre, year, track_number)
}

fn read_audio_metadata(path: &Path, fallbacks: Option<LegacyFallbacks<'_>>) -> Result<AudioMetadata, String> {
  let tagged_file = lofty::read_from_path(path)
    .map_err(|_| "The selected file could not be read as a supported audio format.".to_owned())?;
  let tag = tagged_file.primary_tag();
  let fallback_title = fallbacks.as_ref().and_then(|fallback| fallback.title).unwrap_or_default();
  let fallback_artist = fallbacks.as_ref().and_then(|fallback| fallback.artist).unwrap_or_default();
  let fallback_album = fallbacks.as_ref().and_then(|fallback| fallback.album).unwrap_or_default();
  let title = tag
    .and_then(Accessor::title)
    .map(|value| value.into_owned())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| clean_text(fallback_title, path.file_stem().and_then(|name| name.to_str()).unwrap_or("Untitled audio"), 200));
  let artist = tag
    .and_then(Accessor::artist)
    .map(|value| value.into_owned())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| clean_text(fallback_artist, "Unknown artist", 200));
  let album = tag
    .and_then(Accessor::album)
    .map(|value| value.into_owned())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| clean_text(fallback_album, "Local files", 200));
  let (genre, year, track_number) = metadata_from_tag(tag);
  let artwork = tag
    .and_then(|tag| tag.pictures().iter().find(|picture| image_extension(picture.data()).is_some()))
    .map(|picture| picture.data().to_vec())
    .filter(|bytes| bytes.len() <= MAX_ARTWORK_BYTES);

  Ok(AudioMetadata {
    title: clean_text(&title, "Untitled audio", 200),
    artist: clean_text(&artist, "Unknown artist", 200),
    album: clean_text(&album, "Local files", 200),
    genre,
    year,
    track_number,
    duration_seconds: Some(tagged_file.properties().duration().as_secs().min(i64::MAX as u64) as i64),
    artwork,
  })
}

fn create_managed_directory(path: &Path) -> Result<(), String> {
  fs::create_dir_all(path).map_err(|error| io_message(&error))?;
  let metadata = fs::symlink_metadata(path).map_err(|error| io_message(&error))?;
  if metadata.file_type().is_symlink() || !metadata.is_dir() {
    return Err("A managed library path is not a regular directory.".to_owned());
  }
  Ok(())
}

fn legacy_library_root() -> Result<PathBuf, String> {
  if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
    let data_home = PathBuf::from(data_home);
    if !data_home.is_absolute() {
      return Err("XDG_DATA_HOME is not an absolute path; the legacy library was not read.".to_owned());
    }
    return Ok(data_home.join("offline-music-library"));
  }
  let home = std::env::var_os("HOME").ok_or_else(|| "The home directory is unavailable; legacy import cannot run.".to_owned())?;
  Ok(PathBuf::from(home).join(".local/share/offline-music-library"))
}

fn validate_legacy_entry(entry: &LegacyTrack, audio_directory: &Path) -> Result<(PathBuf, String), (String, String)> {
  let item = entry
    .file_name
    .as_deref()
    .map(|name| clean_text(name, "Previous audio", 255))
    .unwrap_or_else(|| "Previous audio".to_owned());
  let Some(hash) = entry.id.as_deref().filter(|hash| is_sha256(hash)) else {
    return Err((item, "The previous library entry has an invalid checksum.".to_owned()));
  };
  let Some(stored_name) = entry.stored_name.as_deref() else {
    return Err((item, "The previous library entry has no stored audio name.".to_owned()));
  };
  if !is_safe_stored_name(stored_name) || !stored_name.starts_with(&format!("{hash}.")) {
    return Err((item, "The previous library entry contains an unsafe audio path.".to_owned()));
  }
  let source = audio_directory.join(stored_name);
  let metadata = fs::symlink_metadata(&source).map_err(|error| (item.clone(), io_message(&error)))?;
  if metadata.file_type().is_symlink() || !metadata.is_file() {
    return Err((item, "The previous audio copy is not a regular file.".to_owned()));
  }
  let canonical = source.canonicalize().map_err(|error| (item.clone(), io_message(&error)))?;
  if canonical.parent() != Some(audio_directory) {
    return Err((item, "The previous audio path resolves outside its library folder.".to_owned()));
  }
  Ok((canonical, hash.to_owned()))
}

fn is_supported_audio(path: &Path) -> bool {
  audio_extension(path).is_some()
}

fn audio_extension(path: &Path) -> Option<&str> {
  let extension = path.extension()?.to_str()?.to_ascii_lowercase();
  SUPPORTED_EXTENSIONS
    .iter()
    .find(|candidate| **candidate == extension)
    .copied()
}

fn is_safe_stored_name(stored_name: &str) -> bool {
  let Some((hash, extension)) = stored_name.rsplit_once('.') else {
    return false;
  };
  is_sha256(hash)
    && (SUPPORTED_EXTENSIONS.contains(&extension) || LEGACY_STORED_EXTENSIONS.contains(&extension))
    && Path::new(stored_name).file_name().and_then(|name| name.to_str()) == Some(stored_name)
}

fn is_sha256(value: &str) -> bool {
  value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn path_availability(path: &Path) -> &'static str {
  match fs::symlink_metadata(path) {
    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => "available",
    Ok(_) => "unreadable",
    Err(error) if error.kind() == io::ErrorKind::NotFound => "missing",
    Err(_) => "unreadable",
  }
}

fn clean_text(value: &str, fallback: &str, max_chars: usize) -> String {
  let cleaned = value
    .replace('\0', "")
    .split_whitespace()
    .collect::<Vec<_>>()
    .join(" ");
  let cleaned = cleaned.chars().take(max_chars).collect::<String>();
  if cleaned.is_empty() { fallback.to_owned() } else { cleaned }
}

fn safe_file_name(path: &Path) -> String {
  path.file_name()
    .and_then(|name| name.to_str())
    .map(|name| clean_text(name, "Local file", 255))
    .unwrap_or_else(|| "Local file".to_owned())
}

fn io_message(error: &io::Error) -> String {
  match error.kind() {
    io::ErrorKind::NotFound => "The file or folder is no longer available.".to_owned(),
    io::ErrorKind::PermissionDenied => "Access was denied for this file or folder.".to_owned(),
    io::ErrorKind::AlreadyExists => "A file with this name already exists.".to_owned(),
    io::ErrorKind::InvalidInput | io::ErrorKind::InvalidData => "The file or folder could not be read.".to_owned(),
    io::ErrorKind::StorageFull => "There is not enough storage space to copy this file.".to_owned(),
    _ => "A file operation failed; the source was left unchanged.".to_owned(),
  }
}

fn sha256_bytes(bytes: &[u8]) -> String {
  hex_digest(&Sha256::digest(bytes))
}

fn hash_file(path: &Path) -> Result<String, String> {
  let mut file = File::open(path).map_err(|error| io_message(&error))?;
  let mut hasher = Sha256::new();
  let mut buffer = vec![0_u8; COPY_BUFFER_SIZE];
  loop {
    let read = file.read(&mut buffer).map_err(|error| io_message(&error))?;
    if read == 0 {
      break;
    }
    hasher.update(&buffer[..read]);
  }
  Ok(hex_digest(&hasher.finalize()))
}

fn hex_digest(bytes: &[u8]) -> String {
  const HEX: &[u8; 16] = b"0123456789abcdef";
  let mut output = String::with_capacity(bytes.len() * 2);
  for byte in bytes {
    output.push(HEX[(byte >> 4) as usize] as char);
    output.push(HEX[(byte & 0x0f) as usize] as char);
  }
  output
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
  if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
    Some("jpg")
  } else if bytes.starts_with(b"\x89PNG\r\n\x1A\n") {
    Some("png")
  } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
    Some("gif")
  } else if bytes.starts_with(b"BM") {
    Some("bmp")
  } else if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
    Some("tiff")
  } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
    Some("webp")
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use super::{ImportReport, LibraryService, io_message, metadata_from_tag, sha256_bytes};
  use crate::db::Database;
  use lofty::tag::items::Timestamp;
  use lofty::tag::{Accessor, Tag, TagType};
  use std::fs;
  use std::io;
  use std::path::{Path, PathBuf};
  use std::sync::atomic::{AtomicU64, Ordering};

  static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

  fn test_directory() -> PathBuf {
    let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("buitenzorg-library-{}-{id}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
  }

  fn make_service(root: &Path) -> LibraryService {
    let database = Database::open(&root.join("app.sqlite3")).unwrap();
    LibraryService::new(database, root.join("managed/library")).unwrap()
  }

  fn write_wav(path: &Path, seconds: u32) -> Vec<u8> {
    let sample_rate = 8_000_u32;
    let data_length = (sample_rate * seconds) as usize;
    let mut bytes = Vec::with_capacity(44 + data_length);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36_u32 + data_length as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_length as u32).to_le_bytes());
    bytes.resize(44 + data_length, 128);
    fs::write(path, &bytes).unwrap();
    bytes
  }

  #[test]
  fn tag_metadata_omits_missing_or_invalid_values_and_normalizes_genre() {
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_genre("  Ambient   electronic ".to_owned());
    tag.set_date(Timestamp { year: 2026, month: None, day: None, hour: None, minute: None, second: None });
    tag.set_track(4);
    assert_eq!(metadata_from_tag(Some(&tag)), (Some("Ambient electronic".to_owned()), Some(2026), Some(4)));

    tag.set_genre("   ".to_owned());
    tag.set_date(Timestamp { year: 0, month: None, day: None, hour: None, minute: None, second: None });
    tag.set_track(0);
    assert_eq!(metadata_from_tag(Some(&tag)), (None, None, None));
    assert_eq!(metadata_from_tag(None), (None, None, None));
  }

  fn write_legacy_index(root: &Path, source: &Path, bytes: &[u8]) -> String {
    let digest = sha256_bytes(bytes);
    let stored_name = format!("{digest}.wav");
    let audio_directory = root.join("audio");
    fs::create_dir_all(&audio_directory).unwrap();
    fs::copy(source, audio_directory.join(&stored_name)).unwrap();
    let entry = serde_json::json!({
      "id": digest,
      "title": "Legacy title",
      "artist": "Legacy artist",
      "album": "Legacy album",
      "file_name": "legacy.wav",
      "stored_name": stored_name,
      "size": bytes.len()
    });
    fs::write(root.join("library.json"), serde_json::to_vec(&vec![entry]).unwrap()).unwrap();
    digest
  }

  fn summary(report: &ImportReport) -> (usize, usize, usize) {
    (report.imported, report.duplicates, report.skipped)
  }

  #[test]
  fn import_copies_audio_verifies_hash_and_deduplicates_without_touching_source() {
    let root = test_directory();
    let service = make_service(&root);
    let source = root.join("A quiet song.wav");
    let source_bytes = write_wav(&source, 1);
    let original_source = source_bytes.clone();

    let first = service.import_selected_files(vec![source.clone()]).unwrap();
    assert_eq!(summary(&first), (1, 0, 0));
    assert_eq!(fs::read(&source).unwrap(), original_source);
    let page = service.list_library_page("all", 0).unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].title, "A quiet song");
    assert_eq!(page.items[0].availability.as_deref(), Some("available"));
    assert_eq!(page.items[0].format.as_deref(), Some("wav"));
    assert!(page.items[0].duration_seconds.unwrap() >= 1);

    let duplicate = service.import_selected_files(vec![source]).unwrap();
    assert_eq!(summary(&duplicate), (0, 1, 0));
    assert_eq!(service.list_library_page("all", 0).unwrap().total_count, 1);
    assert_eq!(fs::read(&root.join("managed/library/audio").join(format!("{}.wav", sha256_bytes(&original_source)))).unwrap(), original_source);

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn scanner_skips_non_audio_and_reports_missing_or_invalid_roots() {
    let root = test_directory();
    let service = make_service(&root);
    let folder = root.join("music");
    fs::create_dir_all(folder.join("nested")).unwrap();
    write_wav(&folder.join("valid.wav"), 1);
    fs::write(folder.join("notes.txt"), b"not audio").unwrap();
    write_wav(&folder.join("nested/also-valid.wav"), 2);

    let report = service.scan_folder(&folder).unwrap();
    assert_eq!(summary(&report), (2, 0, 0));
    assert_eq!(service.list_library_page("all", 0).unwrap().total_count, 2);
    assert!(service.scan_folder(&root.join("missing")).is_err());
    assert!(service.scan_folder(&folder.join("valid.wav")).is_err());

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn malformed_audio_and_unsupported_files_are_not_registered() {
    let root = test_directory();
    let service = make_service(&root);
    let malformed = root.join("not-audio.mp3");
    fs::write(&malformed, b"plain text").unwrap();
    let unsupported = root.join("notes.txt");
    fs::write(&unsupported, b"not audio").unwrap();

    let report = service.import_selected_files(vec![malformed, unsupported]).unwrap();
    assert_eq!(summary(&report), (0, 0, 2));
    assert_eq!(service.list_library_page("all", 0).unwrap().total_count, 0);

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn missing_managed_audio_is_reported_without_erasing_database_metadata() {
    let root = test_directory();
    let service = make_service(&root);
    let source = root.join("preserved.wav");
    let bytes = write_wav(&source, 1);
    service.import_selected_files(vec![source.clone()]).unwrap();
    let stored = root
      .join("managed/library/audio")
      .join(format!("{}.wav", sha256_bytes(&bytes)));
    fs::remove_file(stored).unwrap();

    let page = service.list_library_page("all", 0).unwrap();
    assert_eq!(page.total_count, 1);
    assert_eq!(page.items[0].availability.as_deref(), Some("missing"));
    assert!(source.exists());

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn playback_source_is_confined_to_managed_audio_and_missing_files_are_rejected() {
    let root = test_directory();
    let service = make_service(&root);
    let source = root.join("playback.wav");
    let bytes = write_wav(&source, 2);
    service.import_selected_files(vec![source]).unwrap();

    let page = service.list_library_page("all", 0).unwrap();
    let track_id = page.items[0].id;
    let resolved = service.resolve_playback_track(track_id).unwrap();
    let managed_audio = root.join("managed/library/audio").canonicalize().unwrap();
    assert!(resolved.audio_url.starts_with("http://127.0.0.1:"));
    assert!(resolved.audio_url.ends_with(&format!("{}.wav", sha256_bytes(&bytes))));
    assert_eq!(resolved.track.id, track_id);
    assert_eq!(resolved.track.format, "wav");

    let history_id = service.start_playback_history(track_id).unwrap();
    service.save_playback_position(history_id, 1).unwrap();
    let resume = service.resumable_playback().unwrap().unwrap();
    assert_eq!(resume.history_id, history_id);
    assert_eq!(resume.position_seconds, 1);
    assert_eq!(resume.track.title, "playback");

    fs::remove_file(managed_audio.join(format!("{}.wav", sha256_bytes(&bytes)))).unwrap();
    assert!(service.resolve_playback_track(track_id).unwrap_err().contains("missing"));
    assert!(service.resumable_playback().unwrap_err().contains("missing"));
    assert_eq!(sha256_bytes(&bytes).len(), 64);
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn legacy_migration_verifies_hash_is_repeatable_and_keeps_old_files_unchanged() {
    let root = test_directory();
    let service = make_service(&root);
    let source = root.join("legacy-source.wav");
    let source_bytes = write_wav(&source, 1);
    let legacy_root = root.join("offline-music-library");
    let digest = write_legacy_index(&legacy_root, &source, &source_bytes);
    let index_path = legacy_root.join("library.json");
    let index_before = fs::read(&index_path).unwrap();
    let audio_before = fs::read(legacy_root.join("audio").join(format!("{digest}.wav"))).unwrap();

    let first = service.migrate_legacy_from(&legacy_root).unwrap();
    assert_eq!(summary(&first), (1, 0, 0));
    let second = service.migrate_legacy_from(&legacy_root).unwrap();
    assert_eq!(summary(&second), (0, 1, 0));
    assert_eq!(service.list_library_page("all", 0).unwrap().items[0].subtitle, "Legacy artist");
    assert_eq!(fs::read(&index_path).unwrap(), index_before);
    assert_eq!(fs::read(legacy_root.join("audio").join(format!("{digest}.wav"))).unwrap(), audio_before);

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn legacy_migration_rejects_bad_checksum_without_copying_or_changing_source() {
    let root = test_directory();
    let service = make_service(&root);
    let source = root.join("legacy-source.wav");
    let source_bytes = write_wav(&source, 1);
    let legacy_root = root.join("offline-music-library");
    let digest = write_legacy_index(&legacy_root, &source, &source_bytes);
    let index_path = legacy_root.join("library.json");
    let audio_directory = legacy_root.join("audio");
    let original_audio_path = audio_directory.join(format!("{digest}.wav"));
    let incorrect_digest = "0".repeat(64);
    let incorrect_name = format!("{incorrect_digest}.wav");
    fs::copy(&original_audio_path, audio_directory.join(&incorrect_name)).unwrap();
    let original = fs::read(&index_path).unwrap();
    let mut entry: serde_json::Value = serde_json::from_slice(&original).unwrap();
    entry[0]["id"] = serde_json::Value::String(incorrect_digest);
    entry[0]["stored_name"] = serde_json::Value::String(incorrect_name.clone());
    fs::write(&index_path, serde_json::to_vec(&entry).unwrap()).unwrap();
    let tampered_index = fs::read(&index_path).unwrap();

    let report = service.migrate_legacy_from(&legacy_root).unwrap();
    assert_eq!(summary(&report), (0, 0, 1));
    assert_eq!(report.issues[0].message, "The previous library checksum does not match the audio file.");
    assert_eq!(service.list_library_page("all", 0).unwrap().total_count, 0);
    assert_eq!(fs::read(&index_path).unwrap(), tampered_index);
    assert_eq!(fs::read(original_audio_path).unwrap(), source_bytes);
    assert_eq!(fs::read(audio_directory.join(incorrect_name)).unwrap(), source_bytes);

    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn permission_errors_have_a_safe_user_facing_message() {
    let error = io::Error::from(io::ErrorKind::PermissionDenied);
    assert_eq!(io_message(&error), "Access was denied for this file or folder.");
  }
}