# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Stack

Tauri 2, Vue 3, TypeScript, Vite, Tailwind CSS, Pinia, Lucide Icons, Rust, and SQLite, as specified by the owner.

## Users

The initial user is the project owner using a private local profile. Ubuntu/Linux is the primary target; Windows should also be supported, and the architecture should remain portable to macOS.

## Product Purpose

Buitenzorg Player is a local-first desktop music library and player. It helps the owner organize and play local audio offline, while exposing online providers only where official APIs, playback methods, and permissions support them.

## Positioning

One desktop application combines a managed local library, local playlists, and an embedded persistent player without requiring an online account or sending profile and listening data to external services.

## Operating Context

The application is used on Ubuntu/Linux first and should account for Windows filesystem, credential-storage, and packaging differences. Local library and playback functions must remain usable without internet access.

## Capabilities and Constraints

- First launch creates a local profile. Display name is required; username and profile photo are optional and editable in Settings.
- SQLite owns profile, track metadata, playlists, liked songs, playback history, settings, and provider status. Audio and artwork files live in the app-managed filesystem.
- Imports copy files into managed storage, verify the copy, deduplicate by SHA-256, and never move or delete user source files.
- One persistent HTML5 Audio element is managed by a root Pinia PlayerStore. YouTube playback uses the official embedded player separately; YouTube pages are never treated as direct audio URLs or as permission to download.
- Provider search, playback, and download are enabled only when official access and permissions are established. Unknown or denied download eligibility fails closed.
- API credentials are stored with OS secure credential storage; they are not embedded in source or ordinary settings records.
- No MPV playback process, DRM bypass, unauthorized extraction, online login, or external transmission of the local profile, playlists, or listening history.

## Brand Commitments

- Brand: BUITENZORG. Product name: Buitenzorg Player. Tagline: Music, Redefined.
- Owner and developer: Luthfie Abdul Hakim. Instagram: `@luthfhkm_`. GitHub: `@fieeabdl-art`.
- Premium, minimal, modern technology identity using midnight navy, charcoal, black, silver, and soft blue, with an original audio-waveform symbol and BUITENZORG wordmark. Do not imitate Spotify or Apple Music and do not use Bogor tourism/history as the main UI theme.
- Settings → About must show the following philosophy verbatim:

  **Our Philosophy**

  Music is more than something you play. It's something you experience.

  Buitenzorg is built around that belief, combining thoughtful design, reliable technology, and a personal listening experience.

  Every detail has a purpose. Every sound has its place.

  **Buitenzorg — Music, Redefined.**

## Evidence on Hand

The previous Spicetify/Python project exists at `/home/fiee/spotify-offline-playback-manager` and is read-only for this work unless a migration is explicitly implemented. No prior Buitenzorg worktree or implementation was found; the official GitHub repository was empty when cloned. No provider credentials or user-supplied artwork are available.

## Product Principles

- Local data and offline playback remain useful without a network connection.
- Never imply access, playback, or download permissions that a provider does not grant.
- Prefer clear recovery guidance over demo content or fake success states.
- Keep the owner’s local files and profile under their control.