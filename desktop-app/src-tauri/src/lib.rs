mod commands;
mod db;
mod events;
mod library;
mod media_server;
mod providers;

use std::fs;

use tauri::Manager;
use db::Database;
use library::LibraryService;
use providers::YouTubeProvider;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_opener::init())
    .setup(|app| {
      let app_data_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| std::io::Error::other(error.to_string()))?;
      fs::create_dir_all(&app_data_directory)?;
      let database = Database::open(&app_data_directory.join("buitenzorg-player.sqlite3"))
        .map_err(std::io::Error::other)?;
      let provider = YouTubeProvider::new(database.clone()).map_err(std::io::Error::other)?;
      let library = LibraryService::new(database, app_data_directory.join("library"))
        .map_err(std::io::Error::other)?;
      app.manage(library);
      app.manage(provider);
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::get_app_info,
      commands::get_library_page,
      commands::search_local_library,
      commands::import_audio_files,
      commands::scan_audio_folder,
      commands::migrate_legacy_library,
      commands::get_library_storage_info,
      commands::resolve_track_for_playback,
      commands::start_playback_history,
      commands::update_playback_history,
      commands::get_last_playback,
      commands::get_playback_history,
      commands::delete_playback_history,
      commands::clear_playback_history,
      commands::get_youtube_provider_status,
      commands::save_youtube_api_key,
      commands::remove_youtube_api_key,
      commands::check_youtube_connection,
      commands::search_youtube,
      commands::open_youtube_video,
      commands::list_playlists,
      commands::get_playlist_tracks,
      commands::create_playlist,
      commands::update_playlist,
      commands::delete_playlist,
      commands::add_track_to_playlist,
      commands::remove_track_from_playlist,
      commands::reorder_playlist_track
    ])
    .run(tauri::generate_context!())
    .expect("error while building tauri application");
}
