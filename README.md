# AYUN

Game ayunan satu jempol. **Rust + macroquad** → Android (APK) & WebAssembly.

> **TAP** = lompat (×2 = double jump)
> **TAHAN** = tangkap jangkar & ayun
> **LEPAS** = terlempar / perfect release
> **CHAIN** = ekspresi mastery · **BENANG** = skin permanen · **PETIR** = dari bawah

by **Bagus / GosGilA**

---

## Tampilan

**A Little World** — diorama **3D low-poly** asli (`Camera3D` + depth buffer),
bukan 2.5D. Semua bentuk (kubus, bola, kerucut, cakram) dibangun dari kode,
tanpa aset FBX/GLB. Satu arah cahaya, tiga nilai warna per hue (aturan
Fancade), flat shading, haze berbasis kedalaman, foreground siluet.

Detail rencana & riset: `00-ROMBAK-PLAN.md`.

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
src/main.rs                 logika game, state, fisika, UI 2D
src/world3d.rs              fondasi renderer low-poly (mesh + palet + helper)
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
