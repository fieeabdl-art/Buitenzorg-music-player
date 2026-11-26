# Audit dan rancangan integrasi provider

**Tanggal audit:** 26 November 2026

**Cakupan:** pencarian YouTube yang sudah ada dan kandidat provider resmi berikutnya. Audit ini tidak mengimplementasikan provider baru.

## Ringkasan keputusan

- Tahap 5 **belum selesai**: pencarian YouTube dengan API key pengguna yang valid belum diverifikasi secara live.
- Pencarian YouTube memakai API resmi, menyimpan key di credential store sistem operasi, dan tidak menghubungkan hasil provider ke audio engine lokal. Pencarian pustaka lokal tetap menjadi jalur terpisah.
- Spotify Web API adalah kandidat awal paling masuk akal untuk **pencarian metadata saja**, tetapi belum layak diaktifkan untuk distribusi umum tanpa kelayakan kuota Spotify dan kajian kebijakan metadata. Apple Music API mempunyai kebutuhan pengelolaan signing key/token tambahan.
- Rekomendasi saat ini: jangan mulai integrasi provider baru. Selesaikan uji live YouTube dengan key milik pengguna, lalu minta persetujuan dan verifikasi prasyarat akses provider.

## Audit implementasi YouTube

| Area | Kondisi yang diaudit | Catatan |
| --- | --- | --- |
| Key dan penyimpanan | Field Discover bertipe password; backend memakai credential store OS. Key bisa diganti atau dihapus dan tidak disimpan ke SQLite. | Key melalui IPC lokal dan berada di memori proses selama request. Perlindungan saat tersimpan mengikuti konfigurasi credential store OS; ini tidak melindungi dari proses lokal yang telah dikompromikan. |
| Request API | Backend memanggil endpoint resmi `search.list` dan `videos.list`; key dikirim dalam header HTTPS `x-goog-api-key`, bukan query URL. | Request pembanding menggunakan key palsu mendapat `400 API_KEY_INVALID`; request tanpa header mendapat `403 ACCESS_DENIED`. Ini menguji penerimaan header saja, bukan pencarian live dengan key valid. |
| Kerahasiaan | Pencarian dan penyimpanan key tidak memiliki pemanggilan log pada jalur provider yang diaudit. Key tidak dikembalikan ke UI setelah disimpan atau ditulis ke file konfigurasi/database. | Google memperingatkan bahwa API key adalah bearer credential dan menyarankan agar key tidak ditanam dalam kode klien. Model key milik pengguna menghindari satu key bersama di binary, tetapi bukan pengganti backend credential proxy atau perlindungan terhadap malware lokal. Batasi key pada YouTube Data API v3, pantau pemakaian, dan cabut key yang terpapar. |
| Status dan error | Store/UI memiliki state loading, hasil kosong, key ditolak, kuota, timeout, jaringan, dan error provider. Request lebih baru menggantikan hasil request lama. | Parser kini membaca `error.errors[]` dan `error.details[]`, termasuk alasan `API_KEY_INVALID`. Key invalid tidak lagi disalahartikan sebagai error provider umum dan status tetap memperlihatkan key tersimpan agar dapat diganti/dihapus. |
| Hasil dan playback | Search/Discover memakai hasil metadata resmi untuk memutar video melalui YouTube IFrame Player API; halaman player dan kontrol standar tetap ditampilkan. | Hasil YouTube masuk ke antrean terpisah dari audio lokal, bukan URL stream atau file library. Download tetap ditolak oleh adapter; pemutaran live masih perlu diverifikasi di Tauri. |
| Pencarian lokal | Dijalankan oleh layanan pustaka lokal, bukan API provider. | Tetap berfungsi terpisah dan tidak memerlukan internet. |

### Status pengujian YouTube

- Tes store frontend menggunakan mock provider; tes Rust memakai fixture lokal. Tes baru mencakup key di header, key tidak berada di query URL, status key tersimpan ketika ditolak, serta parsing `API_KEY_INVALID` dari bentuk respons Google.
- Request publik yang memakai nilai key palsu hanya memeriksa perbedaan respons header/tanpa header. Tidak ada key valid yang diberikan, dibuat, atau digunakan; tidak ada klaim pencarian live berhasil.
- Uji live yang dapat dilakukan setelah pemilik memasukkan key sendiri tercatat pada bagian **Pencarian YouTube** di `desktop-app/README.md`. Jangan membagikan key di issue, chat, log, atau repository.
- Simpan key di Cloud Console dengan pembatasan API ke YouTube Data API v3 dan pantau kuotanya. Pembatasan aplikasi tertentu bergantung pada opsi yang didukung Google Cloud untuk klien desktop; jangan menganggap referrer web cocok untuk aplikasi native.

## Perbandingan provider resmi

| Provider | API pencarian dan data | Autentikasi/kuota | Hak, playback, dan desktop |
| --- | --- | --- | --- |
| **Spotify Web API** | Endpoint resmi Search mengembalikan metadata katalog (misalnya track, album, artist, dan playlist) serta artwork/tautan Spotify. | Authorization Code dengan PKCE sesuai untuk klien publik yang tidak dapat menyimpan client secret. Development Mode mensyaratkan akun Premium pada pemilik app dan membatasi app ke 5 pengguna terdaftar. Ada rate limit `429` dengan `Retry-After`, serta kuota Development Mode terpisah. | Kebijakan mewajibkan atribusi Spotify dan tautan balik untuk metadata/artwork, serta melarang penawaran metadata/artwork sebagai layanan mandiri. Web Playback SDK bukan bagian dari rancangan ini; kebutuhan Premium dan uji WebView Tauri Linux/Windows akan menjadi prasyarat terpisah jika playback kelak dipertimbangkan. |
| **Apple Music API** | REST API resmi dapat mencari katalog berdasarkan storefront dan menyediakan metadata/artwork; cakupan juga mencakup data personal setelah otorisasi pengguna. | Setiap request API memerlukan developer token JWT bertanda tangan ES256 yang bergantung pada private key Apple Developer. Data personal memerlukan Music User Token. Apple menyatakan rate limit dapat menghasilkan `429`; angka limit publik tidak dicantumkan sebagai kontrak tetap di dokumentasi yang ditinjau. | Kunci penandatangan developer tidak boleh ditanam di binary desktop; perlu token issuer yang dikelola aman atau model kredensial milik pengguna yang belum dirancang. Dukungan playback resmi dan perilaku login di WebView Linux/Windows belum dibuktikan dalam audit ini. |

### Kesimpulan kandidat

Spotify lebih sederhana untuk rancangan pencarian metadata karena mendukung klien publik dengan PKCE. Namun, batas 5 pengguna Development Mode dan syarat Premium pada pemilik app tidak cocok untuk distribusi umum; batas kebijakan metadata mandiri juga perlu ditafsirkan terhadap pengalaman Discover Buitenzorg. Integrasi baru hanya patut dipertimbangkan setelah pemilik app memperoleh mode kuota yang sesuai, menyetujui ketentuan atribusi/link, dan pengujian redirect OAuth Linux serta Windows direncanakan.

Apple Music tetap alternatif, bukan pilihan implementasi berikutnya saat ini. Kebutuhan private key penandatangan membuat integrasi desktop yang didistribusikan memerlukan desain layanan token yang aman atau alur kredensial pengguna yang belum disetujui. Tidak ada layanan lain yang dinyatakan layak tanpa audit API resmi dan persyaratan lisensinya sendiri.

## Rancangan teknis bersyarat: Spotify metadata-only

Rancangan ini adalah proposal, bukan persetujuan implementasi.

1. **Batas fitur:** hanya pencarian katalog; tampilkan judul, artis, album, artwork jika sesuai kebijakan, dan URL resmi Spotify. Tampilkan atribusi dan tautan balik pada setiap hasil. Jangan meminta audio stream, mengunduh, atau memasukkan hasil ke `PlayerStore`/audio engine lokal.
2. **Autentikasi:** gunakan Authorization Code dengan PKCE tanpa client secret dalam binary. Redirect callback (loopback atau deep link) harus dipilih setelah memeriksa aturan registrasi Spotify terkini dan diuji di Tauri WebView Linux serta Windows. Simpan token akses/refresh di credential store OS, bukan SQLite/log/config; tangani pencabutan dan kegagalan refresh.
3. **Pemisahan provider:** buat adapter dan tipe hasil Spotify sendiri dengan identitas provider serta URL eksternal. Pertahankan pencarian pustaka lokal sebagai jalur offline mandiri. Jangan memaksa hasil Spotify memenuhi tipe `YouTubeVideo` atau resolusi playback YouTube.
4. **Jaringan dan kuota:** panggil hanya Spotify Web API resmi; batasi request, timeout, dan hasil; pada `429` hormati `Retry-After`, bedakan limit rate dari kuota Development Mode, dan tampilkan state offline/auth/limit secara jujur. Jangan menambah cache tanpa memastikan ketentuan penyimpanan Spotify.
5. **Syarat mulai:** pemilik menyetujui scope metadata-only dan kewajiban atribusi/link; pemilik app mengonfirmasi status Premium serta akses kuota yang sesuai untuk distribusi; dan redirect OAuth Linux/Windows telah dibuktikan. Sampai syarat ini disepakati, jangan membuat command Rust, adapter, skema token, atau UI provider baru.

## Roadmap dan pekerjaan yang menunggu

- **Tahap 5:** masih berlangsung. Pemilik dapat menguji key yang dibuatnya sendiri melalui checklist README; setelah itu putuskan apakah persyaratan Spotify dapat dipenuhi atau provider ditunda.
- **Tahap 6:** fitur playlist lokal sudah mencakup CRUD dan pengurutan menurut implementasi/dokumentasi yang ada. Audit lanjutan metadata, artwork, dan konsistensi riwayat belum dilakukan pada pekerjaan ini.
- **Tahap 7–10:** pengujian stabilitas/performa, rilis Linux, dan audit/build Windows masih menjadi pekerjaan berikutnya; tidak diklaim selesai oleh audit provider ini.

## Sumber resmi yang ditinjau

- [Praktik terbaik kunci API Google Cloud](https://docs.cloud.google.com/docs/authentication/api-keys-best-practices)
- [YouTube Data API: Search: list](https://developers.google.com/youtube/v3/docs/search/list), [Videos: list](https://developers.google.com/youtube/v3/docs/videos/list), dan [biaya kuota](https://developers.google.com/youtube/v3/determine_quota_cost)
- [Spotify Search API](https://developer.spotify.com/documentation/web-api/reference/search), [PKCE](https://developer.spotify.com/documentation/web-api/tutorials/code-pkce-flow), [mode kuota](https://developer.spotify.com/documentation/web-api/concepts/quota-modes), [rate limit](https://developer.spotify.com/documentation/web-api/concepts/rate-limits), [perubahan Development Mode 2026](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide), dan [kebijakan developer](https://developer.spotify.com/policy)
- [Apple Music API](https://developer.apple.com/documentation/applemusicapi), [pencarian katalog](https://developer.apple.com/documentation/applemusicapi/search-for-catalog-resources-(by-type)), [developer token](https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens), dan [autentikasi MusicKit](https://developer.apple.com/documentation/applemusicapi/user-authentication-for-musickit)