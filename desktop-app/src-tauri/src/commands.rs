use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::events;
use crate::db::{LibraryPage, PlaylistSummary, PlaylistTrack};
use crate::library::{ImportReport, LibraryService, LibraryStorageInfo, ResolvedPlaybackTrack, ResumablePlayback};
use crate::providers::{ProviderStatus, YouTubeProvider, YouTubeSearchResponse};
use tauri_plugin_opener::OpenerExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
  name: String,
  version: String,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
  let info = AppInfo {
    name: "Buitenzorg Player".to_owned(),
    version: env!("CARGO_PKG_VERSION").to_owned(),
  };

  let _ = app.emit(events::APP_READY, &info);
  info
}

#[tauri::command]
pub async fn get_library_page(
  state: State<'_, LibraryService>,
  category: String,
  offset: usize,
) -> Result<LibraryPage, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.list_library_page(&category, offset))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn search_local_library(
  state: State<'_, LibraryService>,
  query: String,
  offset: usize,
) -> Result<LibraryPage, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.search_local_library(&query, offset))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn import_audio_files(app: AppHandle, state: State<'_, LibraryService>) -> Result<ImportReport, String> {
  const AUDIO_EXTENSIONS: &[&str] = &["aac", "aif", "aiff", "flac", "m4a", "mp3", "oga", "ogg", "opus", "wav"];
  let library = state.inner().clone();
  let (sender, receiver) = std::sync::mpsc::channel();
  app
    .dialog()
    .file()
    .add_filter("Audio files", AUDIO_EXTENSIONS)
    .pick_files(move |paths| {
      let _ = sender.send(paths);
    });
  let selected = tauri::async_runtime::spawn_blocking(move || receiver.recv())
    .await
    .map_err(|error| error.to_string())?
    .map_err(|_| "The file selection dialog did not return a result.".to_owned())?;
  let Some(selected) = selected else {
    return Ok(ImportReport { cancelled: true, ..ImportReport::default() });
  };
  let paths = selected.into_iter().map(file_path_to_path).collect::<Result<Vec<_>, _>>()?;
  tauri::async_runtime::spawn_blocking(move || library.import_selected_files(paths))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn scan_audio_folder(app: AppHandle, state: State<'_, LibraryService>) -> Result<ImportReport, String> {
  let library = state.inner().clone();
  let (sender, receiver) = std::sync::mpsc::channel();
  app.dialog().file().pick_folder(move |path| {
    let _ = sender.send(path);
  });
  let selected = tauri::async_runtime::spawn_blocking(move || receiver.recv())
    .await
    .map_err(|error| error.to_string())?
    .map_err(|_| "The folder selection dialog did not return a result.".to_owned())?;
  let Some(selected) = selected else {
    return Ok(ImportReport { cancelled: true, ..ImportReport::default() });
  };
  let path = file_path_to_path(selected)?;
  tauri::async_runtime::spawn_blocking(move || library.scan_folder(&path))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn migrate_legacy_library(state: State<'_, LibraryService>) -> Result<ImportReport, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.migrate_legacy_library())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_library_storage_info(state: State<'_, LibraryService>) -> Result<LibraryStorageInfo, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.storage_info())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn resolve_track_for_playback(
  state: State<'_, LibraryService>,
  track_id: i64,
) -> Result<ResolvedPlaybackTrack, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.resolve_playback_track(track_id))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn start_playback_history(state: State<'_, LibraryService>, track_id: i64) -> Result<i64, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.start_playback_history(track_id))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn update_playback_history(
  state: State<'_, LibraryService>,
  history_id: i64,
  position_seconds: i64,
  listened_seconds: i64,
  status: String,
) -> Result<(), String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || {
    library.update_playback_history(history_id, position_seconds, listened_seconds, &status)
  })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_playback_history(state: State<'_, LibraryService>) -> Result<Vec<crate::db::PlaybackHistoryItem>, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.playback_history())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn delete_playback_history(state: State<'_, LibraryService>, history_id: i64) -> Result<(), String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.delete_playback_history(history_id))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn clear_playback_history(state: State<'_, LibraryService>) -> Result<usize, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.clear_playback_history())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_last_playback(state: State<'_, LibraryService>) -> Result<Option<ResumablePlayback>, String> {
  let library = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || library.resumable_playback())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn get_youtube_provider_status(state: State<'_, YouTubeProvider>) -> Result<ProviderStatus, String> {
  let provider = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || provider.status())
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_youtube_api_key(
  state: State<'_, YouTubeProvider>,
  api_key: String,
) -> Result<ProviderStatus, String> {
  let provider = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || provider.save_api_key(&api_key))
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn remove_youtube_api_key(state: State<'_, YouTubeProvider>) -> Result<ProviderStatus, String> {
  let provider = state.inner().clone();
  tauri::async_runtime::spawn_blocking(move || provider.remove_api_key())
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn check_youtube_connection(state: State<'_, YouTubeProvider>) -> Result<ProviderStatus, String> {
  let provider = state.inner().clone();
  let key_provider = provider.clone();
  let api_key = tauri::async_runtime::spawn_blocking(move || key_provider.api_key())
    .await
    .map_err(|error| error.to_string())??;
  Ok(provider.check_connection_with_key(api_key).await)
}

#[tauri::command]
pub async fn search_youtube(
  state: State<'_, YouTubeProvider>,
  query: String,
) -> Result<YouTubeSearchResponse, String> {
  let provider = state.inner().clone();
  let key_provider = provider.clone();
  let api_key = tauri::async_runtime::spawn_blocking(move || key_provider.api_key())
    .await
    .map_err(|error| error.to_string())
    .and_then(|result| result)?;
  Ok(provider.search_with_key(&query, api_key).await)
}

#[tauri::command]
pub fn open_youtube_video(app: AppHandle, video_id: String) -> Result<(), String> {
  if video_id.len() != 11
    || !video_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
  {
    return Err("The YouTube video identifier is invalid.".to_owned());
  }
  app
    .opener()
    .open_url(format!("https://www.youtube.com/watch?v={video_id}"), None::<&str>)
    .map_err(|_| "Could not open the official YouTube page in your browser.".to_owned())
}

#[tauri::command]
pub fn list_playlists(state: State<'_, LibraryService>) -> Result<Vec<PlaylistSummary>, String> {
  state.list_playlists()
}

#[tauri::command]
pub fn get_playlist_tracks(state: State<'_, LibraryService>, playlist_id: i64) -> Result<Vec<PlaylistTrack>, String> {
  state.playlist_tracks(playlist_id)
}

#[tauri::command]
pub fn create_playlist(state: State<'_, LibraryService>, name: String, description: String) -> Result<i64, String> {
  state.create_playlist(&name, &description)
}

#[tauri::command]
pub fn update_playlist(
  state: State<'_, LibraryService>,
  playlist_id: i64,
  name: String,
  description: String,
) -> Result<(), String> {
  state.update_playlist(playlist_id, &name, &description)
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, LibraryService>, playlist_id: i64) -> Result<(), String> {
  state.delete_playlist(playlist_id)
}

#[tauri::command]
pub fn add_track_to_playlist(state: State<'_, LibraryService>, playlist_id: i64, track_id: i64) -> Result<(), String> {
  state.add_track_to_playlist(playlist_id, track_id)
}

#[tauri::command]
pub fn remove_track_from_playlist(
  state: State<'_, LibraryService>,
  playlist_id: i64,
  track_id: i64,
) -> Result<(), String> {
  state.remove_track_from_playlist(playlist_id, track_id)
}

#[tauri::command]
pub fn reorder_playlist_track(
  state: State<'_, LibraryService>,
  playlist_id: i64,
  track_id: i64,
  position: usize,
) -> Result<(), String> {
  state.reorder_playlist_track(playlist_id, track_id, position)
}

fn file_path_to_path(path: FilePath) -> Result<std::path::PathBuf, String> {
  path.into_path().map_err(|_| "The selected location is not a readable local path.".to_owned())
}