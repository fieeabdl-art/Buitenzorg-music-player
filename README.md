# BUITENZORG

**Buitenzorg Player — Music, Redefined.**

A local-first desktop music library and player. Ubuntu/Linux is the first target; the architecture also considers Windows and macOS. The Tauri/Vue shell, versioned SQLite storage, managed local imports, metadata scanning, legacy-library migration, local search, playlist editing, and local playback are implemented. YouTube Data API v3 search is implemented for a user-supplied key, but live search with a valid user key has not yet been verified; profile editing, YouTube playback, and downloads remain unavailable.

## Product principles

- No online login is required for local features.
- The local profile, playlists, and listening history stay on the device.
- Imported audio is copied into managed storage; the user's source file is never moved or deleted.
- Provider search, playback, and download are enabled only when official access and permissions are established.
- YouTube results are not direct audio URLs or permission to download.

## Metadata lokal dan riwayat

- Metadata tag lokal yang tersedia—termasuk genre, tahun, dan nomor track—disimpan bersama judul, artis, album, dan durasi. Artwork tertanam disimpan di direktori terkelola dan memiliki tampilan fallback jika berkas gambar tidak tersedia.
- Playlist lokal mendukung CRUD dan pengurutan lagu. Riwayat saat ini menyimpan posisi untuk melanjutkan pemutaran terakhir; daftar riwayat dan penghapusannya belum tersedia.

## Project

- Desktop application: [`desktop-app/`](desktop-app/README.md)
- Official repository: <https://github.com/fieeabdl-art/Buitenzorg-music-player>

## About

**Our Philosophy**

Music is more than something you play. It's something you experience.

Buitenzorg is built around that belief, combining thoughtful design, reliable technology, and a personal listening experience.

Every detail has a purpose. Every sound has its place.

**Buitenzorg — Music, Redefined.**

Project owner and developer: **Luthfie Abdul Hakim**

- Instagram: [@luthfhkm_](https://www.instagram.com/luthfhkm_/)
- GitHub: [@fieeabdl-art](https://github.com/fieeabdl-art)