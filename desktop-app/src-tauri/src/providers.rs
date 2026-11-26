use std::sync::Arc;
use std::time::Duration;

use keyring::Entry;
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};

use crate::db::{Database, StoredProviderStatus};

const PROVIDER: &str = "youtube";
const CREDENTIAL_SERVICE: &str = "com.buitenzorg.player";
const CREDENTIAL_USER: &str = "youtube-data-api-v3";
const SEARCH_ENDPOINT: &str = "https://youtube.googleapis.com/youtube/v3/search";
const VIDEOS_ENDPOINT: &str = "https://youtube.googleapis.com/youtube/v3/videos";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);
const MAX_QUERY_CHARS: usize = 120;
const RESULT_LIMIT: u8 = 20;

pub trait CredentialStore: Send + Sync {
  fn get_password(&self) -> Result<Option<String>, CredentialStoreError>;
  fn set_password(&self, password: &str) -> Result<(), CredentialStoreError>;
  fn delete_password(&self) -> Result<(), CredentialStoreError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialStoreError {
  Unavailable,
}

pub struct SystemCredentialStore;

impl SystemCredentialStore {
  fn entry() -> Result<Entry, CredentialStoreError> {
    Entry::new(CREDENTIAL_SERVICE, CREDENTIAL_USER).map_err(|_| CredentialStoreError::Unavailable)
  }
}

impl CredentialStore for SystemCredentialStore {
  fn get_password(&self) -> Result<Option<String>, CredentialStoreError> {
    match Self::entry()?.get_password() {
      Ok(password) => Ok(Some(password)),
      Err(keyring::Error::NoEntry) => Ok(None),
      Err(_) => Err(CredentialStoreError::Unavailable),
    }
  }

  fn set_password(&self, password: &str) -> Result<(), CredentialStoreError> {
    Self::entry()?
      .set_password(password)
      .map_err(|_| CredentialStoreError::Unavailable)
  }

  fn delete_password(&self) -> Result<(), CredentialStoreError> {
    match Self::entry()?.delete_credential() {
      Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
      Err(_) => Err(CredentialStoreError::Unavailable),
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderState {
  Available,
  Connected,
  Limited,
  Unavailable,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
  pub provider: String,
  pub state: ProviderState,
  pub has_api_key: bool,
  pub checked_at: Option<String>,
  pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchState {
  Ready,
  Empty,
  Unavailable,
  QuotaExceeded,
  InvalidKey,
  Timeout,
  NetworkError,
  ProviderError,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeVideo {
  pub video_id: String,
  pub title: String,
  pub artist: String,
  pub description: String,
  pub thumbnail_url: Option<String>,
  pub duration_seconds: Option<u64>,
  pub source_url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeSearchResponse {
  pub query: String,
  pub state: SearchState,
  pub items: Vec<YouTubeVideo>,
  pub message: Option<String>,
  pub provider_status: ProviderStatus,
}

#[derive(Clone)]
pub struct YouTubeProvider {
  database: Arc<Database>,
  credentials: Arc<dyn CredentialStore>,
  client: Client,
  search_endpoint: String,
  videos_endpoint: String,
}

impl YouTubeProvider {
  pub fn new(database: Database) -> Result<Self, String> {
    Self::with_credentials(database, Arc::new(SystemCredentialStore))
  }

  pub fn with_credentials(
    database: Database,
    credentials: Arc<dyn CredentialStore>,
  ) -> Result<Self, String> {
    let client = Client::builder()
      .timeout(REQUEST_TIMEOUT)
      .redirect(reqwest::redirect::Policy::none())
      .user_agent(concat!("BuitenzorgPlayer/", env!("CARGO_PKG_VERSION")))
      .build()
      .map_err(|_| "The provider HTTP client could not be initialized.".to_owned())?;

    Ok(Self {
      database: Arc::new(database),
      credentials,
      client,
      search_endpoint: SEARCH_ENDPOINT.to_owned(),
      videos_endpoint: VIDEOS_ENDPOINT.to_owned(),
    })
  }

  pub fn api_key(&self) -> Result<Option<String>, String> {
    self
      .credentials
      .get_password()
      .map_err(|_| "The operating system credential store is unavailable. Unlock or configure it, then try again.".to_owned())
  }

  pub fn status(&self) -> ProviderStatus {
    match self.credentials.get_password() {
      Ok(Some(_)) => self.status_with_key(true),
      Ok(None) => {
        let _ = self.database.set_provider_status(PROVIDER, "unavailable");
        ProviderStatus {
          provider: PROVIDER.to_owned(),
          state: ProviderState::Unavailable,
          has_api_key: false,
          checked_at: None,
          message: "Add a YouTube Data API key in Settings to search this source.".to_owned(),
        }
      }
      Err(CredentialStoreError::Unavailable) => ProviderStatus {
        provider: PROVIDER.to_owned(),
        state: ProviderState::Unavailable,
        has_api_key: false,
        checked_at: None,
        message: "The operating system credential store is unavailable. Unlock or configure it to use YouTube search.".to_owned(),
      },
    }
  }

  pub fn save_api_key(&self, api_key: &str) -> Result<ProviderStatus, String> {
    let api_key = api_key.trim();
    if !(20..=256).contains(&api_key.len()) || !api_key.is_ascii() || api_key.chars().any(char::is_whitespace) {
      return Err("The API key format is invalid. Check the key and try again.".to_owned());
    }
    self
      .credentials
      .set_password(api_key)
      .map_err(|_| "The operating system credential store is unavailable. No key was saved.".to_owned())?;
    self.database.set_provider_status(PROVIDER, "available")?;
    Ok(ProviderStatus {
      provider: PROVIDER.to_owned(),
      state: ProviderState::Available,
      has_api_key: true,
      checked_at: None,
      message: "API key saved in the operating system credential store. Verify it before searching.".to_owned(),
    })
  }

  pub fn remove_api_key(&self) -> Result<ProviderStatus, String> {
    self
      .credentials
      .delete_password()
      .map_err(|_| "The operating system credential store is unavailable. No key was removed.".to_owned())?;
    self.database.set_provider_status(PROVIDER, "unavailable")?;
    Ok(ProviderStatus {
      provider: PROVIDER.to_owned(),
      state: ProviderState::Unavailable,
      has_api_key: false,
      checked_at: None,
      message: "The YouTube API key was removed from the operating system credential store.".to_owned(),
    })
  }

  pub async fn search_with_key(&self, query: &str, api_key: Option<String>) -> YouTubeSearchResponse {
    let query = query.trim();
    if query.is_empty() {
      return self.response(query, SearchState::Empty, Vec::new(), None, None);
    }
    if query.chars().count() > MAX_QUERY_CHARS || query.chars().any(char::is_control) {
      return self.response(
        query,
        SearchState::ProviderError,
        Vec::new(),
        Some("Search terms must be 120 characters or fewer.".to_owned()),
        None,
      );
    }
    let Some(api_key) = api_key else {
      let _ = self.database.set_provider_status(PROVIDER, "unavailable");
      let status = ProviderStatus {
        provider: PROVIDER.to_owned(),
        state: ProviderState::Unavailable,
        has_api_key: false,
        checked_at: None,
        message: "Add a YouTube Data API key in Settings to search this source.".to_owned(),
      };
      return self.response(query, SearchState::Unavailable, Vec::new(), Some(status.message.clone()), Some(status));
    };

    let search_data = match self.fetch_search(query, &api_key, RESULT_LIMIT).await {
      Ok(data) => data,
      Err(failure) => return self.failure_response(query, failure),
    };

    let mut items = normalize_search_results(search_data.items);
    let mut warning = None;
    let video_ids = items.iter().map(|item| item.video_id.as_str()).collect::<Vec<_>>().join(",");
    if !video_ids.is_empty() {
      match self.fetch_durations(&video_ids, &api_key).await {
        Ok(durations) => {
          for item in &mut items {
            item.duration_seconds = durations.get(&item.video_id).copied();
          }
        }
        Err(_) => warning = Some("Search succeeded, but video durations are temporarily unavailable.".to_owned()),
      }
    }

    let state = if items.is_empty() { SearchState::Empty } else { SearchState::Ready };
    let message = warning.or_else(|| (state == SearchState::Empty).then(|| "YouTube returned no matching videos.".to_owned()));
    let _ = self.database.set_provider_status(PROVIDER, "connected");
    let status = self.connected_status(true, "YouTube search is connected.");
    self.response(query, state, items, message, Some(status))
  }

  pub async fn check_connection_with_key(&self, api_key: Option<String>) -> ProviderStatus {
    let response = self.search_with_key("music", api_key).await;
    response.provider_status
  }

  fn fetch_search<'a>(
    &'a self,
    query: &'a str,
    api_key: &'a str,
    max_results: u8,
  ) -> impl std::future::Future<Output = Result<SearchListResponse, ProviderFailure>> + 'a {
    async move {
      let response = youtube_request(&self.client, &self.search_endpoint, api_key)
        .query(&[
          ("part", "snippet"),
          ("q", query),
          ("type", "video"),
          ("videoEmbeddable", "true"),
          ("maxResults", &max_results.to_string()),
        ])
        .send()
        .await
        .map_err(ProviderFailure::from_request)?;
      parse_response(response).await
    }
  }

  async fn fetch_durations(&self, video_ids: &str, api_key: &str) -> Result<std::collections::HashMap<String, u64>, ProviderFailure> {
    let response = youtube_request(&self.client, &self.videos_endpoint, api_key)
      .query(&[("part", "contentDetails"), ("id", video_ids)])
      .send()
      .await
      .map_err(ProviderFailure::from_request)?;
    let data: VideosListResponse = parse_response(response).await?;
    Ok(data
      .items
      .into_iter()
      .filter_map(|item| parse_iso8601_duration(&item.content_details.duration).map(|duration| (item.id, duration)))
      .collect())
  }

  fn failure_response(&self, query: &str, failure: ProviderFailure) -> YouTubeSearchResponse {
    let (state, provider_state, message) = match failure.kind {
      FailureKind::QuotaExceeded => (
        SearchState::QuotaExceeded,
        Some("limited"),
        "YouTube Data API quota is exhausted. Check the Google Cloud quota or try again later.",
      ),
      FailureKind::InvalidKey => (
        SearchState::InvalidKey,
        Some("unavailable"),
        "YouTube rejected this API key. Check that YouTube Data API v3 is enabled and the key restrictions allow this app.",
      ),
      FailureKind::Timeout => (SearchState::Timeout, None, "YouTube search timed out. Check your connection and try again."),
      FailureKind::Network => (SearchState::NetworkError, None, "YouTube could not be reached. Check your internet connection."),
      FailureKind::Provider => (SearchState::ProviderError, None, "YouTube search is temporarily unavailable. Try again later."),
    };
    if let Some(state) = provider_state {
      let _ = self.database.set_provider_status(PROVIDER, state);
    }
    let status = if let Some(state) = provider_state {
      ProviderStatus {
        provider: PROVIDER.to_owned(),
        state: if state == "limited" { ProviderState::Limited } else { ProviderState::Unavailable },
        has_api_key: true,
        checked_at: None,
        message: message.to_owned(),
      }
    } else {
      self.status_with_key(true)
    };
    self.response(query, state, Vec::new(), Some(message.to_owned()), Some(status))
  }

  fn response(
    &self,
    query: &str,
    state: SearchState,
    items: Vec<YouTubeVideo>,
    message: Option<String>,
    provider_status: Option<ProviderStatus>,
  ) -> YouTubeSearchResponse {
    YouTubeSearchResponse {
      query: query.to_owned(),
      state,
      items,
      message,
      provider_status: provider_status.unwrap_or_else(|| self.status()),
    }
  }

  fn status_with_key(&self, has_api_key: bool) -> ProviderStatus {
    let stored = self.database.get_provider_status(PROVIDER).ok().flatten();
    status_from_storage(stored, has_api_key)
  }

  fn connected_status(&self, has_api_key: bool, message: &str) -> ProviderStatus {
    let stored = self.database.get_provider_status(PROVIDER).ok().flatten();
    ProviderStatus {
      provider: PROVIDER.to_owned(),
      state: ProviderState::Connected,
      has_api_key,
      checked_at: stored.and_then(|status| status.checked_at),
      message: message.to_owned(),
    }
  }
}

fn youtube_request(client: &Client, endpoint: &str, api_key: &str) -> RequestBuilder {
  client.get(endpoint).header("x-goog-api-key", api_key)
}

fn status_from_storage(stored: Option<StoredProviderStatus>, has_api_key: bool) -> ProviderStatus {
  let (state, checked_at) = match stored {
    Some(status) => {
      let state = match status.state.as_str() {
        "connected" => ProviderState::Connected,
        "limited" => ProviderState::Limited,
        "unavailable" if has_api_key => ProviderState::Unavailable,
        _ if has_api_key => ProviderState::Available,
        _ => ProviderState::Unavailable,
      };
      (state, status.checked_at)
    }
    None if has_api_key => (ProviderState::Available, None),
    None => (ProviderState::Unavailable, None),
  };
  let message = match state {
    ProviderState::Connected => "YouTube search is connected.",
    ProviderState::Limited => "YouTube search is limited because the API quota was exhausted.",
    ProviderState::Available => "API key is saved but has not been verified yet.",
    ProviderState::Unavailable if has_api_key => "The API key needs attention before YouTube search can run.",
    ProviderState::Unavailable => "Add a YouTube Data API key in Settings to search this source.",
  };
  ProviderStatus { provider: PROVIDER.to_owned(), state, has_api_key, checked_at, message: message.to_owned() }
}

fn normalize_search_results(items: Vec<SearchListItem>) -> Vec<YouTubeVideo> {
  items
    .into_iter()
    .filter_map(|item| {
      let video_id = item.id.video_id?;
      if !valid_video_id(&video_id) {
        return None;
      }
      let title = item.snippet.title.trim();
      if title.is_empty() {
        return None;
      }
      let description = item.snippet.description.trim();
      Some(YouTubeVideo {
        source_url: format!("https://www.youtube.com/watch?v={video_id}"),
        video_id,
        title: title.chars().take(240).collect(),
        artist: item.snippet.channel_title.trim().chars().take(160).collect(),
        description: description.chars().take(360).collect(),
        thumbnail_url: item.snippet.thumbnails.best_url().filter(|url| is_youtube_image_url(url)),
        duration_seconds: None,
      })
    })
    .collect()
}

fn valid_video_id(value: &str) -> bool {
  value.len() == 11 && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

fn is_youtube_image_url(value: &str) -> bool {
  value.starts_with("https://i.ytimg.com/") || value.starts_with("https://yt3.ggpht.com/")
}

fn parse_iso8601_duration(value: &str) -> Option<u64> {
  let mut total = 0_u64;
  let mut number = String::new();
  let mut in_time = false;
  let mut saw_component = false;
  for character in value.chars() {
    match character {
      'P' if number.is_empty() && !saw_component => continue,
      'T' if number.is_empty() && !in_time => in_time = true,
      '0'..='9' => number.push(character),
      unit => {
        let amount = number.parse::<u64>().ok()?;
        let multiplier = match (in_time, unit) {
          (false, 'D') => 86_400,
          (true, 'H') => 3_600,
          (true, 'M') => 60,
          (true, 'S') => 1,
          _ => return None,
        };
        total = total.checked_add(amount.checked_mul(multiplier)?)?;
        number.clear();
        saw_component = true;
      }
    }
  }
  if !number.is_empty() || !saw_component { None } else { Some(total) }
}

async fn parse_response<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T, ProviderFailure> {
  let status = response.status();
  if !status.is_success() {
    let error = response.json::<GoogleErrorResponse>().await.unwrap_or_default();
    return Err(ProviderFailure::from_status(status, error));
  }
  response.json().await.map_err(|_| ProviderFailure { kind: FailureKind::Provider })
}

#[derive(Clone, Copy)]
enum FailureKind {
  QuotaExceeded,
  InvalidKey,
  Timeout,
  Network,
  Provider,
}

struct ProviderFailure {
  kind: FailureKind,
}

impl ProviderFailure {
  fn from_request(error: reqwest::Error) -> Self {
    let kind = if error.is_timeout() {
      FailureKind::Timeout
    } else if error.is_connect() || error.is_request() {
      FailureKind::Network
    } else {
      FailureKind::Provider
    };
    Self { kind }
  }

  fn from_status(status: StatusCode, body: GoogleErrorResponse) -> Self {
    let reasons = body
      .error
      .map(|error| error.errors.into_iter().chain(error.details).map(|detail| detail.reason).collect::<Vec<_>>())
      .unwrap_or_default();
    let kind = if reasons.iter().any(|reason| {
      matches!(reason.as_str(), "quotaExceeded" | "dailyLimitExceeded" | "rateLimitExceeded" | "userRateLimitExceeded")
    }) {
      FailureKind::QuotaExceeded
    } else if reasons.iter().any(|reason| {
      matches!(reason.as_str(), "keyInvalid" | "accessNotConfigured" | "ipRefererBlocked" | "keyExpired" | "API_KEY_INVALID")
    }) {
      FailureKind::InvalidKey
    } else if status == StatusCode::TOO_MANY_REQUESTS {
      FailureKind::QuotaExceeded
    } else if status == StatusCode::UNAUTHORIZED {
      FailureKind::InvalidKey
    } else if status.is_server_error() {
      FailureKind::Network
    } else {
      FailureKind::Provider
    };
    Self { kind }
  }
}

#[derive(Default, Deserialize)]
struct GoogleErrorResponse {
  error: Option<GoogleError>,
}

#[derive(Deserialize)]
struct GoogleError {
  #[serde(default)]
  errors: Vec<GoogleErrorDetail>,
  #[serde(default)]
  details: Vec<GoogleErrorDetail>,
}

#[derive(Deserialize)]
struct GoogleErrorDetail {
  #[serde(default)]
  reason: String,
}

#[derive(Default, Deserialize)]
struct SearchListResponse {
  #[serde(default)]
  items: Vec<SearchListItem>,
}

#[derive(Default, Deserialize)]
struct SearchListItem {
  #[serde(default)]
  id: SearchItemId,
  #[serde(default)]
  snippet: SearchSnippet,
}

#[derive(Default, Deserialize)]
struct SearchItemId {
  #[serde(rename = "videoId")]
  video_id: Option<String>,
}

#[derive(Default, Deserialize)]
struct SearchSnippet {
  #[serde(default)]
  title: String,
  #[serde(rename = "channelTitle", default)]
  channel_title: String,
  #[serde(default)]
  description: String,
  #[serde(default)]
  thumbnails: ThumbnailSet,
}

#[derive(Default, Deserialize)]
struct ThumbnailSet {
  default: Option<Thumbnail>,
  medium: Option<Thumbnail>,
  high: Option<Thumbnail>,
  maxres: Option<Thumbnail>,
}

impl ThumbnailSet {
  fn best_url(&self) -> Option<String> {
    self.maxres.as_ref().or(self.high.as_ref()).or(self.medium.as_ref()).or(self.default.as_ref()).map(|image| image.url.clone())
  }
}

#[derive(Deserialize)]
struct Thumbnail {
  url: String,
}

#[derive(Default, Deserialize)]
struct VideosListResponse {
  #[serde(default)]
  items: Vec<VideoDetails>,
}

#[derive(Deserialize)]
struct VideoDetails {
  id: String,
  #[serde(rename = "contentDetails")]
  content_details: VideoContentDetails,
}

#[derive(Deserialize)]
struct VideoContentDetails {
  duration: String,
}

#[cfg(test)]
mod tests {
  use std::path::Path;
  use std::sync::Arc;

  use super::{
    FailureKind, GoogleErrorResponse, ProviderFailure, ProviderState, SearchState, StatusCode, SystemCredentialStore,
    YouTubeProvider, youtube_request,
  };
  use crate::db::Database;

  #[test]
  fn invalid_key_failure_preserves_saved_key_availability() {
    let database = Database::open(Path::new(":memory:")).unwrap();
    let provider = YouTubeProvider::with_credentials(database, Arc::new(SystemCredentialStore)).unwrap();

    let response = provider.failure_response("song", ProviderFailure { kind: FailureKind::InvalidKey });

    assert_eq!(response.state, SearchState::InvalidKey);
    assert_eq!(response.provider_status.state, ProviderState::Unavailable);
    assert!(response.provider_status.has_api_key);
  }

  #[test]
  fn api_key_is_sent_in_header_not_query_string() {
    let request = youtube_request(
      &reqwest::Client::new(),
      "https://youtube.googleapis.com/youtube/v3/search",
      "test-only-key",
    )
    .query(&[("q", "local test")])
    .build()
    .unwrap();

    assert_eq!(request.headers().get("x-goog-api-key").unwrap().to_str().unwrap(), "test-only-key");
    assert!(request.url().query_pairs().all(|(name, _)| name != "key"));
  }

  #[test]
  fn api_key_invalid_errorinfo_is_classified_as_invalid_key() {
    let response: GoogleErrorResponse = serde_json::from_str(
      r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","errors":[{"reason":"badRequest"}],"details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID","domain":"googleapis.com"},{"@type":"type.googleapis.com/google.rpc.LocalizedMessage","locale":"en-US","message":"API key not valid. Please pass a valid API key."}]}}"#,
    )
    .unwrap();
    let failure = ProviderFailure::from_status(StatusCode::BAD_REQUEST, response);

    assert!(matches!(failure.kind, FailureKind::InvalidKey));
  }
}