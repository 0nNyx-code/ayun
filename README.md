# AYUN

Game ayunan satu jempol. **Rust + macroquad** → Android (APK) & WebAssembly.

> **TAHAN** = menangkap titik ayun terdekat di depan
> **LEPAS** = terlempar, kecepatan dipertahankan
> **KUNANG** = skor + kombo · **PETIR** = −1 nyawa · **JATUH** = tamat

by **Bagus / GosGilA**

---

## Tampilan

Diorama 2.5D: tiap hue punya 3 anak tangga (light / mid / dark) dan palet itu
sendiri yang jadi *lighting model*. Satu arah cahaya dari kiri-atas, bayangan
kontak lembut, bentuk diekstrusi, langit berlapis parallax.

## Build

### APK Android (CI)

Push ke `main` → GitHub Actions membangun APK bertanda tangan
(`aarch64` + `x86_64`), memverifikasi `apksigner` / `zipalign -P 16`, lalu
menerbitkannya ke release **`apk-latest`**.

Butuh: Rust 1.95.0, NDK r28, build-tools 35, `cargo-quad-apk` (fork
[`hoklims/cargo-quad-apk`](https://github.com/hoklims/cargo-quad-apk)).

### WebAssembly

```bash
bash build.sh          # butuh toolchain 1.95.0 + target wasm32-unknown-unknown
python3 serve.py       # http://localhost:8099
```

`/mnt/sdcard` tidak bisa eksekusi file, jadi `CARGO_TARGET_DIR` diarahkan ke
`/tmp`.

## Struktur

```
src/main.rs                 seluruh game (~2.8k baris, tanpa dependency selain macroquad)
assets/                     font + 7 SFX (di-embed lewat include_bytes!)
android-res/                ikon adaptif + warna
.github/workflows/android.yml
tools/make_icon.py          generator ikon
web/                        shell PWA
```

## Catatan teknis

- Fisika fixed timestep **1/120** supaya stabil di HP lambat.
- Jangan pakai `render_target_msaa` di Android (context GLES2 → crash).
- `sample_count` 1 di Android, 4 di desktop.
- Kamera layar wajib `zoom.y` **positif** (macroquad membalik sumbu Y).
