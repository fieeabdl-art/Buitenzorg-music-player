mod commands;
mod db;
mod events;
mod library;
mod media_server;
mod providers;

use std::fs;
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
#[cfg(target_os = "linux")]
use std::path::PathBuf;

use tauri::{Manager, WebviewWindowBuilder};
use db::Database;
use library::LibraryService;
use providers::YouTubeProvider;

#[cfg(target_os = "windows")]
const YOUTUBE_APP_REFERER: &str = "https://com.buitenzorg.player";

#[cfg(target_os = "linux")]
const YOUTUBE_REFERER_EXTENSION: &[u8] = include_bytes!(env!("YOUTUBE_REFERER_EXTENSION_PATH"));

#[cfg(target_os = "linux")]
struct YoutubeExtensionDirectory(PathBuf);

#[cfg(target_os = "linux")]
impl Drop for YoutubeExtensionDirectory {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

#[cfg(target_os = "linux")]
fn prepare_youtube_extension() -> Result<PathBuf, std::io::Error> {
  let mut nonce = [0_u8; 16];
  getrandom::fill(&mut nonce).map_err(|error| std::io::Error::other(error.to_string()))?;
  let suffix = nonce.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
  let directory = std::env::temp_dir().join(format!("buitenzorg-player-yt-{}-{suffix}", std::process::id()));
  let mut directory_builder = fs::DirBuilder::new();
  directory_builder.mode(0o700).create(&directory)?;

  let result = (|| {
    let extension = directory.join("libbuitenzorg_youtube_referer.so");
    let mut file = fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .mode(0o500)
      .open(extension)?;
    file.write_all(YOUTUBE_REFERER_EXTENSION)?;
    Ok::<(), std::io::Error>(())
  })();
  if let Err(error) = result {
    let _ = fs::remove_dir_all(&directory);
    return Err(error);
  }
  Ok(directory)
}

#[cfg(target_os = "windows")]
fn install_youtube_referer_hook(window: &tauri::WebviewWindow) -> Result<(), String> {
  use std::sync::{Arc, Mutex};

  use webview2_com::{
    Microsoft::Web::WebView2::Win32::{COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT},
    WebResourceRequestedEventHandler, take_pwstr,
  };
  use windows::core::{HSTRING, PCWSTR};

  let hook_result = Arc::new(Mutex::new(None));
  let result_for_webview = Arc::clone(&hook_result);
  window.with_webview(move |webview| {
    let result = unsafe {
      let core = webview.controller().CoreWebView2().map_err(|error| error.to_string())?;
      let iframe_api_pattern = HSTRING::from("https://www.youtube.com/iframe_api*");
      core
        .AddWebResourceRequestedFilter(
          PCWSTR(iframe_api_pattern.as_ptr()),
          COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
        )
        .map_err(|error| error.to_string())?;
      let embed_pattern = HSTRING::from("https://www.youtube.com/embed/*");
      core
        .AddWebResourceRequestedFilter(
          PCWSTR(embed_pattern.as_ptr()),
          COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
        )
        .map_err(|error| error.to_string())?;

      let handler = WebResourceRequestedEventHandler::create(Box::new(|_, args| {
        if let Some(args) = args {
          let request = unsafe { args.Request()? };
          let uri = take_pwstr(unsafe { request.Uri()? });
          if uri.starts_with("https://www.youtube.com/iframe_api")
            || uri.starts_with("https://www.youtube.com/embed/")
          {
            let headers = unsafe { request.Headers()? };
            unsafe {
              headers.SetHeader(&HSTRING::from("Referer"), &HSTRING::from(YOUTUBE_APP_REFERER))?;
            }
          }
        }
        Ok(())
      }));
      let mut token = Default::default();
      core
        .add_WebResourceRequested(&handler, &mut token)
        .map_err(|error| error.to_string())?;
      Ok::<(), String>(())
    };
    if let Ok(mut result_slot) = result_for_webview.lock() {
      *result_slot = Some(result);
    }
  }).map_err(|error| error.to_string())?;
  hook_result
    .lock()
    .map_err(|_| "Could not initialize the YouTube request hook.".to_owned())?
    .take()
    .unwrap_or_else(|| Err("The YouTube request hook did not initialize.".to_owned()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      #[cfg(target_os = "linux")]
      let extension_directory = {
        let directory = prepare_youtube_extension()?;
        app.manage(YoutubeExtensionDirectory(directory.clone()));
        directory
      };

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

      let window_config = app.config().app.windows.iter().find(|window| window.label == "main")
        .ok_or_else(|| std::io::Error::other("The main window configuration is missing."))?;
      let builder = WebviewWindowBuilder::from_config(app.handle(), window_config)?;
      #[cfg(target_os = "linux")]
      let builder = builder.extensions_path(extension_directory);
      let _window = builder.build()?;
      #[cfg(target_os = "windows")]
      install_youtube_referer_hook(&_window).map_err(std::io::Error::other)?;
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
      commands::start_youtube_playback_history,
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
