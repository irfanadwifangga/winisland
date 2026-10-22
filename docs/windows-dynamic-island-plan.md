# Windows Dynamic Island — Rencana & Panduan Setup

Dokumen ini dibuat untuk dibawa ke sesi baru. Bagian 0 adalah ringkasan konteks yang bisa langsung ditempel.

---

## 0. Ringkasan konteks (tempel di sesi baru)

**Cara memakai di sesi baru:** unggah dokumen ini, tempel ulang blok PERAN/ATURAN (instruktur kritis, analisis pro-kontra, struktur jawaban a-e) karena sesi baru tidak mengingatnya, lalu minta Claude menarik repo dan membaca kodenya dulu sebelum menulis apa pun.

- **Proyek:** `winisland`, overlay ala Dynamic Island untuk Windows. Repo publik: <https://github.com/irfanadwifangga/winisland>. Lisensi MIT (atribusi Slint Royalty-free 2.0 lewat badge README). Dipakai pribadi dulu, kontributor diharapkan.
- **Prioritas:** RAM idle terkecil. Metrik resmi: **private working set**, commit sebagai pembanding. **Tidak ada anggaran keras** (diputuskan 21 Okt 2026); angka dicatat per fitur sebagai informasi dan pendeteksi lonjakan. CPU idle harus tetap 0%. Ukuran `.exe` sekunder.
- **Arah teknis:** Rust native, Slint 1.18.1, renderer software dikunci lewat kode (`BackendSelector`), femtovg dibuang, integrasi Windows lewat crate `windows` 0.62.2. Pembangunan pakai profil `dist`.
- **Pengalaman:** pertama kali memakai Rust. Shell PowerShell, editor VS Code. Teks UI berbahasa Inggris, komentar kode berbahasa Indonesia.
- **Status (22 Okt 2026): langkah 0-11 selesai, ditambah fitur setelahnya:**
  - Overlay tersembunyi dari taskbar dan Alt+Tab, idle berupa garis tipis di tepi atas (140x4), expand saat hover, ikon tray (Show/Hide, Autostart, tampilan isi notifikasi, pilihan monitor, hide saat fullscreen, Quit), autostart lewat registry.
  - Media via event SMTC (worker thread, debounce 150 ms, dedup) dengan tombol previous/play-pause/next.
  - Notifikasi: dibaca saat pill expanded (tanpa event, tanpa polling), lencana jumlah, tampilan notifikasi bergantian dengan media, otomatis beralih ke notifikasi bila tidak ada media (`8ab082e`), sakelar privasi isi pesan.
  - Multi-monitor dan DPI (`12da5b5`), auto-hide saat aplikasi fullscreen (`628f719`); keduanya lolos uji pengguna.
- **Angka terkini (build `dist`):** `.exe` sekitar 7,45 MB; private working set idle sekitar 2,9 MB tanpa fetch notifikasi, sekitar 4,3 MB setelah fetch notifikasi pertama. Hasil bisect (22 Okt 2026) membuktikan seluruh kenaikan +1,4 MB berasal dari jalur WinRT pembacaan notifikasi. **Pengguna menerima kenaikan itu.** CPU idle 0%.
- **Keputusan yang sudah diambil:** software renderer; media lewat event SMTC dengan worker permanen; notifikasi dibaca saat expanded karena `NotificationChanged` gagal (0x80070490) pada aplikasi unpackaged; privasi default nama aplikasi + judul tanpa isi; satu proses tray (bukan Windows Service); jendela mengikuti state (idle = garis tipis).
- **Fase 2 (dimulai 22 Okt 2026, lihat Bagian 9):** state **Home** (jam, notifikasi 1 baris, statistik sistem), tab navigasi (Kalender, Timer/Stopwatch/Jadwal, Email atau widget berguna lain), dan **ukuran island dinamis** mengikuti konten. UI dan font yang kurang rapi **sengaja ditunda ke backlog**. Perencanaan RAM untuk fitur Fase 2 diabaikan dulu; diukur setelah fitur jalan.
- **Pekerjaan terbuka:**
  1. Selesai (22 Okt 2026): tiga keputusan di 9.6 (media di dalam Home, mengingat tab terakhir di memori, jadwal di berkas teks `%APPDATA%`). Langkah 12 siap dimulai.
  2. Langkah 12-17 (Bagian 9).
  3. Backlog (9.8), termasuk `cargo-deny` sebelum rilis biner pertama.
- **Berkas lokal yang sengaja belum di-commit:** `run-all.ps1`, `measure.ps1`, `cleanup.ps1`.
- **Cara kerja:** satu kelompok perubahan kecil, verifikasi dengan checklist, commit dengan pesan yang hanya mengklaim yang sudah dilaporkan, perbarui dokumen ini, baru lanjut. Claude tidak bisa mengompilasi (sandbox bukan Windows): semua kode lewat `cargo check` pengguna, kirim berkas utuh bila patch gagal. Verifikasi nama feature dan API di dokumentasi sesuai versi crate, jangan menebak.
- **Peran Claude:** instruktur/analis kritis. Timbang pro-kontra sebelum merekomendasikan, sampaikan ketidaksetujuan bila perlu, akui bila data membantah dugaan sebelumnya.

---

## 1. Setup lingkungan (Windows, PowerShell)

### 1.1 Prasyarat: C++ Build Tools (linker MSVC)

Rust di Windows memakai toolchain `msvc`, yang butuh linker dan library dari Visual Studio / Build Tools.

- Pilih workload **"Desktop development with C++"**. Pastikan komponen **MSVC v143 build tools** dan **Windows 11 SDK** ikut tercentang.
- Opsi A (GUI): unduh _Build Tools for Visual Studio_ dari <https://visualstudio.microsoft.com/visual-cpp-build-tools/> lalu centang workload di atas.
- Opsi B (PowerShell, jika winget tersedia):

  ```powershell
  winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
  ```

  Jika perintah ini gagal, pakai Opsi A. Cek juga apakah ada versi Visual Studio yang lebih baru dari 2022 di halaman unduhan.

- Catatan lisensi: Microsoft menyatakan penggunaan C++ Build Tools memerlukan lisensi Visual Studio yang valid. Edisi Community gratis untuk individu, mahasiswa, dan kontributor open source.
- Alternatif: kalau kamu belum punya Visual Studio, `rustup-init` akan menawarkan memasangnya otomatis. Itu memasang edisi Community.

### 1.2 Pasang Rust (rustup)

```powershell
winget install --id Rustlang.Rustup
```

Tutup lalu buka ulang PowerShell, kemudian:

```powershell
rustup default stable-msvc
rustup update
rustc --version
cargo --version
rustup show
```

Jika `cargo` tidak dikenali, PATH belum ter-refresh. Buka terminal baru atau restart.

### 1.3 Editor

```powershell
winget install --id Microsoft.VisualStudioCode
code --install-extension rust-lang.rust-analyzer
```

### 1.4 Buat proyek

```powershell
cargo new winisland
cd winisland
cargo add slint
cargo add --build slint-build
```

`cargo add` otomatis memilih versi terbaru. Setelah itu buka `Cargo.toml` dan lengkapi seperti ini:

```toml
[package]
name = "winisland"
version = "0.1.0"
edition = "2021"
build = "build.rs"

[dependencies]
slint = "..."        # biarkan versi yang dipilih cargo add

[build-dependencies]
slint-build = "..."  # biarkan versi yang dipilih cargo add

[profile.release]
opt-level = "z"      # kecilkan ukuran; bandingkan dengan "s" dan 3 nanti
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

Buat `build.rs` di root proyek:

```rust
fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
```

Buat `ui/main.slint`:

```slint
export component MainWindow inherits Window {
    no-frame: true;
    always-on-top: true;
    background: transparent;
    width: 400px;
    height: 120px;

    in-out property <bool> expanded: false;

    Rectangle {
        x: (parent.width - self.width) / 2;
        y: 8px;
        width: root.expanded ? 360px : 140px;
        height: root.expanded ? 100px : 32px;
        border-radius: min(self.height / 2, 28px);
        background: black;

        animate width, height { duration: 250ms; easing: ease-out; }

        TouchArea {
            clicked => { root.expanded = !root.expanded; }
        }
    }
}
```

Ganti isi `src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    ui.run()
}
```

Jalankan:

```powershell
cargo run               # debug, untuk dev
cargo build --release   # untuk mengukur ukuran dan RAM yang sebenarnya
```

Catatan: sintaks di atas ditulis dari pengetahuan umum tentang Slint dan belum saya uji. Jika ada error kompilasi, cek dokumentasi Slint untuk versi yang terpasang. Jangan tebak-tebak.

### 1.5 Sanity check

- Jendela hitam berbentuk pill muncul tanpa bingkai, di atas jendela lain.
- Klik pill, ukurannya beranimasi memanjang lalu mengecil lagi.
- Kalau jendela tidak transparan atau muncul bingkai, itu temuan spike pertama. Catat, jangan diabaikan.

### 1.6 Perintah harian

```powershell
cargo fmt        # rapikan kode
cargo clippy     # lint, bagus untuk belajar Rust idiomatis
cargo check      # cek cepat tanpa build penuh
rustup update    # perbarui toolchain
```

---

## 2. Cara mengukur (supaya perbandingan adil)

Selalu ukur build **release** dalam keadaan **idle** (jangan disentuh minimal 1 menit).

- **Ukuran:** lihat ukuran `target\release\winisland.exe`.
- **RAM:** buka Task Manager, tab **Details**, tambahkan kolom **"Memory (private working set)"**. Angka itu yang dipakai.
- **Alternatif PowerShell** (nama counter bisa berbeda di Windows berbahasa Indonesia; jika error, pakai Task Manager):

  ```powershell
  Get-Process winisland | Select-Object Name, @{n='WorkingSetMB';e={[math]::Round($_.WorkingSet64/1MB,1)}}, @{n='CommitMB';e={[math]::Round($_.PrivateMemorySize64/1MB,1)}}
  ```

- Catat juga CPU saat idle. Overlay yang tampak diam tapi menggambar ulang terus akan membuang baterai.
- Ulangi pengukuran setelah mengganti renderer (lihat bagian 4).

---

## 3. Spike (1-2 hari)

Tujuan: menjawab "apakah arah Rust + Slint layak?" dengan data, bukan opini.

**Angka target (usulan awal, BELUM disepakati dan belum terbukti realistis):**

| Metrik | Target usulan | Target final (isi sendiri) |
| --- | --- | --- |
| RAM idle (private working set) | ≤ 30 MB | Tanpa anggaran keras (diputuskan 21 Okt 2026); dicatat per fitur sebagai informasi. Terukur 2,93 MB (setelah 9d-1) |
| Ukuran `.exe` release | ≤ 10 MB | Tanpa target keras (sekunder). Terukur 7,35 MB (build `dist`) |
| CPU idle | mendekati 0% | Mendekati 0%. Terukur 0% di semua build yang diukur |

**Lulus/gagal:**

1. Window pill: transparan, tanpa bingkai, always-on-top, bisa expand/collapse dengan animasi yang mulus.
2. Window tidak mencuri fokus dari aplikasi lain saat diklik atau di-hover (atau ada jalan keluar yang jelas).
3. Menampilkan judul lagu dari media session (SMTC) lewat `windows-rs`.
4. Angka RAM/ukuran/CPU diukur dan dicatat di tabel.

**Percobaan paralel (kalau sempat):** bandingkan renderer. Slint punya beberapa renderer, dan pilihannya memengaruhi RAM dan kehalusan animasi. Cek dokumentasi Slint untuk daftar feature flag renderer pada versi yang terpasang.

**Cek hari pertama (risiko tertinggi):** apakah pendengar notifikasi Windows (`UserNotificationListener`) bisa dipakai aplikasi unpackaged tanpa package identity. Ini belum saya verifikasi. Kalau butuh package identity, fitur notifikasi harus ditunda atau dirancang ulang.

Dependensi untuk langkah 3 (verifikasi nama feature di dokumentasi crate `windows`):

```powershell
cargo add windows --features "Media_Control,Foundation"
```

### Gerbang keputusan setelah spike

| Hasil | Tindakan |
| --- | --- |
| Semua lulus | Lanjut ke MVP dengan Slint |
| Animasi patah-patah atau RAM di atas target | Coba renderer lain di Slint. Jika tetap gagal, pertimbangkan windows-rs + Direct2D/DirectComposition |
| Integrasi WinRT di Rust terlalu menyakitkan | Evaluasi ulang C#, dengan sadar mengorbankan kriteria ukuran bundle |
| Target RAM tidak mungkin dicapai di opsi mana pun | Longgarkan target, lalu Tauri jadi opsi realistis lagi |

---

## 4. MVP (setelah spike lulus)

**Masuk MVP:**

- Berjalan sebagai aplikasi latar di system tray (bukan Windows Service), tanpa ikon taskbar dan tanpa muncul di Alt+Tab.
- Keadaan idle: menempel di tepi atas layar dan tidak menghalangi apa pun. Saat kursor diarahkan ke sana, ia expand dengan animasi mulus; saat kursor pergi, ia kembali ke idle.
- Kontrol media: judul, artis, play/pause, next/prev.
- Penempatan di atas layar, mendukung pilihan monitor.
- Autostart saat login.

**Ditunda:**

- Notifikasi (akses terkonfirmasi `Allowed` pada langkah 10; implementasi bisa dimulai)
- Clipboard history, file tray, shortcut, widget cuaca/baterai/timer

Prinsip: satu fitur pembeda yang benar-benar kamu pakai sehari-hari, bukan menyalin semua fitur kompetitor.

**Titik rawan yang harus dirancang sejak awal** (keluhan pengguna aplikasi sejenis):

- Perilaku di aplikasi fullscreen
- Multi-monitor dan DPI scaling
- CPU/RAM saat idle
- Kenyamanan klik: overlay tidak boleh menghalangi tombol minimize/close jendela di bawahnya

---

## 5. Jika dipublikasikan (nanti, bukan sekarang)

- **Lisensi Slint:** header kode Slint menyebut tiga opsi, yaitu GPL-3.0, lisensi royalty-free Slint, atau lisensi komersial Slint. Untuk pemakaian pribadi tidak masalah. Sebelum publikasi, baca syarat lengkapnya di situs Slint dan pilih lisensi proyekmu agar kompatibel.
- **Jangan menyalin kode DynamicWin.** Repo itu berlisensi CC BY-SA 4.0. Terinspirasi boleh, menyalin tidak.
- **Distribusi:** installer tanpa code signing memicu peringatan SmartScreen. Pembaruan otomatis harus dibangun sendiri atau lewat installer seperti Inno Setup.
- **Hygiene repo:** jangan commit secret, dan pilih lisensi dengan sengaja.

---

## 6. Risiko yang harus selalu diingat

1. **Mandek karena terlalu lama membangun fondasi.** Ini pertama kali kamu memakai Rust. Jaga MVP sangat kecil, dan rayakan spike yang selesai.
2. **Angka RAM belum terukur.** Semua perkiraan di percakapan sebelumnya hanya estimasi, bukan benchmark.
3. **Transparansi, no-frame, always-on-top, click-through di Slint belum diverifikasi** untuk kebutuhan ini. Itulah gunanya spike.
4. ~~**Notifikasi** mungkin butuh package identity (lihat bagian 3).~~ **Teratasi (langkah 10):** akses `Allowed` tanpa identity.
5. **Janji "lebih ringan" butuh bukti.** Kalau hasil ukur tidak mendukung, ubah keputusan. Jangan membela pilihan awal.

---

## 7. Checklist sesi baru

- [x] C++ Build Tools terpasang
- [x] `rustc --version` dan `cargo --version` berhasil
- [x] Proyek `winisland` jalan dengan `cargo run`
- [x] Target angka final diisi (bagian 3): RAM dan `.exe` tanpa target keras, CPU idle mendekati 0%
- [x] Spike selesai dan hasil ukurnya dicatat (lihat bagian 8): keempat kriteria lulus/gagal di bagian 3 terpenuhi; cek notifikasi hari pertama selesai dengan hasil campuran (membaca bisa, event gagal)
- [x] Keputusan renderer setelah melihat data (software, femtovg dibuang)

---

## 8. Cara kerja dan log kemajuan

### Cara kerja (disepakati)

Setiap langkah berjalan dengan urutan yang sama, dan dokumen ini diperbarui di akhir tiap langkah:

1. Kerjakan satu kelompok perubahan kecil.
2. Verifikasi dengan checklist langkah itu.
3. Commit memakai pesan yang sudah disiapkan.
4. Perbarui log di bagian ini.
5. Baru lanjut ke langkah berikutnya.

### Konvensi kode

- Teks yang tampil di UI (label menu, tooltip) memakai bahasa Inggris.
- Komentar pada fungsi dan kode memakai bahasa Indonesia.

### Revisi kebutuhan 1 (21 Okt 2026)

1. **Berjalan di latar belakang, bukan di taskbar.** Dikoreksi: Windows Service sungguhan tidak bisa menampilkan UI karena isolasi Session 0, jadi bentuk yang benar adalah **aplikasi tray yang berjalan otomatis saat login**. Overlay tanpa bingkai tidak punya tombol minimize, jadi "masuk tray" ditangani lewat menu tray (Show/Hide, Quit).
2. **Idle tidak berbentuk pill.** Menempel di tepi atas layar dan sekecil mungkin sehingga tidak menghalangi klik. Expand hanya saat kursor menghampiri.
3. **Tidak muncul di taskbar maupun Alt+Tab.**

### Keputusan desain dan alasan

| Topik | Pilihan | Alasan | Risiko |
| --- | --- | --- | --- |
| Bentuk proses | Satu proses: aplikasi tray + autostart | Satu proses lebih hemat RAM dibanding Service + proses UI terpisah (dua proses dan IPC) | Tidak berjalan sebelum login (tidak dibutuhkan) |
| Deteksi hover saat idle | Jendela idle sangat tipis di tepi atas (garis beberapa piksel), berbasis event | Nol CPU saat idle, tanpa polling. Tepi atas layar mudah dicapai kursor | Beberapa piksel paling atas di area tengah tetap menghalangi klik. Perlu uji batas ukuran jendela terkecil di Windows |
| Alternatif hover | Low-level mouse hook, atau polling `GetCursorPos` | Tidak ada jendela saat idle | Hook: risiko latensi mouse seluruh sistem dan sering ditandai antivirus/anti-cheat. Polling: bertentangan dengan target CPU idle |
| Ukuran jendela | Mengikuti state (opsi A): idle = garis tipis, expanded = ukuran penuh | Menyelesaikan temuan klik terhalang yang sudah terkonfirmasi | Jendela di-resize saat animasi: rencananya jendela membesar dulu baru konten dianimasikan, dan mengecil setelah animasi selesai. Perlu dijaga tetap di tengah |
| Ikon tray | `SystemTrayIcon` bawaan Slint (feature `system-tray`, aktif di default features 1.18.1), dengan `ui/tray.png` | Tanpa dependensi baru, satu event loop, ukuran `.exe` kemungkinan tidak bertambah (fitur sudah ter-compile). Klik kanan membuka menu, klik kiri memanggil toggle | Relatif baru (rilis 1.17, Juni 2026). Jika nanti memakai `default-features = false`, feature `system-tray` harus dipertahankan. Cadangan bila bermasalah: crate `tray-icon` |
| Autostart | Kunci registry `HKCU\...\Run` | Sederhana, tanpa hak admin | Perlu opsi di menu tray untuk mematikan |
| Posisi saat expand | Floating 8 px dari tepi atas; saat idle menempel (y = 0) | Kesan Dynamic Island, animasi turun-naik jadi isyarat visual | Strip transparan 8 px menangkap klik saat expanded. `TouchArea` harus tetap menutupi seluruh jendela (bukan hanya pill) agar hover tidak putus di celah itu. Angka 8 ada di dua tempat (`main.slint` dan `FULL_H` di `main.rs`) |
| Renderer | Software, dikunci lewat kode (`BackendSelector`), bukan env var | Terukur 2,3 MB idle vs 42 MB pada femtovg (pengukuran sama, build release) | CPU render lebih berat pada layar resolusi tinggi atau konten besar. Fitur visual yang tidak didukung software renderer perlu dicek di dokumentasi sebelum menambah efek. Animasi dinilai mulus oleh pengguna (21 Okt 2026); selisih CPU expand vs idle sekitar 0,3-0,4% |

### Log langkah

| # | Isi | Status |
| --- | --- | --- |
| 0 | Setup lingkungan (Build Tools, Rust, VS Code), `setup.ps1` melaporkan 24 OK / 0 FAIL | Selesai |
| 1 | Prototipe Slint: pill tanpa bingkai, transparan, animasi expand, posisi tengah-atas via `GetSystemMetrics` | Selesai (diverifikasi lewat screenshot) |
| 2 | Sembunyikan dari taskbar dan Alt+Tab, cegah curi fokus | Selesai (2b): style dipasang lewat timer di dalam event loop, terpasang pada percobaan ke-1, exstyle `0x40118` menjadi `0x8000198`. Ikon hilang dari taskbar dan Alt+Tab. Uji fokus dan always-on-top belum dilaporkan |
| 3 | Uji apakah area transparan menghalangi klik | Selesai: **terkonfirmasi terhalang** |
| 4 | Baseline RAM cepat (build release, Task Manager) | Selesai: femtovg bawaan 42 MB idle / 43 MB expanded, software 2,3 MB idle / 2,5 MB expanded, CPU 0%. Ukuran `.exe` 11,34 MB. Detail di tabel bagian "Hasil baseline RAM" |
| 5 | Idle = garis tipis di tepi atas, expand saat hover, ukuran jendela mengikuti state | Selesai: idle 140x4, expanded 360x108 (pill 360x100 floating 8 px). Jendela membesar sebelum animasi dan mengecil 280 ms setelah. Tebal garis aktual dan ada tidaknya kedip di tepi belum dilaporkan |
| 6 | Ikon tray (Show / Hide, Quit), jendela tidak keluar dari taskbar | Selesai: tray bawaan Slint berjalan baik (laporan pengguna). Setelah hide lalu show, style jendela dipasang ulang lewat `try_apply`. RAM idle sekitar 3 MB setelah tray, ukuran `.exe` belum dilaporkan |
| 7 | Autostart saat login | Selesai: menu "Autostart: On/Off" berganti, entri `HKCU\...\Run` ditulis dan dihapus dengan benar. Build release dari `%LOCALAPPDATA%\winisland\` berjalan saat login tanpa jendela terminal, ikon tray ada, terdaftar di Startup apps (laporan pengguna). Posisi overlay dan taskbar/Alt+Tab saat login tidak dirinci |
| 8 | Pengukuran RAM, CPU, ukuran `.exe` lengkap (bagian 2) pada bentuk aplikasi yang sudah baru | Selesai: femtovg dibuang, profil `dist` diterapkan, build `dist` lulus checklist (tray, hover, autostart, quit). `.exe` 7,3 MB, private working set sekitar 2,6 MB, CPU idle 0% |
| 9 | Kontrol media via SMTC (windows-rs), dipecah: 9a baca judul/artis ke konsol, 9b tampil di overlay, 9c pembaruan otomatis, 9d kontrol (9d-1 ikon status, 9d-2 tombol) | Selesai: 9a, 9b, 9c (event SMTC + worker), 9d-1 (ikon play/pause), 9d-2 (tombol previous, play/pause, next) lolos uji (laporan pengguna). RAM 9d-2 tidak diukur sendiri (tercakup di pengukuran setelah 10e) |
| 10 | Notifikasi lewat `UserNotificationListener` pada aplikasi unpackaged | 10a selesai: `Allowed`. 10b selesai: **membaca daftar dan isi berhasil, registrasi `NotificationChanged` gagal** (0x80070490). Desain tampilan dan opsi pembacaan menunggu keputusan **10c selesai:** `notif.rs` (baca daftar, tanpa event) dan `notif_text.rs` (pembersihan, pengurutan, dedup; 13 tes lolos), terhubung ke konsol lewat `debug_run`. UI tersambung di 10d dan 10e (baris di bawah) |
| 10d | Notifikasi di UI: lencana jumlah, tampilan notifikasi yang menggantikan tampilan media, judul redundan dikosongkan | Selesai, lolos uji pengguna (22 Okt 2026): `cargo check`, `cargo test` (17 tes), uji interaktif. Commit `4c353e9` |
| 10e | Sakelar privasi "tampilkan isi notifikasi" di menu tray, disimpan di registry | Selesai, lolos uji pengguna (22 Okt 2026). `src/config.rs` baru, item menu "Notification body: Hidden/Shown", env var `WINISLAND_NOTIF_BODY` dihapus dari jalur fetch. RAM diukur gabungan (bagian di bawah). Menunggu commit |
| 11 | Lisensi dan dokumen repo (MIT + atribusi Slint Royalty-free 2.0): `LICENSE`, `README.md`, `CONTRIBUTING.md`, field `license` di `Cargo.toml` | Selesai: commit `b2bdb01` |

### Status temuan spike

| Temuan | Status | Catatan |
| --- | --- | --- |
| Ikon aplikasi muncul di taskbar dan Alt+Tab | **Teratasi** (2b) | Perlu dicek ulang setelah ikon tray ditambahkan (langkah 6) |
| Jendela mencuri fokus saat diklik | **Lolos** (dikonfirmasi pengguna, 21 Okt 2026) | `WS_EX_NOACTIVATE` terpasang |
| Always-on-top | **Lolos** (dikonfirmasi pengguna, 21 Okt 2026), tidak ada perilaku aneh di fullscreen yang dilaporkan | Game fullscreen eksklusif belum diuji khusus |
| Area transparan menghalangi klik | **Terkonfirmasi, solusi diterapkan** (langkah 5) | Sisa: strip 140x4 saat idle dan celah 8 px saat expanded |
| Tebal garis idle sebenarnya | Belum dilaporkan | Windows/Slint mungkin menolak jendela setipis 4 px. Lihat dengan mata atau tangkapan layar |
| Kedip/loop expand-collapse di tepi pill | Belum dilaporkan | Log pernah menunjukkan dua `place 360x...` berturut-turut (kursor keluar-masuk < 280 ms), kemungkinan normal |
| Ikon tray: Show / Hide dan Quit | **Lolos** (dikonfirmasi pengguna) | Klik kiri men-toggle overlay. Alt+Tab bersih setelah hide lalu show. RAM idle sekitar 3 MB (naik sekitar 0,7 MB dari 2,3 MB). Ukuran `.exe` setelah tray dicatat di langkah 8 |
| Autostart saat login | **Lolos** (build release, lokasi tetap) | Tanpa terminal, ikon tray ada, terdaftar di Startup apps. Batasan yang diketahui: jika dimatikan dari Settings > Apps > Startup, menu tetap menampilkan "On" (jalur `StartupApproved` tidak dibaca) |
| Animasi halus, RAM, CPU | RAM dan CPU baseline terukur (langkah 4). Animasi dinilai mulus di software renderer (laporan pengguna) | Pengukuran lengkap tetap di langkah 8 |
| SMTC pada aplikasi unpackaged | **Lolos** (9a) | Tanpa error akses atau inisialisasi COM, `join()` di thread latar langsung bekerja. Pembacaan sesi aktif dikonfirmasi dengan video browser |
| UserNotificationListener pada aplikasi unpackaged (10a) | **Lolos sebagian** | `RequestAccessAsync` mengembalikan `Allowed` (status 1) dari thread latar. Status ini tidak menjamin semua API berjalan tanpa package identity: pembacaan daftar lolos, event gagal (dua baris berikut) |
| Membaca isi notifikasi (10b) | **Lolos** | `GetNotificationsAsync(Toast)` mengembalikan 27 notifikasi aktif lengkap dengan nama aplikasi dan teks (judul + isi) tanpa package identity |
| Event `NotificationChanged` (10b) | **Gagal** | Registrasi mengembalikan `Element not found` (0x80070490) pada aplikasi unpackaged. Penyebab pastinya tidak dinyatakan oleh pesan error; dugaan kuat: butuh package identity (didukung banyak laporan komunitas, belum dikonfirmasi dokumentasi Microsoft untuk API ini) |
| Event SMTC dan kontrol media (9c, 9d) | **Lolos** | Event `MediaPropertiesChanged`, `PlaybackInfoChanged`, `CurrentSessionChanged` menembak pada aplikasi unpackaged; `TryTogglePlayPauseAsync` dan skip bekerja tanpa `join()` |

### Perubahan dependensi sejauh ini

- `slint` (+ feature `raw-window-handle-06`) dan `slint-build`
- `raw-window-handle` 0.6
- `windows` dengan feature `Win32_Foundation` dan `Win32_UI_WindowsAndMessaging`
- Ikon tray memakai `SystemTrayIcon` bawaan Slint (feature `system-tray`, default), tanpa dependensi baru. Aset: `ui/tray.png`
- Langkah 9a: `windows` ditambah feature `Media_Control` dan `Foundation`; kode di `src/media.rs`
- `winreg` (langkah 7): autostart lewat kunci registry `HKCU\...\Run`, kode di `src/autostart.rs`
- Langkah 8: `slint` memakai `default-features = false` dengan feature `compat-1-18`, `std`, `backend-winit`, `renderer-software`, `system-tray`, `raw-window-handle-06`, `accessibility` (femtovg dibuang). `[profile.dist]` ditambahkan; `release` tetap standar. Femtovg bisa ditambahkan lagi lewat feature `renderer-femtovg` bila nanti terbukti perlu.

- Langkah 10b: `windows` ditambah feature `ApplicationModel` dan `Foundation_Collections` (dibutuhkan `GetNotificationsAsync`, yang mengembalikan `IVectorView`, dan `AppInfo`). Dipakai hanya oleh probe (build debug).

### Catatan implementasi (supaya tidak mengulang kesalahan)

- Handle jendela baru tersedia setelah event loop berjalan. Style `WS_EX_TOOLWINDOW` dan `WS_EX_NOACTIVATE` dipasang lewat timer di dalam event loop dengan percobaan ulang terbatas (`try_apply`), dan dipasang ulang setiap kali jendela di-`show()`.
- Ukuran dan posisi jendela diatur dari Rust (`place`, dalam piksel logis lalu dikonversi ke fisik). Jendela membesar dulu sebelum animasi expand dan mengecil 280 ms setelah collapse.
- `TouchArea` menutupi seluruh jendela (bukan hanya pill) supaya hover tidak putus di celah 8 px saat floating. Jangan dipindahkan ke dalam `Rectangle` pill.
- Di Slint 1.18.1, method Rust `on_clicked` tidak dihasilkan untuk `SystemTrayIcon` (error E0599) walau ada di dokumentasi "latest". Callback `clicked` ditangani di file `.slint` (`clicked => { toggle(); }`).
- Loop dijalankan dengan `slint::run_event_loop()` (bukan `ui.run()`), dan ikon tray menjaga proses tetap hidup saat jendela disembunyikan.
- Item menu tray: `checkable`/`checked` dikenali kompilator tetapi centang tidak terlihat di menu Windows. Status autostart ditampilkan lewat judul teks ("Autostart: On" / "Autostart: Off"). Status "aktif" berarti entri registry sama dengan path exe yang sedang berjalan.
- Callback Slint dengan parameter (mis. `toggle-autostart(bool)`) menghasilkan closure Rust dengan satu argumen.
- Build debug adalah aplikasi konsol (`windows_subsystem` hanya berlaku di release), jadi autostart dari build debug membuka jendela terminal. Aktifkan autostart hanya dari build release di lokasi tetap. Exe yang sedang berjalan terkunci, jadi `cargo build` gagal bila proses masih hidup (Quit dulu).
- Cara pasang untuk pemakaian harian: `cargo build --profile dist`, salin `target\dist\winisland.exe` ke `%LOCALAPPDATA%\winisland\`, jalankan dari sana, lalu aktifkan Autostart lewat menu. Quit dulu sebelum menyalin versi baru.

- **API windows-rs 0.62.2 (diverifikasi dari source crate, bukan dokumentasi C#):** closure `TypedEventHandler::new` menerima `windows_core::Ref<T>` (pakai `.as_ref()`), bukan `&Option<T>`, dan harus `Fn + Send + 'static`. Token event bertipe `i64` (`Remove*Changed(token: i64)`). Tulis tipe generik eksplisit (`TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new`) supaya inferensi tidak ambigu.
- Handler event WinRT hanya mengirim sinyal ke channel; semua pembacaan dan perintah dijalankan di satu worker thread (`media.rs`). Hindari `join()` di dalam callback WinRT (race antar-event, urutan selesai tidak terjamin). Jangan pakai `std::thread::park()` untuk menjaga thread hidup: `park` boleh kembali tanpa `unpark`, sehingga listener ikut di-drop diam-diam.
- `slint::Weak::upgrade_in_event_loop` aman dipanggil dari thread mana pun (termasuk worker); tidak perlu `invoke_from_event_loop`.
- Slint: `color` adalah properti bawaan `Rectangle` (alias lama `background`), jadi tidak boleh dipakai sebagai nama properti baru (pakai mis. `icon-color`). `Path` di software renderer hanya digambar bila feature `software-renderer-path` aktif (menarik `zeno` dan `lyon_path`); ikon pill digambar dari `Rectangle` (strip vertikal 1 px untuk segitiga) tanpa dependensi tambahan.
- Slint `TouchArea` bersarang: `has-hover` induk tetap `true` saat kursor di atas `TouchArea` anak (source `i-slint-core` 1.18.1, dan terbukti di uji 9d-2). Letakkan `TouchArea` tombol setelah `@children` supaya berada di atas ikon.
- Perintah kontrol media dijalankan fire-and-forget (`TryXAsync()` tanpa `join()`); operasi tetap berjalan setelah objeknya di-drop (terbukti di uji 9d-2).

- Langkah 10c: privasi diterapkan di sumber (isi pesan tidak dibaca bila `include_body` salah), bukan hanya disembunyikan di UI. Kunci dedup adalah apa yang ditampilkan: (aplikasi, judul) atau (aplikasi, judul, isi). U+200C dan U+200D (ZWNJ, ZWJ) sengaja dipertahankan saat membersihkan teks; U+200E, U+200F, dan kontrol bidi dibuang. `GetAccessStatus` dicek tiap pembacaan karena bila akses dicabut Windows mengembalikan daftar kosong tanpa error.
- Alur uji tanpa Windows di sandbox: logika murni dipisah ke berkas tanpa crate `windows` (pola `notif_text.rs`) sehingga bisa diuji di Linux (`apt-get install rustc-1.91 cargo-1.91`; crate `windows` sendiri tidak bisa dikompilasi di Linux, jadi `cargo check` pengguna tetap gerbang untuk kode WinRT).

- **Sakelar privasi notifikasi (10e):** disimpan sebagai DWORD `ShowNotificationBody` di `HKCU\Software\winisland` (`src/config.rs`); nilai tidak ada, bukan 1, atau gagal dibaca berarti **Hidden**. Keadaan di memori (`AtomicBool`) diubah lebih dulu, baru ditulis ke registry, jadi pilihan berlaku seketika walau penyimpanan gagal. `apply_notifications` membaca sakelar saat hasil diterapkan (bukan saat fetch dimulai) dan toggle mengosongkan notifikasi yang sudah tampil, supaya fetch yang dimulai saat Shown tidak bisa menampilkan isi setelah pengguna memilih Hidden.
- Nama properti dan callback Slint dengan tanda hubung menjadi `snake_case` di Rust (`show-notif-body` menjadi `set_show_notif_body`, `toggle-notif-body` menjadi `on_toggle_notif_body`).

### Prosedur langkah 8

**Feature Slint 1.18.1 yang terverifikasi** (halaman `docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/`):

- Default: `compat-1-18` (wajib), `std`, `accessibility`, `system-tray`, `backend-default` (alias), `renderer-femtovg`, `renderer-software`.
- Dengan `default-features = false` yang harus ditambahkan manual: `compat-1-18`, `std`, `backend-winit`, `renderer-software`, `system-tray`, plus `raw-window-handle-06` yang sudah kita pakai.
- Halaman yang sama menyebut `compat-1-2` di deskripsi `system-tray`, padahal feature wajib yang tertulis adalah `compat-1-18`. Dokumentasinya tidak konsisten. Pakai `compat-1-18`, dan jika cargo menolak nama feature, baca pesan errornya.
- `accessibility` bukan syarat tray maupun renderer. Diuji sebagai varian terpisah, bukan dibuang sekaligus.

**Varian build** (rantai pengukuran, bukan semua kombinasi):

| Kode | Feature                                  | `opt-level` | Tujuan                     |
| ---- | ---------------------------------------- | ----------- | -------------------------- |
| A-z  | default (kondisi sekarang)               | z           | baseline langkah 8         |
| B-z  | `default-features = false` tanpa femtovg | z           | efek membuang femtovg saja |
| B-s  | sama dengan B                            | s           | efek opt-level             |
| B-3  | sama dengan B                            | 3           | efek opt-level             |
| C-z  | B tanpa `accessibility` (opsional)       | z           | harga accessibility        |

`opt-level` diganti lewat variabel lingkungan `CARGO_PROFILE_RELEASE_OPT_LEVEL`, tanpa mengedit `Cargo.toml`.

**Yang diukur per varian:** ukuran exe, private working set dan commit setelah satu siklus hover lalu idle 1 menit, serta CPU selama fase hover (bukan hanya idle). Alasannya: pada software renderer, rasterisasi dikerjakan CPU, dan `opt-level = "z"` bisa membuat animasi memakan lebih banyak detik-CPU. Skrip: `measure.ps1`.

**Risiko khusus:** build release tidak punya konsol. Jika `BackendSelector::select()` gagal setelah feature dikurangi, aplikasi keluar tanpa pesan. Jalankan varian B lewat `cargo run` (debug) dulu sebelum build release.

### Hasil langkah 8, putaran 1 (21 Okt 2026)

Dijalankan lewat `run-all.ps1`: build release, hover otomatis 10 siklus, idle 60 detik, satu kali per varian. Tidak ada `[profile.release]` di `Cargo.toml`, jadi yang berbeda antar varian hanya feature dan `opt-level` (lewat env var). `lto`, `codegen-units`, `panic`, dan `strip` belum pernah diterapkan.

| Varian | Feature | opt-level | `.exe` MB | PWS awal | PWS hover | PWS idle | Commit idle | CPU hover (dtk) | CPU idle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A-z | default | z | 10,71 | 2,90 | 2,89 | 2,80 | 3,70 | 0,172 | 0% |
| B-z | tanpa femtovg | z | 10,18 | 2,68 | 2,63 | 2,58 | 3,50 | 0,188 | 0% |
| B-s | tanpa femtovg | s | 10,75 | 2,74 | 2,74 | 2,64 | 3,49 | 0,094 | 0% |
| B-3 | tanpa femtovg | 3 | 12,54 | 2,73 | 2,69 | 2,62 | 3,47 | 0,125 | 0% |

Temuan:

- RAM tidak membedakan varian (selisih di bawah 0,3 MB), dan seluruhnya sekitar 10% dari anggaran RAM 20-30 MB yang ditetapkan pengguna untuk fitur ke depan.
- Membuang femtovg menghemat 0,53 MB `.exe` (sekitar 5%).
- `opt-level`: `z` terkecil, `s` +0,57 MB, `3` +2,36 MB. Selisih CPU hover (0,09-0,19 dtk per sekitar 24 dtk) ada dalam derau satu kali ukur: `3` tidak lebih cepat dari `s`. Belum ada bukti `z` merugikan CPU.
- Private working set tidak turun selama idle 60 dtk, jadi dugaan trimming tidak terbukti oleh data ini. Pembacaan Task Manager 0,5-0,9 MB setelah login tetap tidak dipakai.
- `.exe` belum mencapai target usulan 10 MB (10,18 MB di B-z). Tuas ukuran berikutnya: `[profile.release]` (varian D-z/E-z).

**Keputusan sementara (setelah putaran 1):**

- `opt-level = "z"` dipilih (terkecil, tanpa bukti merugikan CPU).
- Membuang femtovg **belum final**. Pengguna ingin menyisakan opsi untuk styling. Fakta yang sudah dicek: femtovg adalah renderer, bukan fitur styling. Software renderer tidak mendukung properti `drop-shadow-*` (dokumentasi Slint terbaru). Runtime femtovg terukur 42 MB, di atas anggaran RAM 20-30 MB. Putaran 2 menambah varian F-z (femtovg tetap ada, plus `[profile.release]`) supaya biaya "menyisakan femtovg" terukur pada pengaturan final.

### Hasil langkah 8, putaran 2 (21 Okt 2026)

Pengaturan final kandidat: `opt-level = "z"` plus `[profile.release]` (`lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`). Satu kali ukur per varian, hover otomatis 10 siklus, idle 60 detik.

| Varian | Feature | `.exe` MB | PWS awal | PWS hover | PWS idle | Commit idle | CPU hover (dtk) | CPU idle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D-z | tanpa femtovg | 7,30 | 2,70 | 2,66 | 2,59 | 3,47 | 0,188 | 0% |
| F-z | femtovg tetap ter-compile | 7,63 | 2,95 | 2,91 | 2,86 | 3,73 | 0,141 | 0% |

Temuan:

- `[profile.release]` memangkas `.exe` dari 10,18 MB (B-z) menjadi 7,30 MB (D-z), sekitar 28%. Dibanding baseline A-z (10,71 MB), total turun sekitar 32%. Target usulan 10 MB tercapai di kedua varian.
- Biaya menyisakan femtovg ter-compile (renderer tetap software): +0,33 MB `.exe` dan sekitar +0,2 MB RAM (selisih RAM konsisten dengan A vs B di putaran 1, tapi tetap kecil).
- Selisih CPU hover D-z vs F-z (0,188 vs 0,141 dtk) berada dalam derau satu kali ukur.
- RAM sekitar 2,6-2,9 MB, jauh di bawah anggaran 20-30 MB.
- Varian tanpa `accessibility` (E-z) tidak diuji. `accessibility` dipertahankan.
- Menjalankan femtovg sebagai renderer (42 MB pada baseline langkah 4) tetap di atas anggaran RAM. Menyisakan femtovg ter-compile hanya membuka pintu, bukan menyelesaikan celah `drop-shadow-*` pada software renderer.

### Prioritas dan repo publik (koreksi 21 Okt 2026)

- **Prioritas metrik:** RAM adalah yang utama. Ukuran `.exe` sekunder dan **tanpa target keras**: angka 10 MB hanyalah usulan awal yang tidak pernah disepakati, dan ukuran pasti naik seiring fitur. Yang dipantau di setiap fitur baru (SMTC, notifikasi) adalah RAM, diukur ulang setelah fitur masuk.
- **Repo sudah publik** dan kontributor diharapkan. Hasil pemeriksaan repo (commit `197ce5c`):
  - Belum ada `LICENSE`, `README`, maupun `CONTRIBUTING`. Tanpa lisensi, kode otomatis berstatus hak cipta penuh milik penulis dan orang lain tidak punya dasar hukum untuk memakai atau berkontribusi. Pilih lisensi sebelum PR eksternal pertama, dan baca syarat lisensi Slint (bagian 5) lebih dulu supaya pilihannya kompatibel.
  - Pencarian sederhana tidak menemukan path pribadi atau secret di berkas terlacak (bukan audit penuh). Alamat email penulis commit terlihat publik di riwayat git.
  - Aturan yang perlu didokumentasikan untuk kontributor: renderer dikunci ke software lewat kode, jadi efek yang belum didukung software renderer (mis. `drop-shadow-*`) tidak akan tampil; daftar feature Slint sengaja dipangkas.
- **Profil build untuk repo publik:** `[profile.release]` yang agresif (lto, `codegen-units = 1`, `panic = "abort"`, `strip = true`) membebani build kontributor dan membuat laporan crash tanpa nama fungsi. Usulan: `release` tetap moderat, profil agresif dipisah ke `[profile.dist]`. Menunggu keputusan pengguna.

### Hasil ukur final langkah 8 (build `dist`, 21 Okt 2026)

Diukur lewat `measure.ps1` dari `%LOCALAPPDATA%\winisland\winisland.exe`, hover manual sekitar 8,5 detik, idle 60 detik.

| Fase                       | Private working set (MB) | Working set total (MB) | Commit (MB) |
| -------------------------- | ------------------------ | ---------------------- | ----------- |
| Awal (5 dtk setelah start) | 2,80                     | 18,77                  | 3,78        |
| Setelah hover              | 2,61                     | 18,61                  | 3,55        |
| Idle 60 dtk                | 2,55                     | 18,58                  | 3,45        |

- Ukuran `.exe`: 7,3 MB (sesuai perkiraan dari varian D-z). CPU saat hover 0,172 detik-CPU, CPU idle 0%.
- **Dua angka memori yang berbeda jauh.** Working set total (sekitar 18,6 MB) ikut menghitung halaman DLL sistem bersama yang dipetakan ke proses; private working set (sekitar 2,6 MB) hanya menghitung milik proses ini. Metrik resmi proyek ini adalah private working set (bagian 2), dengan commit dicatat sebagai pembanding. Apakah anggaran 20-30 MB dimaksudkan terhadap private working set atau working set total belum dikonfirmasi pengguna.

### Catatan langkah 9a

- `windows-future` seri 0.3 (dipakai `windows 0.62.2`) menamai method blocking `join()`, **bukan** `get()`. Halaman docs.rs yang mengarah ke versi lama menyesatkan; cocokkan dengan `cargo doc --open -p windows-future` bila ragu.
- Panggilan blocking WinRT dijalankan di thread latar, tidak di thread UI.
- Data nyata: judul dari YouTube bisa sangat panjang (puluhan kata) dan "artis" berisi nama kanal. Tampilan di pill 360 px harus memotong teks (elide), satu baris.
- Pesan `[autostart] nilai registry tidak terbaca` di konsol debug normal bila entri Autostart belum ada.

### Hasil ukur langkah 9 (build `dist`, 21 Okt 2026)

Satu kali ukur per titik, hover manual lewat `measure.ps1`.

| Titik | `.exe` MB | Private WS idle | Commit idle | Working set total idle | CPU idle |
| --- | --- | --- | --- | --- | --- |
| Baseline langkah 8 | 7,30 | 2,55 | 3,45 | 18,58 | 0% |
| Setelah 9a (SMTC dibaca sekali saat start) | 7,31 | 2,77 | 3,71 | 30,2 | 0% |
| Setelah 9b (teks digambar, baca saat hover) | 7,33 | 2,89 | 3,88 | 25,13 | 0% |

- Total sejak baseline: private working set sekitar +0,34 MB, commit sekitar +0,43 MB, `.exe` +0,03 MB. Seluruhnya jauh di bawah anggaran 20-30 MB.
- Dugaan sebelumnya bahwa teks pertama kali digambar akan menaikkan RAM lebih besar daripada WinRT **dibantah data**: 9b hanya menambah sekitar 0,12 MB private dan 0,17 MB commit.
- Working set total berfluktuasi (18,6 menjadi 30,2 menjadi 25,1) sementara private working set stabil. Penyebab fluktuasi belum diselidiki; ini mendukung pemakaian private working set sebagai metrik resmi, tetapi definisi anggaran tetap perlu dikonfirmasi pengguna.

### Catatan langkah 9b

- Properti Slint bernama `track-title` dan `track-artist` (di Rust `set_track_title`, `set_track_artist`), karena `Window` sudah punya properti bawaan `title`.
- Pembacaan media dijalankan lewat `refresh_media` di thread latar setiap hover. Penjaga `FETCHING` (`AtomicBool`) melewati pembacaan baru bila yang sebelumnya belum selesai, supaya thread tidak menumpuk saat kursor keluar-masuk cepat. Hasil dikirim ke UI lewat `upgrade_in_event_loop`.
- Teks lama tampil beberapa milidetik sebelum diganti saat expand (sengaja tidak dikosongkan di awal agar tidak berkedip). Pengguna menerima jeda ini.
- `clip: true` pada `Rectangle` pill mencegah teks meluber saat animasi mengecil. Teks disembunyikan lewat `visible`, bukan `opacity`.
- Tanpa media atau saat error, UI menampilkan "Nothing playing".

### Hasil baseline RAM (langkah 4)

Build release, Slint 1.18.1, private working set (Performance counter dan Task Manager konsisten). Baseline kasar, pengukuran resmi tetap di langkah 8.

| Metrik | femtovg (bawaan), idle | femtovg, expanded | software, idle | software, expanded |
| --- | --- | --- | --- | --- |
| Private working set | 42,0 MB | 43,4 MB | 2,3 MB | 2,5 MB |
| CPU | belum diukur | belum diukur | 0% | belum diukur |

- Ukuran `winisland.exe` release: 11,34 MB (kedua renderer ter-compile di default features, jadi ukuran ini belum mencerminkan build software saja).
- Renderer software awalnya diuji lewat `SLINT_BACKEND=winit-software`, kini dikunci lewat kode.
- Setelah ikon tray (langkah 6): private working set sekitar 3 MB saat idle (software renderer, Task Manager).
- Pembacaan Task Manager setelah login (0,5 sampai 0,9 MB) tidak dipakai sebagai baseline. Dugaan (belum terbukti): working set proses idle dipangkas Windows. Pengukuran resmi di langkah 8 mencatat private working set dan commit size setelah satu kali hover dan idle 1 menit.
- Target usulan awal (RAM <= 30 MB, `.exe` <= 10 MB): RAM terpenuhi jauh pada software renderer, ukuran `.exe` belum.

### Sesi lanjutan (21 Okt 2026): verifikasi repo, riset langkah 10, lisensi

**Verifikasi repo.** Repo ditarik dan dibaca (8 commit, `1f93283` sampai `936a38c`). Isinya cocok dengan log di atas: `src/main.rs`, `src/media.rs`, `src/autostart.rs`, `ui/main.slint`, `ui/tray.png`, `setup.ps1`. Tidak ada `LICENSE`, `README`, maupun `CONTRIBUTING`. Angka ukur di pesan commit (`.exe` 7,33 MB, private working set sekitar 2,9 MB) sama dengan tabel langkah 9. Angka itu **tidak diukur ulang** di sesi ini (lingkungan Claude bukan Windows) dan kode tidak dikompilasi.

**Catatan review kode** (belum ada yang diperbaiki):

1. `FETCHING` tidak dikembalikan ke `false` bila thread pembaca media panic. Di build `dist` (`panic = "abort"`) proses mati saja, tetapi di build debug atau `release` biasa penjaga bisa macet dan media tidak pernah menyegarkan lagi. Perbaikan kecil: guard yang mengembalikan flag saat di-drop. Prioritas rendah.
2. `autostart::is_enabled()` mencetak log di setiap panggilan. Tidak berbahaya di release (tanpa konsol), hanya bising di debug.
3. `place()` memusatkan jendela dengan `GetSystemMetrics(SM_CXSCREEN)` (monitor utama) dan tidak dipanggil ulang saat resolusi atau DPI berubah. Sudah masuk daftar "belum diuji".
4. `Cargo.toml` belum punya field `license` dan `description`. Isi setelah lisensi dipilih.

**Anggaran RAM: pertanyaan terbuka yang menentukan.** Private working set stabil di sekitar 2,6-2,9 MB, tetapi working set total tercatat 18,6, lalu 30,2, lalu 25,1 MB (langkah 9). Jika anggaran 20-30 MB dimaksudkan terhadap **working set total**, proyek sudah berada di dalam atau di atas batas itu dan ruang untuk fitur baru sangat sempit. Jika terhadap **private working set** (kolom "Memory" di Task Manager), ruangnya sekitar 17-27 MB. Konfirmasi dari pengguna diperlukan sebelum menambah fitur besar.

**Riset langkah 10 (UserNotificationListener tanpa package identity).**

- Dokumentasi Microsoft yang dibaca: API ini butuh izin eksplisit pengguna dan `RequestAccessAsync` dipanggil dari thread UI. Halaman itu tidak menyebut package identity.
- Sumber sekunder: sebuah proyek pihak ketiga yang memakai API ini menyatakan butuh package identity lewat sparse package, dengan skrip pendaftaran dari PowerShell elevated. Tanya-jawab Microsoft menyatakan WinRT API yang butuh package identity hanya didukung pada aplikasi MSIX atau lewat "packaging with external location".
- Kesimpulan: **kuat dugaan, belum terbukti** untuk build kita. Inferensi tak terverifikasi: aplikasi sejenis yang beredar lewat Microsoft Store otomatis punya identity.
- Jika terbukti butuh identity, ada tiga opsi:

| Opsi | Pro | Kontra |
| --- | --- | --- |
| A. Tunda fitur notifikasi, MVP hanya media | Tidak mengubah distribusi, RAM aman | Kehilangan fitur yang umum di aplikasi sejenis |
| B. Sparse package (package with external location) | Tetap bisa memakai exe biasa | Butuh sertifikat penandatangan yang dipercaya mesin pengguna, instalasi lebih rumit, bertentangan dengan pola "salin exe ke LOCALAPPDATA" |
| C. Distribusi MSIX/Store | Jalur resmi | Mengubah cara distribusi dan build untuk semua kontributor |

- Rekomendasi sementara: jalankan probe dulu (di bawah), lalu putuskan. Jika hasilnya butuh identity, condong ke A untuk sekarang.

**Prosedur langkah 10 (probe, hanya build debug).**

1. Tambahkan feature: `cargo add windows --features UI_Notifications_Management` (nama feature belum diverifikasi; jika ditolak, baca pesan cargo atau `cargo info windows`).
2. Buat `src/notif_probe.rs`:

```rust
//! Probe sementara (hanya build debug): apakah aplikasi unpackaged boleh memakai UserNotificationListener.

use windows::core::Result;
use windows::UI::Notifications::Management::UserNotificationListener;

pub fn probe() -> Result<()> {
    let listener = UserNotificationListener::Current()?;
    let status = listener.RequestAccessAsync()?.join()?;
    eprintln!("[notif] status akses: {status:?}");
    Ok(())
}
```

3. Di `src/main.rs`, tambahkan deklarasi modul (bersama `mod media;`) dan pemanggilan sebelum `slint::run_event_loop()`:

```rust
#[cfg(debug_assertions)]
mod notif_probe;

// di main(), sebelum slint::run_event_loop():
#[cfg(debug_assertions)]
std::thread::spawn(|| {
    if let Err(e) = notif_probe::probe() {
        eprintln!("[notif] GAGAL: {e}");
    }
});
```

4. Jalankan `cargo run` dan catat baris `[notif]`. Tiga hasil yang mungkin:
   - **GAGAL** dengan kode error (mis. terkait package identity): konfirmasi dugaan.
   - **Status akses `Denied` atau `Unspecified`** tanpa prompt: identity atau pengaturan Windows menghalangi.
   - **Status `Allowed`** (mungkin setelah prompt): berfungsi tanpa identity. Langkah berikutnya membaca isi notifikasi.
5. Dokumentasi menyarankan pemanggilan dari thread UI. Probe memakai thread latar lebih dulu (seperti SMTC). Jika hasilnya tidak informatif, ulangi dari thread UI.
6. Probe hanya ada di build debug, jadi RAM dan ukuran `dist` tidak terpengaruh. Hapus setelah keputusan.

**Keputusan lisensi (diputuskan 21 Okt 2026: opsi B; analisis awal di bawah).** Slint dilisensikan rangkap: Royalty-free 2.0, GPLv3, dan Commercial (dibaca dari halaman lisensi Slint; teks berkas untuk versi 1.18.1 belum dibuka langsung, cek `LICENSE.md` di crate yang terpasang). Ini bukan nasihat hukum.

| Opsi | Isi | Pro | Kontra |
| --- | --- | --- | --- |
| A. Seluruh proyek GPL-3.0 | Slint dipakai lewat jalur GPLv3; karya lengkap tunduk GPLv3 | Jalur paling sederhana, tanpa kewajiban atribusi, semua kontribusi satu lisensi | Fork tertutup tidak mungkin; sulit berubah ke lisensi permisif atau proprietari tanpa izin semua kontributor |
| B. Kode sendiri MIT (atau Apache-2.0), Slint lewat Royalty-free 2.0 | Atribusi Slint wajib, misalnya badge di README atau widget `AboutSlint` | Kontribusi MIT fleksibel dan bisa direlisensi; tanpa biaya RAM bila cukup badge | Syarat atribusi harus dipenuhi, termasuk oleh fork yang membagikan biner; teks lengkap 2.0 belum terbaca utuh; bergantung pada ketentuan Slint yang bisa berubah pada versi mendatang |

Rekomendasi sementara: **B**, dengan syarat teks Royalty-free 2.0 dibaca utuh dan memang cukup dengan badge di README (bukan widget di aplikasi). Pilih **A** bila tidak ingin mengurus atribusi dan ingin keputusan paling sederhana. Setelah memilih: tambahkan `LICENSE`, badge dan catatan di `README`, `CONTRIBUTING` (aturan renderer software terkunci, daftar feature Slint sengaja dipangkas, efek seperti `drop-shadow-*` tidak tampil), dan isi `license` di `Cargo.toml`. Lisensi dependensi lain (`windows`, `winreg`, `raw-window-handle`) belum diperiksa; cek dengan `cargo-deny` atau `cargo-license`.

### Keputusan pengguna (21 Okt 2026) dan tindak lanjut

**RAM fleksibel.** Tidak ada anggaran keras. Metrik yang dicatat tetap private working set (Task Manager, kolom "Memory") plus commit, per fitur baru, tanpa ambang lulus/gagal. Risiko yang diterima: tidak ada gerbang otomatis untuk fitur berat. Pengaman yang dipertahankan: `measure.ps1` dan catatan angka per langkah, dan pengguna meninjau ulang bila angka melonjak tidak wajar.

**Lisensi: opsi B** (kode sendiri MIT, Slint lewat Royalty-free 2.0). Teks Royalty-free 2.0 sudah dibaca utuh dari repo Slint:

- Atribusi: cukup salah satu dari (a) widget `AboutSlint` di layar "About" yang bisa dijangkau dari menu level atas aplikasi, atau (b) badge Slint di halaman web publik, sebaiknya tempat biner diunduh. Rencana: **(b)**, badge di `README.md` (tanpa biaya RAM di aplikasi).
- Pembatasan yang relevan: tidak boleh mendistribusikan Slint sendiri tanpa integrasi ke aplikasi, tidak untuk sistem embedded, dan tidak untuk aplikasi yang mengekspos API Slint. `winisland` tidak melakukan satu pun.
- Lisensi dependensi langsung (dari crates.io): `windows` MIT OR Apache-2.0, `winreg` MIT, `raw-window-handle` MIT OR Apache-2.0 OR Zlib. Semuanya kompatibel dengan MIT. Sekitar 538 crate di `Cargo.lock` (termasuk transitif) **belum diperiksa**; jalankan `cargo-deny` atau `cargo-license` sebelum rilis biner pertama.
- Hak cipta di `LICENSE` memakai handle GitHub `irfanadwifangga`; ganti dengan nama lengkap bila diinginkan.

**Berkas yang disiapkan** (folder `repo-docs/`): `LICENSE` (MIT), `README.md` (fitur, status, batasan, build, bagian lisensi dengan badge Slint), `CONTRIBUTING.md` (aturan proyek). Perubahan `Cargo.toml` di bagian `[package]`: `description`, `license = "MIT"`, `repository`, dan `publish = false` (opsional, mencegah terbit tidak sengaja ke crates.io).

**Langkah 10.** Probe di commit `50d7110` dijalankan (21 Okt 2026). Hasil:

```
[notif] status akses: UserNotificationListenerAccessStatus(1)
```

Status `1` = `Allowed`. **Aplikasi unpackaged bisa mengakses `UserNotificationListener` tanpa package identity.** Dugaan sebelumnya (butuh identity) **dibantah data**. Opsi A/B/C (tunda / sparse package / MSIX) tidak perlu dipilih.

Catatan: pesan commit `50d7110` masih memuat placeholder di baris verified. Tidak di-force-push; hasil sebenarnya dicatat di commit dan dokumen ini.

Langkah selanjutnya: membaca daftar notifikasi aktif lewat `GetNotificationsAsync`, menampilkan di overlay.

### Hasil langkah 9c-9d (21 Okt 2026)

**Arsitektur akhir (`src/media.rs`).** Satu worker thread permanen (`spawn_watcher`) menunggu `Signal` lewat `recv()` (CPU 0% saat idle). Handler `CurrentSessionChanged`, `MediaPropertiesChanged`, dan `PlaybackInfoChanged` hanya mengirim sinyal. Worker membaca berurutan, menggabungkan sinyal susulan selama 150 ms (debounce), mengirim ke UI hanya bila hasil berbeda dari kiriman terakhir (dedup), dan melepas listener sesi lama (`RemoveMediaPropertiesChanged` / `RemovePlaybackInfoChanged`, token `i64`) saat sesi berganti. `MediaHandle` (clone-able) menyediakan `refresh()` (dipanggil saat hover), `toggle_play_pause()`, `skip_next()`, `skip_previous()`; perintah dijalankan segera tanpa menunggu debounce dan tanpa `join()`.

**UI (`ui/main.slint`).** Pill expanded 360x116 (jendela 360x124, `FULL_H = 124`). Baris tiga tombol bulat 30 px di bawah teks, hanya ada bila ada sesi media. Tombol tengah menampilkan aksi berikutnya (pause saat diputar, play saat dijeda). Ikon dari `Rectangle`.

| Titik | `.exe` MB | Private WS idle | Commit idle | WS total idle | CPU idle |
| --- | --- | --- | --- | --- | --- |
| Setelah 9b | 7,33 | 2,89 | 3,88 | 25,13 | 0% |
| Setelah 9c (event + worker) | 7,34 | 2,83 | 3,93 | 25,01 | 0% |
| Setelah 9d-1 (ikon status) | 7,35 | 2,93 | 3,98 | 25,42 | 0% |
| Setelah 9d-2 (tombol) | tidak diukur sendiri; lihat "Hasil langkah 10e dan pengukuran RAM" |  |  |  |  |

- Dugaan bahwa thread permanen dan langganan event menaikkan RAM **dibantah data** (9c: private WS turun 0,06 MB, dalam derau). CPU hover 0,172 dtk-CPU (9c) dan 0,109 (9d-1).
- Uji fungsional: 9c lulus (ganti lagu, pindah pemutar, tanpa media, skip cepat), 9d-1 lolos semua, 9d-2 lolos semua (pill tidak collapse di atas tombol, klik tidak mencuri fokus, ikon ikut berganti, previous/next/play-pause bekerja, klik beruntun cepat aman, tata letak muat). Semuanya laporan pengguna.
- Commit: `69f178f` (9c), `2fbf5a7` (9d-1), `2e20893` (9d-2).

### Hasil langkah 10b (21 Okt 2026)

Probe di `src/notif_probe.rs` (debug-only) membaca daftar toast aktif dan mencoba mendaftarkan `NotificationChanged`. Ringkasan keluaran konsol:

- `GetNotificationsAsync(Toast)`: **27 notifikasi** terbaca, masing-masing dengan id, nama aplikasi (`AppInfo().DisplayInfo().DisplayName()`), dan teks (`Visual().GetBinding(ToastGeneric).GetTextElements()`), tanpa error.
- Setelah daftar awal, probe berhenti dengan `FAILED: Element not found. (0x80070490)`. Baris `NotificationChanged registered` tidak pernah tercetak, sehingga kegagalan ada pada **registrasi event** (kesimpulan dari urutan log). Polling pembanding di probe belum sempat berjalan.

**Pengamatan dari data nyata (penting untuk desain):**

1. Daftar berisi **isi Action Center**, bukan hanya notifikasi baru. Ada banyak entri lama; tanpa event, "notifikasi baru" hanya bisa diketahui dengan membandingkan daftar antar-pembacaan.
2. **Duplikat**: teks yang sama muncul berkali-kali (Antigravity 5x dengan judul sama, NVIDIA App 5x). Perlu dedup/pengelompokan per (aplikasi, teks).
3. **Urutan daftar tidak menurut id** (id 75225 muncul paling akhir setelah 75265). Jangan mengandalkan urutan; pakai `CreationTime()` untuk menentukan yang terbaru.
4. **Isi sensitif** terbaca penuh: pesan WhatsApp (nama grup, pengirim, isi), keluaran agen AI (perintah, path, data mesin). Bila ditampilkan di overlay yang selalu di atas semua jendela, isi bisa terlihat orang lain atau saat berbagi layar.
5. Teks bisa berisi newline, markdown, emoji, dan karakter tak terlihat U+200E (WhatsApp); sebagian dipotong oleh aplikasi pengirimnya. Rendering emoji di software renderer belum diuji.

**Konsekuensi:** pola event (opsi A di analisis) **gugur** untuk build unpackaged ini. Sisa opsi:

| Opsi | Pro | Kontra |
| --- | --- | --- |
| **B. Baca saat pill expanded** (lewat `refresh()` yang sudah ada, tanpa event) | Nol biaya idle, arsitektur worker sudah siap, tidak ada polling | Notifikasi yang masuk saat pill collapsed baru terlihat setelah hover; "baru" harus ditentukan dengan diff daftar |
| **B+. B ditambah polling lambat** (mis. 5-10 detik) untuk memunculkan notifikasi baru otomatis | Fitur "peek" ala Dynamic Island mungkin dilakukan | Wake-up periodik bertentangan dengan prioritas RAM/CPU idle; biaya harus diukur |
| **C. Tunda notifikasi** | Nol risiko dan nol RAM | Kehilangan fitur utama aplikasi sejenis |

**Rekomendasi (dari analisis sesi ini, belum diputuskan pengguna):** mulai dari **B**, ukur RAM, dan pertimbangkan B+ hanya bila memang perlu. **Privasi: default hanya nama aplikasi + judul, tanpa isi**, dengan opsi menampilkan isi di menu tray. Tampilan butuh mode sendiri di pill (pill 116 px sudah terisi teks dan tombol media).

**Bukti tambahan (pencarian, 21 Okt 2026):** beberapa laporan komunitas independen (tanya-jawab WinForms dan WPF, plus proyek pihak ketiga yang membaca notifikasi) menggambarkan gejala yang **identik**: `RequestAccessAsync` mengembalikan `Allowed`, tetapi mendaftarkan `NotificationChanged` melempar `0x80070490`; perbaikannya selalu package identity (proyek WPF dibungkus Windows Application Packaging Project, atau paket sparse dengan skrip pendaftaran identitas yang dijalankan sebagai administrator). Ini memperkuat dugaan penyebab, tetapi sumbernya komunitas, bukan pernyataan Microsoft untuk API ini. Satu laporan menyebut pembacaan notifikasi juga butuh identity; data probe kita **membantahnya untuk `GetNotificationsAsync` di mesin ini**. Perilaku itu baru teruji di satu mesin dan satu build Windows, jadi bisa berbeda di mesin lain.

**Kebersihan data:** isi notifikasi nyata (nama, potongan pesan) tidak boleh masuk repo publik, issue, atau pesan commit. Pencarian di HEAD dan riwayat commit pada 21 Okt 2026 tidak menemukannya.

**Risiko yang masih terbuka:** perilaku Focus/Do Not Disturb (apakah notifikasi yang ditekan tetap muncul di daftar) belum diuji; notifikasi yang di-update aplikasi yang sama (mis. progres) bisa membuat tampilan berkedip; biaya RAM membaca daftar berulang belum diukur.

### Hasil langkah 10c (21 Okt 2026)

Kode: `src/notif.rs` (pembacaan WinRT, hanya daftar) dan `src/notif_text.rs` (logika murni tanpa API Windows: `clean`, `group`, `age_secs`), menggantikan `notif_probe.rs`. `debug_run` (build debug) membaca tiga kali dan mencetak ringkasan; isi pesan baru dibaca bila variabel lingkungan `WINISLAND_NOTIF_BODY` diset.

Hasil (laporan pengguna, Windows, build debug):

| Pembacaan   | Tanpa isi | Dengan isi |
| ----------- | --------- | ---------- |
| #1 (dingin) | 49,2 ms   | 26,8 ms    |
| #2          | 14,6 ms   | 14,6 ms    |
| #3          | 12,1 ms   | 12,7 ms    |

- `cargo test`: 13 tes lolos di Windows. Daftar saat uji hanya berisi **1 notifikasi**, jadi pengurutan dan dedup pada data nyata belum teramati (hanya lewat unit test).
- Mode tanpa isi tidak mencetak isi pesan: terbukti isi tidak dibaca di mode itu.
- Umur notifikasi bertambah tepat 1 detik per pembacaan, jadi `CreationTime` dan konversi tick berfungsi.
- Waktu baca adalah waktu dinding untuk 1 notifikasi. Biaya untuk daftar besar (puluhan notifikasi, tiap notifikasi memicu beberapa panggilan lintas proses) dan biaya CPU belum diukur; ini masukan untuk keputusan B+ bila diangkat lagi.

**Temuan desain:** judul notifikasi sering sama dengan nama aplikasi (Antigravity: judul "Antigravity", informasi ada di isi), sedangkan aplikasi lain memakai judul informatif (Hermes, Snipping Tool) atau nama obrolan (WhatsApp). Keputusan: bila judul sama dengan nama aplikasi, tampilkan sekali saja (kosongkan judul sebelum dedup); isi tetap hanya lewat sakelar privasi. Opsi menampilkan isi otomatis untuk aplikasi tertentu ditolak karena privasinya bergantung pada kebetulan nama aplikasi.

### Hasil langkah 10d (22 Okt 2026)

**Status:** lolos uji pengguna (22 Okt 2026, laporan pengguna). Kode disiapkan oleh Claude; sandbox Claude tidak punya Rust untuk target Windows, jadi kode Rust tidak pernah dikompilasi. Satu-satunya verifikasi mesin: berkas `ui/main.slint` lolos kompilasi dengan paket Python `slint` 1.18.1b1 (versi pustaka Slint pengguna 1.18.1) dan berkas yang sengaja dirusak ditolak, jadi pengecekan itu bermakna. Perilaku (hover, klik, tata letak) tidak bisa dijalankan tanpa layar, jadi belum teramati.

**Yang berubah:**

- `src/notif_text.rs`: `group()` mengosongkan judul yang sama dengan nama aplikasi (`eq_ignore_ascii_case`, setelah `clean`) **sebelum** penggabungan; fungsi baru `format_age()` ("just now", "N min ago", "N h ago", "N d ago"); 4 tes baru (total 17).
- `src/main.rs`: `refresh_notifications()` membaca di thread latar setiap pill dibuka (opsi B, tanpa event dan tanpa polling), dengan penjaga `NOTIF_FETCHING` berbentuk RAII (`NotifFetchGuard`) supaya flag tidak macet bila thread panic; `apply_notifications()` mengisi properti UI di thread UI. Isi pesan hanya dibaca bila variabel lingkungan `WINISLAND_NOTIF_BODY` diset (sakelar sementara sampai 10e).
- `ui/main.slint`: struct `NotifRow` (header, title, body, age; semuanya teks siap tampil), komponen `CountBadge`, properti `notif-total`, `notifs`, `notif-index`, `show-notifs`, tampilan notifikasi yang menggantikan tampilan media (tinggi pill tetap 116 px, jendela tetap 360x124), `changed expanded` yang mengembalikan ke tampilan media dan item pertama setiap pill dibuka.

**Keputusan desain dan alasan:**

| Keputusan | Alasan | Risiko / catatan |
| --- | --- | --- |
| Daftar dan indeks disimpan di model Slint, navigasi dilakukan di Slint | Penutup untuk `upgrade_in_event_loop` harus `Send`, sehingga status `Rc`/`RefCell` di Rust tidak bisa dibawa masuk | Aritmetika indeks ada di Slint; sintaks interpolasi `"\{...}"` sudah lolos kompilasi |
| Klik kartu berpindah ke item berikutnya (berputar), posisi tampil sebagai "1/3" | Tanpa tombol navigasi tambahan: tinggi 116 px sempit, ikon prev/next menambah elemen dan tabrakan tata letak | Penemuan fitur rendah; bila terasa kurang, tambah tombol di langkah berikutnya |
| Lencana merah di kanan bawah, berpindah media/notifikasi; putih saat tampilan notifikasi aktif | Area kanan bawah kosong di tampilan media (tombol berada di tengah) dan tampilan notifikasi diberi `padding-right: 56px` | **Angka tata letak adalah perkiraan awal**, belum pernah dirender oleh Claude. Kemungkinan perlu disetel secara visual |
| Lencana menampilkan `Snapshot::total` (notifikasi mentah di Action Center), daftar menampilkan maksimal 5 kelompok | Sesuai handoff ("angka total notifikasi aktif") | Angka lencana bisa lebih besar dari jumlah item yang bisa digulir |
| Bila akses ditolak atau pembacaan gagal: daftar dikosongkan, lencana hilang, hanya log konsol | Sederhana | Pengguna tidak diberi tahu bahwa akses dimatikan. Kandidat perbaikan di 10e |
| Bawaan tampilan awal selalu media | Sederhana dan konsisten | Bila tidak ada media tetapi ada notifikasi, pengguna harus mengklik lencana |

**Checklist verifikasi (untuk pengguna):**

1. `cargo fmt` lalu `cargo check` bersih; `cargo test` melaporkan 17 tes lolos.
2. Hover: pill terbuka mulus seperti sebelumnya; saat ada notifikasi di Action Center, lencana angka muncul dalam sekitar 50 ms.
3. Klik lencana: tampilan beralih media ke notifikasi dan kembali; tombol media tetap bekerja di tampilan media.
4. Tampilan notifikasi: nama aplikasi, judul (tanpa isi), umur; "Antigravity" tidak tampil sebagai "Antigravity | Antigravity".
5. Klik kartu berpindah item; indeks "n/m" tampil bila lebih dari satu item.
6. Pill ditutup lalu dibuka lagi: kembali ke tampilan media.
7. Tanpa `WINISLAND_NOTIF_BODY` isi tidak tampil; dengan variabel itu isi tampil.
8. Ukur RAM (`measure.ps1`, build `dist`) dan catat di tabel.

### Hasil langkah 10e dan pengukuran RAM (22 Okt 2026)

**10e (lolos uji pengguna):** `src/config.rs` (baru), item menu tray "Notification body: Hidden/Shown" (judul teks, bukan `checkable`, seperti Autostart), `config::load()` di awal startup, env var `WINISLAND_NOTIF_BODY` dihapus dari jalur fetch (tersisa hanya di `debug_run` lewat `config::show_body()`). Tinggi pill tidak berubah: baris body (12 px, satu baris, elide) sudah ada di UI 10d dan muat di 116 px.

#### Hasil Bisect & Verifikasi RAM (22 Okt 2026)

Pengukuran bisect menggunakan `measure.ps1` pada build `dist` membuktikan secara definitif sumber kenaikan RAM:

| Varian | A (awal) | B (setelah hover) | C (idle 60s) | Commit C |
|---|---|---|---|---|
| 9d-1 | 2,90 MB | 2,93 MB | 2,87 MB | 3,96 MB |
| 9d-2 | 2,88 MB | 2,86 MB | 2,83 MB | 3,94 MB |
| 10d | 2,88 MB | **4,31 MB** | **4,25 MB** | 5,49 MB |
| 10e | 2,89 MB | **4,44 MB** | **4,37 MB** | 5,64 MB |
| **HEAD-nofetch** | 2,88 MB | **2,93 MB** | **2,88 MB** | 3,98 MB |

**Temuan & Kesimpulan:**
1. **Seluruh kenaikan (+1,4 MB) berasal dari jalur pembacaan notifikasi (WinRT fetch).** Varian `HEAD-nofetch` yang memiliki UI dan binary identik dengan 10e (hanya mematikan satu baris `refresh_notifications`) mencatatkan RAM idle 2,88 MB (identik dengan baseline 9d-2).
2. **Hipotesis ukuran UI memperbesar RAM startup terbantahkan.** Fase A stabil di ~2,88-2,90 MB di semua varian.
3. **Keputusan Pengguna:** Pengguna menerima kenaikan ~1,4 MB ini (RAM idle total ~4,3 MB masih sangat jauh di bawah batas fleksibel 20-30 MB) sebagai harga wajar untuk fitur pembacaan notifikasi, dan memilih melanjutkan pengembangan.

### Riwayat commit

Isi dari output `git log --oneline` (pesan commit sebenarnya, bukan rencana):

1. `1f93283` feat: add Slint pill overlay prototype centered at top of screen
2. `f76017d` feat: Slint overlay hidden from taskbar, idle top-edge strip with hover expand
3. `efb2fd1` feat: float pill below top edge while expanded
4. `6c81196` feat: system tray icon with show/hide and quit
5. `197ce5c` feat: autostart toggle in tray menu via HKCU Run entry
6. `496cf16` build: drop femtovg and add size-optimized dist profile (langkah 8)
7. `ab409de` feat: read now-playing title and artist via SMTC (console probe) (langkah 9a)
8. `936a38c` feat: show now-playing title and artist in the expanded overlay (langkah 9b)
9. `b2bdb01` docs: add MIT license, README and CONTRIBUTING; declare license in Cargo.toml
10. `50d7110` feat: add debug-only probe for UserNotificationListener access
11. `5b472f2` chore: add doc comments, fix formatting, and update plan with probe result
12. `69f178f` feat: update now-playing text automatically via SMTC events (langkah 9c)
13. `2fbf5a7` feat: show play/pause status icon in the expanded overlay (langkah 9d-1)
14. `2e20893` feat: add previous, play/pause and next buttons to the expanded overlay (langkah 9d-2)

15. `f891c66` feat: probe reading notification contents and test NotificationChanged event (langkah 10b)
16. `da4b621` docs: repair plan corrupted by unexpanded regex back-references
17. `7c406ae` docs: tick the remaining checklist items and update the stale target table
18. `25301b7` feat: read, clean and group active toast notifications (langkah 10c)
19. `733be0a` docs: fill in commit hashes 7c406ae and 25301b7 in plan
20. `4c353e9` feat: show notifications in the expanded overlay with badge and mode toggle (langkah 10d)
21. `b2ee857` feat: add notification body privacy toggle to the tray menu (langkah 10e)
22. `ab3a2f2` docs: record step 10e RAM bisect results and update commit history
23. `8ab082e` feat: auto-switch to notifications on expand when no media is playing
24. `12da5b5` feat: add multi-monitor and DPI-aware placement with tray selector (Opsi A)
25. `628f719` feat: auto-hide overlay when foreground app is fullscreen with tray toggle

### Multi-Monitor & DPI Aware Placement (22 Okt 2026)

- **Masalah:** Fungsi `place()` sebelumnya memakai `GetSystemMetrics(SM_CXSCREEN)` yang hanya mengambil lebar monitor utama dan mengasumsikan koordinat mulai dari (0, 0). Pada desktop multi-monitor, koordinat bisa memiliki offset atau bernilai negatif, dan jendela tidak bisa dipindahkan ke monitor sekunder.
- **Penyelesaian (Opsi A - Monitor Tetap + Pilihan di Tray):**
  - Membuat modul `src/monitor.rs` menggunakan Win32 GDI (`EnumDisplayMonitors`, `GetMonitorInfoW`, `MONITORINFO`). Monitor utama selalu diurutkan di indeks 0.
  - Koordinat penempatan dihitung presisi: `x = mon.left + (mon.width - pw) / 2` dan `y = mon.top`.
  - Item menu baru di Tray `display-label` ("Display: Primary", "Display: 2", dst.) dengan callback `cycle-display` untuk berpindah monitor secara dinamis.
  - Preferensi monitor disimpan persisten di registry `HKCU\Software\winisland\MonitorIndex`.
- **Verifikasi:** 20 unit tests lolos (termasuk 3 tes baru di `monitor.rs`), `cargo check` dan `cargo fmt` bersih.

Hash diambil dari `git log --oneline` pada repo (22 Okt 2026).

### Deteksi Aplikasi Fullscreen (Game / Video) (22 Okt 2026)

- **Masalah:** Karena overlay disetel `always-on-top`, strip idle 4 px di tepi atas layar tetap terlihat atau berisiko terklik saat bermain game layar penuh (exclusive atau borderless) maupun menonton video layar penuh (YouTube, VLC, dsb).
- **Penyelesaian:**
  - Membuat modul `src/fullscreen.rs` yang memeriksa jendela aktif latar depan (`GetForegroundWindow`).
  - Membandingkan batas koordinat jendela dengan monitor tempat overlay berada (`rect_covers_monitor`), serta memeriksa window style (`!has_caption && !is_child`).
  - Mengabaikan desktop dan shell (`Progman`, `WorkerW`, `Shell_TrayWnd`, `Shell_SecondaryTrayWnd`, `Windows.UI.Core.CoreWindow`).
  - Timer Slint 500 ms di UI loop memantau keadaan secara berkala (0% CPU idle, tanpa hook global yang memicu anti-cheat game).
  - Sakelar "Hide in fullscreen: On / Off" di menu Tray yang disimpan persisten di registry `HKCU\Software\winisland\HideOnFullscreen` (bawaan: true).
  - Membedakan sembunyikan manual pengguna (`user_hidden`) dengan supresi otomatis fullscreen (`fs_suppressed`).
- **Verifikasi:** 27 unit tests lolos (7 tes baru di `fullscreen.rs`), `cargo check` dan `cargo fmt` bersih.
- **Uji Pengguna (22 Okt 2026): Lolos.** Mode fullscreen video browser (tombol F) menyembunyikan strip idle ~0,5 dtk, tombol Esc memunculkan kembali, sakelar "Hide in fullscreen" di menu tray berfungsi mematikan auto-hide saat dibutuhkan.

### Langkah 12: Kerangka Tab dan Ukuran Dinamis (22 Okt 2026)

- **Masalah:** Konstanta ukuran `FULL_W` (360) dan `FULL_H` (124) sebelumnya di-hardcode di `main.rs` dan duplikat dengan nilai di `ui/main.slint`. Setiap tab di Fase 2 membutuhkan ukuran jendela berbeda.
- **Penyelesaian:**
  - Menjadikan Slint sebagai **sumber kebenaran tunggal** untuk ukuran jendela (`requested-width`, `requested-height`).
  - Menghapus konstanta `FULL_W` dan `FULL_H` di `main.rs`, digantikan oleh batas atas aman (`MAX_EXPANDED_W = 480.0`, `MAX_EXPANDED_H = 300.0`, `MIN_WINDOW_W = 140.0`, `MIN_WINDOW_H = 4.0`).
  - Fungsi `clamp_window_size` dan `current_window_size` membaca properti Slint dan memastikan ukuran tidak melebihi batas.
  - Callback `request-window-resize` di Slint memicu Rust mengubah ukuran jendela secara instan saat tab berganti atau status media berubah.
  - Komponen `TabItem` dan bar tab navigasi (Home, Calendar, Timer, Mail) dengan highlight status aktif dan interaksi hover.
  - Tab terakhir yang dipilih diingat di memori (`active-tab`), dipulihkan otomatis saat expand berikutnya.
  - Placeholder dibuat untuk tab Calendar, Timer, dan Mail, siap diisi pada langkah 14, 15, dan 17.
- **Verifikasi:** 30 unit tests lolos (3 tes baru untuk `clamp_window_size`), `cargo check` dan `cargo fmt` bersih.

## 9. Fase 2: Home, tab, dan ukuran dinamis (22 Okt 2026)

### 9.1 Permintaan pengguna

1. **State Home.** Saat tidak ada aktivitas, island masuk ke Home, bukan "Nothing playing". Home menampilkan jam, notifikasi 1 baris, dan statistik RAM, CPU, GPU, baterai (dan metrik lain yang berguna).
2. **Tab** untuk berpindah dari Home: Kalender; Timer/Stopwatch/Jadwal; Email atau hal lain yang berguna.
3. **Ukuran island menyesuaikan konten.** Batas ukuran lama (360x116) boleh dilampaui.
4. **Ditunda ke backlog:** perapihan UI (sekarang masih berantakan) dan font yang sebagian tidak terbaca. Perencanaan RAM Fase 2 diabaikan dulu dan diuji belakangan.

### 9.2 Tafsir yang saya pakai (koreksi bila keliru)

- Garis idle di tepi atas **tetap ada**. "Home" adalah isi tampilan saat pill di-expand dan tidak ada aktivitas lain, menggantikan "Nothing playing". Bila yang dimaksud Home selalu tampil tanpa perlu hover, itu mengubah keputusan klik-tembus (bagian 8) dan harus dibahas lebih dulu.
- Fitur notifikasi dan media yang sudah selesai dipertahankan; keduanya dipindah ke dalam kerangka Home/tab, bukan dibuang.

### 9.3 Analisis kritis: tiga benturan dengan keputusan sebelumnya

| Benturan | Mengapa | Penyelesaian yang diusulkan |
| --- | --- | --- |
| **Ukuran dinamis vs klik-tembus.** Ukuran jendela sama dengan area yang menghalangi klik (temuan langkah 3, terkonfirmasi). Island yang lebih besar menutupi lebih banyak jendela di bawahnya. | Pro dinamis: sesuai konsep, konten tidak dipaksa muat. Kontra: tiap piksel tambahan di tepi atas tengah memblokir tombol aplikasi lain. | Boleh membesar, tetapi **per state ada ukuran sendiri dan ada batas atas** (usulan awal 480x300 logis, belum disepakati). Mekanisme yang sudah terbukti dipertahankan: jendela membesar sebelum animasi, mengecil 280 ms setelah collapse. |
| **Statistik live vs CPU idle 0%.** CPU, RAM, jam butuh pembaruan berkala. | Polling terus-menerus merusak prinsip utama proyek. | **Polling hanya selama pill expanded**: timer Slint mulai saat expand, berhenti saat collapse. Saat idle tidak ada timer. Jam cukup tick 1 detik, dan hanya saat expanded. |
| **Konstanta ukuran tersebar.** `FULL_H` di `main.rs` dan angka 8 px ada di dua tempat (`main.slint` dan `main.rs`, sudah tercatat di bagian 8). Ukuran per state akan melipatgandakan masalah ini. | Ukuran harus dua arah sinkron. | Langkah 12 merapikan: **Slint menjadi sumber kebenaran ukuran** per state, Rust hanya membaca ukuran lalu memanggil `place()`. Pendekatan ini belum diuji; verifikasi cara membaca properti Slint dari Rust (`changed` callback) di dokumentasi 1.18.1 sebelum menulis. |

### 9.4 Rancangan tampilan

**Kerangka:** bar tab di bagian atas pill (Home | Calendar | Timer | Mail), konten di bawahnya, tinggi mengikuti tab aktif. Tab terakhir yang dipakai diingat di memori dan dipulihkan setiap pill di-expand.

**Home (tab bawaan):**

- Baris atas: jam (HH:MM, detik opsional) dan tanggal.
- Satu baris notifikasi terbaru (aplikasi + judul, elide). Klik membuka tampilan notifikasi yang sudah ada.
- Statistik: CPU %, RAM %, baterai % (dan status mengisi). GPU menyusul (langkah 16). Bila perangkat tanpa baterai, baris baterai disembunyikan.
- Bila ada sesi media: baris media (judul + tiga tombol yang sudah ada) muncul di Home. Ini memakai ukuran dinamis (Home lebih tinggi saat media ada). Lihat keputusan 1 di 9.6.

**Kalender:** grid bulan dihitung lokal dari tanggal sistem (hari dalam seminggu dihitung manual atau lewat `GetLocalTime`, tanpa dependensi baru). Acara nyata dari kalender Windows belum dirancang: API Appointments mungkin butuh package identity (**belum diverifikasi**, probe seperti 10a bila diinginkan).

**Timer/Stopwatch/Jadwal:**

- Stopwatch dan timer murni lokal. Waktu berjalan dihitung dari `Instant` (selisih), bukan dari hitungan tick, sehingga tetap benar walau tick berhenti saat collapse.
- **Alarm timer selesai** saat pill collapsed: dibangunkan satu kali pada waktu target (satu thread tidur atau waitable timer, tanpa polling), lalu expand otomatis beberapa detik. Dugaan: CPU idle tetap 0%; **harus diukur**.
- Jadwal (pengingat): butuh penyimpanan. Lihat keputusan 3.

**Email/lainnya:** slot tab dicadangkan. Email adalah fitur paling mahal di daftar ini (jaringan, TLS, OAuth, penyimpanan token, repo publik sehingga tidak boleh ada secret). Rekomendasi: kerjakan terakhir dan jadikan opsional; isi tab bisa ditentukan setelah langkah 15 selesai.

### 9.5 Cara membaca statistik (perlu verifikasi sebelum menulis kode)

| Metrik | Kandidat API | Catatan |
| --- | --- | --- |
| CPU % | `GetSystemTimes` (dua pembacaan, selisih) | Ringan, tanpa dependensi baru; butuh feature `windows` yang sesuai (verifikasi nama) |
| RAM % | `GlobalMemoryStatusEx` | Ini beban memori sistem, bukan RAM proses winisland |
| Baterai | `GetSystemPowerStatus` | Perangkat tanpa baterai mengembalikan status "tidak ada" |
| GPU | PDH counter "GPU Engine" (lintas vendor) atau NVML (hanya NVIDIA) | Paling berisiko; vendor dan versi Windows memengaruhi. **Probe dulu di mesin pengguna** (langkah 16), jangan ditebak |

### 9.6 Keputusan pengguna (22 Okt 2026)

1. **Posisi media: baris di dalam Home** saat ada sesi (ukuran Home dinamis). Tombol media dan tampilan notifikasi lama dipindah ke kerangka ini.
2. **Tab saat expand: mengingat tab terakhir** (berbeda dari rekomendasi awal "selalu Home"). Tafsir yang dipakai: diingat **selama aplikasi berjalan, di memori**, tidak disimpan ke registry. Konsekuensi yang diterima: setelah dibuka dari tab Timer, pill langsung menampilkan Timer; Home hanya terlihat bila tab Home yang terakhir dipakai. Mengingat lintas restart bisa ditambah nanti bila diinginkan (satu nilai di `HKCU\Software\winisland`).
3. **Penyimpanan jadwal: berkas teks sederhana** di `%APPDATA%\winisland\`, tanpa dependensi baru (sesuai rekomendasi). Format dan parsing ditulis sendiri; tulis ke berkas sementara lalu ganti nama supaya tidak rusak bila proses mati saat menulis.

### 9.7 Urutan pengerjaan (tambahkan ke tabel "Log langkah")

| # | Isi | Status |
| --- | --- | --- |
| 12 | Kerangka: model state (tab), ukuran jendela per state dengan Slint sebagai sumber kebenaran, batas ukuran | Selesai (30 tes lolos) |
| 13 | Home: jam, notifikasi 1 baris, statistik CPU/RAM/baterai, timer polling hanya saat expanded; ukur RAM dan CPU | Direncanakan |
| 14 | Bar tab + Kalender (grid bulan lokal) | Direncanakan |
| 15 | Timer dan Stopwatch (alarm dengan peek), lalu Jadwal | Direncanakan |
| 16 | Statistik GPU (probe dulu, lalu implementasi bila layak) | Direncanakan |
| 17 | Tab Email atau lainnya (probe kelayakan, putuskan setelah 15) | Direncanakan |

Tiap langkah memakai alur lama: perubahan kecil, checklist, commit, perbarui dokumen. Langkah 12 harus lolos uji klik-tembus dan multi-monitor yang sudah ada sebelum lanjut ke 13.

### 9.8 Backlog (disepakati ditunda)

- Perapihan UI (tata letak, jarak, warna), termasuk angka tata letak 10d yang masih perkiraan.
- Font yang sebagian tidak terbaca.
- Pesan di pill bila akses notifikasi dicabut (sekarang hanya log konsol).
- Uji judul non-Latin dan emoji di software renderer; uji `drop-shadow-*`.
- Penjaga `FETCHING` pada media bila thread panic (prioritas rendah).
- Perbarui `README.md` untuk fitur baru dan jalankan `cargo-deny` atau `cargo-license` sebelum rilis biner pertama.
- Penyelidikan RAM Fase 2: diukur setelah fitur jalan, bukan direncanakan sekarang.

### 9.9 Risiko Fase 2

1. Home dan tab menambah banyak kode UI sekaligus; kerjakan per langkah di 9.7, jangan sekaligus.
2. Ukuran dinamis tanpa batas atas bisa membuat island menutupi tombol aplikasi lain.
3. GPU dan Email bisa gagal di probe; rencana tidak boleh bergantung pada keduanya.
4. Semua kode Rust tetap hanya diverifikasi lewat `cargo check` dan uji pengguna; perilaku UI yang tidak bisa dirender Claude harus dikonfirmasi lewat laporan pengguna.
