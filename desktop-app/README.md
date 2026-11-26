# Buitenzorg Player desktop app

Tauri 2 + Vue 3 + TypeScript + Vite + Tailwind CSS + Pinia + Lucide Icons. The selected Stillpoint Chamber interface connects to a versioned local SQLite database and includes managed local audio playback plus optional YouTube Data API v3 search. Local profile editing, YouTube playback, and downloads are not implemented.

## Requirements

- Node.js 22.12 or newer and npm for Vite 8 and the frontend.
- Rust stable and Cargo installed through rustup. The Rust package declares a minimum Rust version of 1.90.
- Ubuntu/Linux: WebKitGTK 4.1 and the native development packages listed below.
- Windows: Rust's MSVC toolchain, Microsoft C++ Build Tools, and WebView2 Runtime, as described in the official Tauri prerequisites.

The project does not install system packages or Rust toolchains. The owner supplied Ubuntu prerequisites; native Tauri startup, database initialization, and local WAV playback have been verified in the Linux WebKitGTK app. Release packaging and Windows builds remain unverified.

### Ubuntu 24.04 native prerequisites

Install the packages required by [Tauri 2's Linux prerequisites](https://v2.tauri.app/start/prerequisites/) from a terminal:

```bash
sudo apt update
sudo apt install \
  build-essential \
  curl \
  file \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  pkg-config \
  wget
```

Install the minimal stable Rust toolchain using the official rustup installer. Review the installer first if you prefer not to pipe a downloaded script directly into a shell:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
```

Confirm that the toolchain and native libraries are discoverable:

```bash
rustup show active-toolchain
rustc --version
cargo --version
pkg-config --modversion webkit2gtk-4.1
pkg-config --modversion librsvg-2.0
```

Then, from the application directory, install the locked JavaScript dependencies and start the native development app:

```bash
cd /home/fiee/Buitenzorg-music-player/desktop-app
npm ci
npm run tauri:dev
```

Use `npm run tauri:build` to create release bundles after native development succeeds. A successful `npm run build` only validates the frontend; it does not validate Rust compilation, WebKitGTK integration, or packaging. The app currently requests all bundle targets, so platform-specific packaging requirements may also apply. This project does not install dependencies or alter system configuration.

## Development

Run these commands from `desktop-app/`:

```bash
npm install
npm run dev
```

The Vite preview uses browser mode and displays that desktop services are unavailable; it does not provide filesystem access, SQLite, or local audio playback. To run the Tauri window after native prerequisites are available:

```bash
npm run tauri:dev
```

The `tauri:dev` script applies a development-only CSP allowance for the local Vite server. Both CSPs allow media only from the app's loopback audio endpoint; production configuration is in `src-tauri/tauri.conf.json`.

## Checks

```bash
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

`npm run build` type-checks and builds only the frontend. `npm run tauri:build` also requires the Rust and native system prerequisites above; a successful frontend build is not evidence that the desktop application or installer builds.

## Local library

- The application initializes `buitenzorg-player.sqlite3` under Tauri's app-data directory and applies numbered SQL migrations using SQLite `user_version`.
- Imported audio is copied into the managed `library/audio/` folder; SHA-256 is calculated while copying and the final copy is verified before it is registered. Files are limited to 2 GiB and duplicate content is detected by hash. The source file is never moved, changed, or deleted.
- **Add audio** opens a native file picker; **Scan folder** recursively reads a folder selected by the user. Symlinks and non-audio files are skipped, permission and missing-file errors are reported, and folder traversal is capped. Metadata and duration are read where supported; embedded cover images with recognized formats are copied into `library/artwork/`.
- Tag genre, year, and track number are stored when present; missing or invalid values stay empty. Library, local Search, Home, playlist, and Player share the same metadata and artwork. Artwork uses an authenticated, tokenized loopback route and falls back to the Buitenzorg mark if its image cannot load.
- The **Import previous library** action reads the legacy `~/.local/share/offline-music-library/library.json` (or its `XDG_DATA_HOME` equivalent) and its `audio/` files without changing them. SHA-256 and confined filenames are checked; the migration can be repeated safely. Legacy `.webm` entries are recognized but skipped because the current metadata reader does not support that container.
- Playlists support local create, rename, delete, track add/remove, and ordering. Search checks actual local title, artist, album, and source filename metadata; it does not query any online provider.
- SQLite migrations and file operations are owned by Rust commands. The frontend receives records and import reports through Tauri IPC; it has no filesystem or shell capability. Provider credentials are not stored in SQLite.

## Pencarian YouTube

- Untuk menyiapkan key milik sendiri, pilih atau buat project di Google Cloud Console, aktifkan **YouTube Data API v3**, lalu buat API key. Batasi penggunaan key ke API tersebut, pantau kuota di Cloud Console, dan jangan membagikan atau memasukkan key ke repository.
- Buka **Discover** di aplikasi desktop, masukkan key pada field bertipe password, lalu pilih **Save API key** dan **Verify connection**. Key disimpan di credential store sistem operasi; key dapat diganti dengan menyimpan key baru atau dihapus lewat **Remove key**. Jika credential store tidak tersedia, aplikasi tidak menyimpan key dan menampilkan error.
- Aplikasi mengirim key melalui header HTTPS `x-goog-api-key`, bukan URL. Google merekomendasikan header untuk menghindari key masuk ke URL/log URL. Key melewati IPC lokal Tauri dan berada di memori proses selama request; perlindungan saat tersimpan mengikuti konfigurasi credential store OS dan tidak melindungi dari proses lokal yang sudah dikompromikan. Implementasi provider tidak mencatat key ke log, UI setelah penyimpanan, SQLite, atau file konfigurasi.
- Pencarian YouTube mengirim query yang diketik pengguna ke API resmi. Metadata pustaka, profil, dan riwayat dengar lokal tidak dikirim. Pencarian **Local library** tetap terpisah dan tidak memerlukan internet.
- Hasil YouTube dibedakan dari hasil lokal dan hanya menampilkan metadata serta membuka halaman resmi YouTube. Hasil tidak diimpor sebagai track lokal, tidak masuk ke audio engine, dan tidak diunduh.
- Pencarian memerlukan koneksi internet, API key valid, API aktif, dan kuota yang tersedia. Status loading, hasil kosong, key ditolak, kuota, timeout, jaringan, dan error provider ditampilkan tanpa mengganti hasil dengan data contoh.
- **Status verifikasi:** tes otomatis memakai mock atau fixture aman dan memverifikasi penanganan respons/error; pencarian live dengan key pengguna yang valid belum diverifikasi. Request pembanding menggunakan key palsu hanya membuktikan endpoint membedakan header key invalid dari header yang tidak ada, bukan bahwa integrasi live berhasil.
- **Uji live setelah key disiapkan:** di aplikasi desktop, simpan key dan pilih **Verify connection**; cari judul yang diketahui, pastikan hasil metadata nyata terbuka pada YouTube, lalu coba query yang tidak menghasilkan kecocokan. Jangan sengaja menghabiskan kuota. Hapus atau rotasi key melalui Cloud Console jika key terpapar; key yang dihapus dari aplikasi tetap perlu dicabut di Cloud Console bila ingin dinonaktifkan.

## Local playback

- The root application owns exactly one HTML5 Audio element. `PlayerStore` keeps it alive across route changes and owns the queue, playback status, position, volume, mute, shuffle, repeat, and history updates.
- Tracks from the managed library can be played, paused/resumed, stopped, sought, queued, skipped, shuffled, and repeated. Playback advances to the next queued item at end of track; repeat-one and repeat-all are supported. Space toggles playback, Left/Right seeks by five seconds, Up/Down changes volume, and N/P select next/previous when focus is not in an editable field.
- The most recent track and position are restored without autoplay. Position is persisted periodically and on pause/stop. Playback history and track metadata remain in local SQLite.
- WebKitGTK on Linux rejects media served by Tauri's custom `asset://` protocol even when the file is readable, so managed audio and album artwork use a small HTTP server bound only to `127.0.0.1`. It uses an OS-random per-process token, accepts only validated hash-based filenames within their managed directories, rejects symlinks and other methods, and supports single byte ranges for audio seeking. It does not expose a general filesystem endpoint or bind to a network interface.
- Codec support depends on the system WebView and its media plugins. Missing files, playback/decoder errors, and unsupported formats are shown in the player; a format is not assumed to be supported because its extension is recognized.
- Verification on Linux used a locally generated synthetic WAV fixture: the desktop WebView reported active playback, advanced current time, displayed the track and decoded duration, kept the same audio element playing across navigation, and returned to paused/zero on Stop. This does not verify every codec or Windows/macOS.

## Security and current scope

- The Tauri capability grants only core event listen/unlisten permissions required by the app-ready bridge; filesystem and shell permissions are not enabled.
- One persistent HTML5 audio engine is connected to local managed files through the restricted loopback range server; YouTube results do not enter this playback engine.
- SQLite local library, managed imports, read-only legacy migration, playlist CRUD/order, local search, and playback controls are implemented. Playback history currently records resume position for the latest track; a browsable history list and delete action are not yet implemented. Local profile creation/editing is not implemented yet.
- YouTube search is available only through the user's configured official API key; provider results are never fabricated. Downloads are disabled; unknown or denied rights must not be queued. YouTube is not an authorized offline-download source in this app.
- No music samples are shipped or shown as user content; synthetic items existed only in the design-selection previews.