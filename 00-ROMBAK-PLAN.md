# AYUN — Rencana Rombak Total (MENUNGGU APPROVAL NODAL)

Status: **rencana saja. Belum ada baris kode yang diubah, belum ada push, belum ada build.**
Riset: 5 agen paralel + web search (kompetitor, loop ketagihan, seni low-poly, lore, kelayakan teknis).
Tanggal: 2026-11-26.

---

## 1. Positioning brief (draft — perlu dikonfirmasi Nodal)

| Axis | Isi |
|---|---|
| Identity | "Little World" — diorama low-poly 3D yang terasa seperti mainan di meja. Ceria, hangat, ada bekas tangan manusia (ukiran, jitter, perbaikan amatir). |
| Offer | Game ayunan satu jempol, offline, tanpa iklan paksa. |
| Pemain | Kasual mobile, sesi 1–3 menit, Indonesia + global. |
| Differentiator | Satu jempol, tapi **tiap ayunan terasa beda** dan dunia ikut merespons. |
| Tension | **WOW (spectacle) × KETAGIHAN (mastery)**. |
| Scoping consequence | Beri bobot lebih pada *distinctiveness* & *craft*, bukan sekadar overlap mekanik. |

> Catatan: kata "chat" di permintaan Nodal = sapaan, bukan UI chat. Jadi yang dirembak = **design visual + lore + gameplay**.

---

## 2. Temuan riset yang paling mengubah keputusan

**Kompetitor (18 game discoping, 10 diprofiled):**
- Mekanik `tap–hold–release` sudah dipakai 150 jt+ unduhan (Stickman Hook). **Kontrol bukan pembeda — "rasa" yang pembeda.**
- Kebanyakan DIRECT itu capability-led + session-based + konvensional. Celah kosong: **brand-led + progresi + contrarian + aesthetic-led** (dihuni Hook Heroes, Grapplo, Hang Line, Fancade).
- Review game sejenis paling sering mengeluh: **iklan berlebih**, level terasa sama, energy gate. → positioning "no forced ads, reward opsional" itu senjata, bukan kerugian.
- Roper (Fancade): 15,7 MB, offline, review minta "level lagi". Hang Line: low-poly textureless + hard shadow + GDC talk soal variasi dari satu mekanik.

**Loop ketagihan:**
- Restart < 0,5 dtk adalah **mekanik retensi**, bukan kenyamanan. Delay 3 dtk saja menurunkan jumlah attempt per sesi.
- Near-miss mengaktifkan sirkuit reward yang sama dengan menang. Jadikan momen ("SO CLOSE!"), bukan sekadar mati.
- Hit-stop 60–90 ms di *catch* & *perfect release* = "berat" termurah.
- Juice mengubah tampilan/suara, **bukan aturan**. "Juice is icing, good controls are the cake."
- Meta-progresi ringan (benang/skin) bikin **tidak ada run yang sia-sia** — tanpa energy meter.
- Skill ceiling lewat **perfect release + style score**, bukan timing makin ketat (kedalaman ≠ kesulitan).
- Audio pitch naik per chain (+16 Hz, cap 24, detune ±4%) bikin streak terasa sebelum dihitung.

**Seni low-poly:**
- Fancade: 11 hue × 3 nilai (light/mid/dark) = 33 warna, flat, tanpa PBR. "Wow" dari kejelasan bentuk + warna, bukan efek.
- Monument Valley: buang 70–90% ide, kamera isometrik terkunci, ~3 warna per level. *Limitasi = identitas.*
- Resep ringan: 1 directional + ambient, warna per-face/vertex, bayang ellipse + Bayer 4×4, haze berbasis Z, parallax 3–5 layer, foreground silhouette, jitter seeded, squash-stretch, trail segitiga, dolly zoom 0,3 dtk di momen sempurna.
- Uji: thumbnail 231 px + grayscale + squint. Kalau hero & objective hilang → palet gagal.

**Lore tanpa text dump:**
- Kunang = jejak timing (menyala tepat sebelum apex saat timing pas; bubar saat meleset).
- Petir datang **dari bawah** — badai di lautan awan di bawah pulau, jangkar menahan sesuatu.
- Jangkar punya bekas tangan: ukiran jari, rantai dipelintir 3×, sambungan diperbaiki kawat beda warna.
- Kredit "by Bagus / GosGilA" jadi **goresan tiga garis di logam**, bukan cuma splash.
- Totem diam yang menoleh saat ayunan sempurna; warna = status hubungan dengan jangkar; satu "nada panggil" dipakai kunang/petir/jangkar.
- Hook 30 dtk: pemain sudah dalam keadaan mengayun, tanpa tutorial. *(Catatan: splash + tombol MULAI tetap dipertahankan karena itu permintaan sebelumnya — kompromi: setelah MULAI, run langsung mulai mid-swing.)*

**Teknis — sudah diputuskan:**
- **Tetap Rust + macroquad 0.4 + `Camera3D` + mesh prosedural.** Depth buffer GPU aktif otomatis saat `set_camera(&Camera3D)`, jadi **tanpa depth sorting manual** untuk opaque geometry.
- Flat shading = warna ditanam di vertex (normal tidak dipakai shader default).
- `z_far` 200–500 (bukan default 10000) supaya tidak z-fighting.
- MSAA tetap mati di Android; render target 3D wajib `depth: true`.
- **CI GitHub Actions tidak berubah** (fork cargo-quad-apk, Rust 1.95, NDK r28), preview wasm tetap jalan.
- Bevy **ditolak**: `cargo-quad-apk` tidak kompatibel sejak 0.15 → rewrite CI penuh (cargo-ndk + Gradle + JDK 21) + APK jauh lebih besar. Proyeksi manual + `draw_triangle` juga kalah (depth mati, sorting manual).

---

## 3. Tiga arah yang bisa dipilih Nodal

### 🟢 Opsi A — "AYUN: Little World" **(rekomendasi aku)**
Diorama low-poly 3D isometrik yang hangat dan ceria. Fokus: **wow visual + feel yang ketagihan**. Lore halus lewat benda. Progresi skin + benang. Cocok dengan identitas Fancade-like yang sudah ada dan CI yang sudah jalan.
**Risiko:** paling rendah. **Daya tarik:** paling luas.

### 🟡 Opsi B — "AYUN: Storm Below"
Versi lebih dramatis: langit cerah tapi badai dari bawah, warna sebagai emosi, jangkar menahan sesuatu, petir menyala putih menampakkan ukiran. Fokus: **atmosfer + misteri**.
**Risiko:** nada lebih gelap, bisa menjauh dari target kasual. **Daya tarik:** identitas kuat & beda.

### 🔵 Opsi C — "AYUN: Flow Run"
Fokus ke **skill ceiling**: perfect release ±40 ms, style score, chain 3/5/8/12, daily ghost challenge, leaderboard ±5 peringkat. Visual pendukung saja.
**Risiko:** bisa terasa "kaku" buat pemain kasual. **Daya tarik:** retensi keras buat pemain ribet.

> Bisa juga: **A + elemen C** (A sebagai world, C sebagai sistem) — ini yang paling lengkap, tapi paling lama.

---

## 4. Kalau di-approve, urutan kerjanya

| Phase | Isi | Hasil yang bisa dicek Nodal |
|---|---|---|
| P0 | Fondasi 3D: `Camera3D`, mesh kubus/bola/pulau, palet 3-nilai, flat shading, haze, depth | Preview wasm: dunia 3D low-poly, masih pakai kontrol lama |
| P1 | Feel: hit-stop, trauma shake, squash-stretch, trail segitiga, audio pitch chain, near-miss | APK: tiap tangkap & lepas terasa "nendang" |
| P2 | Loop: restart < 0,5 dtk, benang + skin, daily ghost, perfect release & style score | APK: "satu lagi" kerasa, kalah pun dapat benang |
| P3 | Lore: kunang timing, petir dari bawah, ukiran jangkar, totem, tanda tangan by Bagus | APK: dunia terasa punya cerita tanpa teks |
| P4 | Brand & polish: splash kinetik dipertahankan, copy store, thumbnail test, reduced-motion | APK final |

**Aturan keras yang tetap:**
- Build APK **hanya via GitHub Actions**, tidak pernah compile/link di HP.
- **Nodal yang verifikasi feel & gameplay** — aku tidak mengklaim sudah menguji.
- Tidak publish / kontak / aksi eksternal tanpa izin.
- Screenshot QA dibersihkan di akhir tugas.

---

## 5. Yang aku butuh dari Nodal sekarang

1. Pilih arah: **A / B / C / A+C**.
2. Setuju stack: **tetap macroquad + Camera3D** (atau kamu mau memaksa Bevy dengan konsekuensi CI rewrite).
3. Setuju scope fase pertama: **P0+P1 dulu**, atau langsung **sampai P4**.
4. Setuju positioning **"no forced ads"** sebagai brand stance.

Setelah itu baru aku ubah kode. 🔒
