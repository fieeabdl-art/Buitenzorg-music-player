use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_HEADER_BYTES: usize = 16 * 1024;
const MAX_CONNECTIONS: usize = 8;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(20);
const AUDIO_EXTENSIONS: &[&str] = &["aac", "aif", "aiff", "flac", "m4a", "mp3", "mp4", "oga", "ogg", "opus", "wav"];

pub struct LocalMediaServer {
  address: SocketAddr,
  token: String,
  shutdown: Arc<AtomicBool>,
  worker: Option<JoinHandle<()>>,
}

struct ConnectionGuard(Arc<AtomicUsize>);

impl Drop for ConnectionGuard {
  fn drop(&mut self) {
    self.0.fetch_sub(1, Ordering::AcqRel);
  }
}

impl LocalMediaServer {
  pub fn start(audio_directory: &Path) -> Result<Self, String> {
    let audio_directory = audio_directory.canonicalize().map_err(|error| error.to_string())?;
    let mut random = [0_u8; 32];
    getrandom::fill(&mut random).map_err(|error| format!("Could not create a local media session token: {error}"))?;
    let token = random.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|error| error.to_string())?;
    listener.set_nonblocking(true).map_err(|error| error.to_string())?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let worker_shutdown = shutdown.clone();
    let active_connections = Arc::new(AtomicUsize::new(0));
    let worker_token = token.clone();
    let worker = thread::Builder::new()
      .name("buitenzorg-local-media".to_owned())
      .spawn(move || {
        while !worker_shutdown.load(Ordering::Acquire) {
          match listener.accept() {
            Ok((mut stream, _)) => {
              if active_connections.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
                active_connections.fetch_sub(1, Ordering::AcqRel);
                reject_connection(&mut stream, "503 Service Unavailable");
                continue;
              }
              let root = audio_directory.clone();
              let expected_token = worker_token.clone();
              let active = active_connections.clone();
              if thread::Builder::new()
                .name("buitenzorg-media-request".to_owned())
                .spawn(move || {
                  let _guard = ConnectionGuard(active);
                  serve_request(stream, &root, &expected_token, address.port());
                })
                .is_err()
              {
                active_connections.fetch_sub(1, Ordering::AcqRel);
              }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => thread::sleep(POLL_INTERVAL),
            Err(_) => break,
          }
        }
      })
      .map_err(|error| error.to_string())?;

    Ok(Self { address, token, shutdown, worker: Some(worker) })
  }

  pub fn url_for(&self, stored_name: &str) -> Result<String, String> {
    if !valid_stored_name(stored_name) {
      return Err("The managed audio file name is invalid.".to_owned());
    }
    Ok(format!("http://127.0.0.1:{}/{}/{}", self.address.port(), self.token, stored_name))
  }
}

impl Drop for LocalMediaServer {
  fn drop(&mut self) {
    self.shutdown.store(true, Ordering::Release);
    if let Some(worker) = self.worker.take() {
      let _ = worker.join();
    }
  }
}

fn serve_request(mut stream: TcpStream, root: &Path, expected_token: &str, port: u16) {
  let _ = stream.set_read_timeout(Some(CONNECTION_TIMEOUT));
  let _ = stream.set_write_timeout(Some(CONNECTION_TIMEOUT));
  let (request_line, headers) = match read_headers(&mut stream) {
    Ok(headers) => headers,
    Err(status) => {
      reject_connection(&mut stream, status);
      return;
    }
  };

  let mut request_parts = request_line.split_whitespace();
  let method = request_parts.next().unwrap_or_default();
  let target = request_parts.next().unwrap_or_default();
  let version = request_parts.next().unwrap_or_default();
  if !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
    reject_connection(&mut stream, "400 Bad Request");
    return;
  }
  if !matches!(method, "GET" | "HEAD") {
    reject_connection(&mut stream, "405 Method Not Allowed");
    return;
  }

  let mut host: Option<String> = None;
  let mut range = None;
  for header in headers {
    let Some((name, value)) = header.split_once(':') else { continue };
    if name.eq_ignore_ascii_case("host") {
      host = Some(value.trim().to_owned());
    } else if name.eq_ignore_ascii_case("range") {
      range = Some(value.trim().to_owned());
    }
  }
  let expected_host = format!("127.0.0.1:{port}");
  if host.as_deref() != Some(expected_host.as_str()) {
    reject_connection(&mut stream, "404 Not Found");
    return;
  }

  let Some((provided_token, stored_name)) = parse_route(target) else {
    reject_connection(&mut stream, "404 Not Found");
    return;
  };
  if !constant_time_eq(provided_token, expected_token) || !valid_stored_name(stored_name) {
    reject_connection(&mut stream, "404 Not Found");
    return;
  }

  let path = root.join(stored_name);
  let metadata = match fs::symlink_metadata(&path) {
    Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
    _ => {
      reject_connection(&mut stream, "404 Not Found");
      return;
    }
  };
  let canonical = match path.canonicalize() {
    Ok(path) if path.parent() == Some(root) => path,
    _ => {
      reject_connection(&mut stream, "404 Not Found");
      return;
    }
  };
  let total_length = metadata.len();
  let selected_range = match range.as_deref().map(|value| parse_range(value, total_length)) {
    Some(Ok(range)) => Some(range),
    Some(Err(())) => {
      let _ = write!(
        stream,
        "HTTP/1.1 416 Range Not Satisfiable\r\nAccept-Ranges: bytes\r\nContent-Range: bytes */{total_length}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
      );
      return;
    }
    None => None,
  };
  let (status, start, end) = selected_range
    .map(|(start, end)| ("206 Partial Content", start, end))
    .unwrap_or_else(|| ("200 OK", 0, total_length.saturating_sub(1)));
  let content_length = if total_length == 0 { 0 } else { end - start + 1 };
  let content_type = content_type(stored_name);
  let response = if selected_range.is_some() {
    format!(
      "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nAccept-Ranges: bytes\r\nContent-Range: bytes {start}-{end}/{total_length}\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
    )
  } else {
    format!(
      "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nAccept-Ranges: bytes\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n"
    )
  };
  if stream.write_all(response.as_bytes()).is_err() || method == "HEAD" || content_length == 0 {
    return;
  }
  let Ok(mut file) = File::open(canonical) else { return };
  if file.seek(SeekFrom::Start(start)).is_err() {
    return;
  }
  let _ = io::copy(&mut file.take(content_length), &mut stream);
}

fn read_headers(stream: &mut TcpStream) -> Result<(String, Vec<String>), &'static str> {
  let mut reader = BufReader::new(stream);
  let mut bytes = Vec::with_capacity(MAX_HEADER_BYTES);
  let mut chunk = [0_u8; 512];
  loop {
    let size = reader.read(&mut chunk).map_err(|_| "400 Bad Request")?;
    if size == 0 {
      return Err("400 Bad Request");
    }
    bytes.extend_from_slice(&chunk[..size]);
    if bytes.len() > MAX_HEADER_BYTES {
      return Err("431 Request Header Fields Too Large");
    }
    if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
      let text = std::str::from_utf8(&bytes[..end]).map_err(|_| "400 Bad Request")?;
      let mut lines = text.split("\r\n");
      let request_line = lines.next().unwrap_or_default().to_owned();
      let headers = lines.map(str::to_owned).collect();
      return Ok((request_line, headers));
    }
  }
}

fn parse_route(target: &str) -> Option<(&str, &str)> {
  if target.chars().any(|character| matches!(character, '?' | '#' | '%')) {
    return None;
  }
  let mut parts = target.strip_prefix('/')?.split('/');
  let token = parts.next()?;
  let stored_name = parts.next()?;
  if parts.next().is_some() || token.is_empty() || stored_name.is_empty() {
    return None;
  }
  Some((token, stored_name))
}

fn valid_stored_name(stored_name: &str) -> bool {
  let Some((hash, extension)) = stored_name.rsplit_once('.') else { return false };
  hash.len() == 64
    && hash.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    && AUDIO_EXTENSIONS.contains(&extension)
    && Path::new(stored_name).file_name().and_then(|name| name.to_str()) == Some(stored_name)
}

fn parse_range(value: &str, total_length: u64) -> Result<(u64, u64), ()> {
  if total_length == 0 || value.contains(',') {
    return Err(());
  }
  let range = value.strip_prefix("bytes=").ok_or(())?;
  let (start, end) = range.split_once('-').ok_or(())?;
  if start.is_empty() {
    let suffix = end.parse::<u64>().map_err(|_| ())?;
    if suffix == 0 {
      return Err(());
    }
    return Ok((total_length.saturating_sub(suffix), total_length - 1));
  }
  let start = start.parse::<u64>().map_err(|_| ())?;
  if start >= total_length {
    return Err(());
  }
  let end = if end.is_empty() {
    total_length - 1
  } else {
    end.parse::<u64>().map_err(|_| ())?.min(total_length - 1)
  };
  if end < start {
    return Err(());
  }
  Ok((start, end))
}

fn content_type(stored_name: &str) -> &'static str {
  match stored_name.rsplit_once('.').map(|(_, extension)| extension).unwrap_or_default() {
    "aac" => "audio/aac",
    "aif" | "aiff" => "audio/aiff",
    "flac" => "audio/flac",
    "m4a" | "mp4" => "audio/mp4",
    "mp3" => "audio/mpeg",
    "oga" | "ogg" | "opus" => "audio/ogg",
    "wav" => "audio/wav",
    _ => "application/octet-stream",
  }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
  if left.len() != right.len() {
    return false;
  }
  left.bytes().zip(right.bytes()).fold(0_u8, |difference, (left, right)| difference | (left ^ right)) == 0
}

fn reject_connection(stream: &mut TcpStream, status: &str) {
  let body = b"Not found";
  let _ = write!(
    stream,
    "HTTP/1.1 {status}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len(),
  );
  let _ = stream.write_all(body);
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::{Read, Write};
  use std::net::TcpStream;
  use std::path::PathBuf;
  use std::sync::atomic::{AtomicU64, Ordering};
  use std::time::{SystemTime, UNIX_EPOCH};

  static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

  fn test_directory() -> PathBuf {
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory = std::env::temp_dir().join(format!(
      "buitenzorg-media-server-test-{}-{suffix}-{}",
      std::process::id(),
      NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&directory).unwrap();
    directory
  }

  fn request(url: &str, method: &str, range: Option<&str>) -> String {
    let url = url.strip_prefix("http://127.0.0.1:").unwrap();
    let (port, path) = url.split_once('/').unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap())).unwrap();
    let range_header = range.map(|value| format!("Range: {value}\r\n")).unwrap_or_default();
    write!(
      stream,
      "{method} /{path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{range_header}Connection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    String::from_utf8(response).unwrap()
  }

  #[test]
  fn serves_only_authenticated_managed_files_with_byte_ranges() {
    let root = test_directory();
    let filename = format!("{}.wav", "a".repeat(64));
    fs::write(root.join(&filename), b"abcdefghij").unwrap();
    let server = LocalMediaServer::start(&root).unwrap();
    let url = server.url_for(&filename).unwrap();

    let partial = request(&url, "GET", Some("bytes=2-5"));
    assert!(partial.starts_with("HTTP/1.1 206 Partial Content\r\n"));
    assert!(partial.contains("Content-Range: bytes 2-5/10\r\n"));
    assert!(partial.ends_with("cdef"));

    let suffix = request(&url, "GET", Some("bytes=-3"));
    assert!(suffix.starts_with("HTTP/1.1 206 Partial Content\r\n"));
    assert!(suffix.ends_with("hij"));

    let invalid_range = request(&url, "GET", Some("bytes=10-"));
    assert!(invalid_range.starts_with("HTTP/1.1 416 Range Not Satisfiable\r\n"));

    let wrong_token = url.replace(&server.token, &"0".repeat(server.token.len()));
    assert!(request(&wrong_token, "GET", None).starts_with("HTTP/1.1 404 Not Found\r\n"));
    assert!(server.url_for(&format!("../{}", filename)).is_err());
    assert!(server.url_for(&format!("{}.txt", "b".repeat(64))).is_err());
    let address = server.address;
    drop(server);
    assert!(TcpListener::bind(address).is_ok());
    fs::remove_dir_all(root).unwrap();
  }

  #[test]
  fn denies_symlinked_audio_files_and_rejects_non_get_methods() {
    let root = test_directory();
    let outside = root.join("outside.wav");
    fs::write(&outside, b"protected").unwrap();
    let audio = root.join("audio");
    fs::create_dir(&audio).unwrap();
    let filename = format!("{}.wav", "c".repeat(64));
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, audio.join(&filename)).unwrap();
    let server = LocalMediaServer::start(&audio).unwrap();
    let url = server.url_for(&filename).unwrap();

    assert!(request(&url, "GET", None).starts_with("HTTP/1.1 404 Not Found\r\n"));
    let ordinary_file = format!("{}.wav", "d".repeat(64));
    fs::write(audio.join(&ordinary_file), b"data").unwrap();
    let method_url = server.url_for(&ordinary_file).unwrap();
    assert!(request(&method_url, "POST", None).starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    drop(server);
    fs::remove_dir_all(root).unwrap();
  }
}