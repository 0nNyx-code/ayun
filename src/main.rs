// AYUN — game ayunan satu jempol.
// Rust + macroquad -> WebAssembly. Tanpa dependency selain macroquad.
//
// Aturan main:
//   TAHAN  = menangkap titik ayun terdekat di depan
//   LEPAS  = terlempar, kecepatan dipertahankan
//   KUNANG = skor + kombo, PETIR = -1 nyawa, JATUH = tamat
//
// Semua fisika memakai fixed timestep 1/120 supaya stabil di HP lambat.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

// ---------------------------------------------------------------- palet ----
// Aturan Fancade: tiap hue punya 3 anak tangga (light / mid / dark) dan
// palet itu sendiri yang jadi lighting model. Satu arah cahaya, kiri-atas.
const PAPER: Color = Color::new(1.000, 0.957, 0.898, 1.0); // #FFF4E5
const PAPER_2: Color = Color::new(0.996, 0.902, 0.804, 1.0); // #FEE6CD
const PAPER_3: Color = Color::new(0.984, 0.835, 0.694, 1.0); // #FBD5B1
const INK: Color = Color::new(0.165, 0.133, 0.200, 1.0); // #2A2233
const INK_SOFT: Color = Color::new(0.420, 0.361, 0.451, 1.0); // #6B5C73
const VERMILION: Color = Color::new(1.000, 0.420, 0.290, 1.0); // #FF6B4A
const TEAL: Color = Color::new(0.184, 0.710, 0.604, 1.0); // #2FB59A
const MUSTARD: Color = Color::new(1.000, 0.761, 0.302, 1.0); // #FFC24D
const PLUM: Color = Color::new(0.357, 0.290, 0.420, 1.0); // #5B4A6B
// Hasil tumpang tindih teal + mustard (overprint), sengaja opaque.
const OVERPRINT: Color = Color::new(0.376, 0.584, 0.345, 1.0);

// ---- langit & lanskap (diorama) -------------------------------------------
const SKY_TOP: Color = Color::new(1.000, 0.827, 0.663, 1.0); // #FFD3A9
const SKY_MID: Color = Color::new(1.000, 0.902, 0.796, 1.0); // #FFE6CB
const SKY_BOT: Color = Color::new(1.000, 0.965, 0.910, 1.0); // #FFF6E8
const HILL_FAR: Color = Color::new(0.776, 0.906, 0.847, 1.0); // #C6E7D8
const HILL_MID: Color = Color::new(0.494, 0.812, 0.702, 1.0); // #7ECFB3
const HILL_NEAR: Color = Color::new(0.278, 0.671, 0.580, 1.0); // #47AB94
const CLOUD: Color = Color::new(1.000, 0.992, 0.976, 1.0);
const CLOUD_SHADE: Color = Color::new(0.965, 0.867, 0.784, 1.0);
const BODY: Color = Color::new(0.290, 0.243, 0.361, 1.0); // #4A3E5C

const GRAVITY: f32 = 2150.0;
const STEP: f32 = 1.0 / 120.0;
const MAX_SPEED: f32 = 1560.0;
const REACH: f32 = 430.0;

// ------------------------------------------------------------- utilitas ----
fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn with_a(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// Alpha asli dikali `k` — dipakai buat fade bertumpuk.
#[allow(dead_code)]
fn fade(c: Color, k: f32) -> Color {
    Color::new(c.r, c.g, c.b, (c.a * k).clamp(0.0, 1.0))
}

// ------------------------------------------------------------- 2.5D look ----
// Bayangan lembut: satu tekstur radial 64x64 dipakai ulang untuk semua objek.
// Ini trik termurah & paling meyakinkan untuk "ada bayangan" di HP.
fn make_shadow_tex() -> Texture2D {
    let n = 64u32;
    let mut data = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let fx = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let fy = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let d = (fx * fx + fy * fy).sqrt().min(1.0);
            let a = (1.0 - d).powf(1.55);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    let t = Texture2D::from_rgba8(n as u16, n as u16, &data);
    t.set_filter(FilterMode::Linear);
    t
}

fn shadow_at(tex: &Texture2D, x: f32, y: f32, rx: f32, ry: f32, a: f32) {
    draw_texture_ex(
        tex,
        x - rx,
        y - ry,
        with_a(INK, a),
        DrawTextureParams {
            dest_size: Some(vec2(rx * 2.0, ry * 2.0)),
            ..Default::default()
        },
    );
}

/// Gelap/terang tanpa menggeser hue — aturan Fancade (shadow = multiply).
fn shade(c: Color, k: f32) -> Color {
    Color::new(
        (c.r * k).clamp(0.0, 1.0),
        (c.g * k).clamp(0.0, 1.0),
        (c.b * k).clamp(0.0, 1.0),
        c.a,
    )
}

fn sky_gradient(left: f32, top: f32, vw: f32, vh: f32) {
    let bands = 30;
    let bh = vh / bands as f32 + 1.5;
    for i in 0..bands {
        let t = i as f32 / (bands - 1) as f32;
        let c = if t < 0.55 {
            mix(SKY_TOP, SKY_MID, t / 0.55)
        } else {
            mix(SKY_MID, SKY_BOT, (t - 0.55) / 0.45)
        };
        draw_rectangle(left, top + i as f32 * vh / bands as f32, vw, bh, c);
    }
}

/// Bukit bulat berlapis: 3 pass (dasar gelap, badan, highlight) = volume.
fn hill_layer(
    cam_x: f32,
    vw: f32,
    base_y: f32,
    parallax: f32,
    col: Color,
    seed: u32,
    step: f32,
    amp: f32,
) {
    let ox = cam_x * parallax;
    let i0 = ((ox - vw * 0.62) / step).floor() as i64 - 1;
    let i1 = ((ox + vw * 0.62) / step).ceil() as i64 + 1;

    let mut i = i0;
    while i <= i1 {
        let h = hash01(i as u32 ^ seed.wrapping_mul(2654435761));
        let r = step * (0.62 + 0.55 * h);
        let x = i as f32 * step - ox;
        let y = base_y + (1.0 - h) * amp;
        draw_circle(x, y + r * 0.26, r, shade(col, 0.80));
        i += 1;
    }
    i = i0;
    while i <= i1 {
        let h = hash01(i as u32 ^ seed.wrapping_mul(2654435761));
        let r = step * (0.62 + 0.55 * h);
        let x = i as f32 * step - ox;
        let y = base_y + (1.0 - h) * amp;
        draw_circle(x, y, r * 0.97, col);
        i += 1;
    }
    i = i0;
    while i <= i1 {
        let h = hash01(i as u32 ^ seed.wrapping_mul(2654435761));
        let r = step * (0.62 + 0.55 * h);
        let x = i as f32 * step - ox;
        let y = base_y + (1.0 - h) * amp;
        draw_circle(x - r * 0.20, y - r * 0.22, r * 0.46, mix(col, PAPER, 0.22));
        i += 1;
    }
}

/// Pulau melayang: batu trapesium + rumput 3 nilai + pohon kecil.
/// Ini yang bikin langit terasa punya KEDALAMAN, bukan wallpaper.
fn island_at(x: f32, y: f32, w: f32, h: f32, seed: u32, haze: f32) {
    let hw = w * 0.5;
    let bot = vec2(x + (hash01(seed) - 0.5) * w * 0.16, y + h);
    // Haze atmosferik: makin jauh makin ditarik ke warna langit bawah.
    // Tanpa ini pulau solid di tengah layar kebaca sebagai pijakan, bukan latar.
    let rock = mix(mix(PLUM, PAPER_3, 0.42), SKY_BOT, haze);
    let grass = mix(HILL_MID, SKY_BOT, haze * 0.82);
    let tree = mix(HILL_NEAR, SKY_BOT, haze * 0.70);

    // badan batu
    draw_triangle(vec2(x - hw, y), vec2(x + hw, y), bot, rock);
    // sisi kanan gelap
    draw_triangle(
        vec2(x + hw, y),
        bot,
        vec2(x + hw * 0.12, y + h * 0.62),
        shade(rock, 0.86),
    );
    // lapisan terang kiri
    draw_triangle(
        vec2(x - hw, y),
        vec2(x - hw * 0.12, y + h * 0.42),
        bot,
        mix(rock, PAPER, 0.14),
    );
    // garis retak tipis
    draw_line(
        x - hw * 0.30,
        y + h * 0.18,
        x + hw * 0.05,
        y + h * 0.55,
        1.4,
        with_a(INK, 0.08 * (1.0 - haze)),
    );

    // rumput: 3 nilai (gelap, badan, kilau)
    draw_rectangle(x - hw - 4.0, y - 8.0, w + 8.0, 14.0, shade(grass, 0.86));
    draw_rectangle(x - hw - 4.0, y - 13.0, w + 8.0, 10.0, grass);
    draw_rectangle(
        x - hw - 4.0,
        y - 13.0,
        w + 8.0,
        3.5,
        mix(grass, PAPER, 0.30),
    );

    // pohon kecil
    let tx = x + (hash01(seed ^ 7) - 0.5) * w * 0.52;
    draw_rectangle(tx - 2.0, y - 28.0, 4.0, 16.0, shade(mix(PLUM, SKY_BOT, haze), 0.88));
    draw_circle(tx + 1.0, y - 33.0, 11.0, shade(tree, 0.88));
    draw_circle(tx, y - 34.0, 9.5, tree);
    draw_circle(tx - 3.5, y - 37.5, 4.2, mix(tree, PAPER, 0.24));
}

/// Awan: 3-5 gumpalan, versi gelap digambar dulu sebagai alas bawah.
fn cloud_at(x: f32, y: f32, s: f32, alpha: f32) {
    let blobs = [
        (-1.35f32, 0.18f32, 0.62f32),
        (-0.55, -0.10, 0.86),
        (0.35, -0.02, 0.74),
        (1.20, 0.22, 0.54),
    ];
    for b in blobs.iter() {
        draw_circle(
            x + b.0 * s,
            y + b.1 * s + s * 0.30,
            b.2 * s,
            with_a(CLOUD_SHADE, alpha),
        );
    }
    for b in blobs.iter() {
        draw_circle(x + b.0 * s, y + b.1 * s, b.2 * s, with_a(CLOUD, alpha));
    }
    for b in blobs.iter() {
        draw_circle(
            x + b.0 * s - b.2 * s * 0.22,
            y + b.1 * s - b.2 * s * 0.26,
            b.2 * s * 0.42,
            with_a(PAPER, alpha * 0.55),
        );
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn smooth(cur: f32, target: f32, rate: f32, dt: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-rate * dt).exp())
}

fn smooth_v(cur: Vec2, target: Vec2, rate: f32, dt: f32) -> Vec2 {
    cur + (target - cur) * (1.0 - (-rate * dt).exp())
}

/// Getaran kamera: jumlah tiga sinus, bukan random tiap frame.
/// Random per frame terbaca sebagai jitter; noise terbaca sebagai hentakan.
fn shake_noise(x: f32) -> f32 {
    x.sin() * 0.55 + (x * 2.13 + 1.7).sin() * 0.30 + (x * 4.31 + 3.1).sin() * 0.15
}

// ---------------------------------------------------------------- haptic ----
// Di web, index.html menambahkan env.ayun_vibrate ke importObject.
// Di desktop fungsi ini jadi no-op supaya build tetap jalan.
#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn ayun_vibrate(ms: i32);
}

fn haptic(ms: i32) {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        ayun_vibrate(ms)
    };
    #[cfg(not(target_arch = "wasm32"))]
    let _ = ms;
}

// ---------------------------------------------------------------- random ----
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as u32
    }
    fn f(&mut self) -> f32 {
        self.next() as f32 / u32::MAX as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

// ------------------------------------------------------------------ pegas ----
// Pegas kritis-teredam untuk animasi UI. Bukan animasi "sekali jalan".
#[derive(Clone, Copy)]
struct Spring {
    value: f32,
    vel: f32,
    target: f32,
    k: f32,
    d: f32,
}

impl Spring {
    #[allow(dead_code)]
    fn new(v: f32) -> Self {
        Spring { value: v, vel: 0.0, target: v, k: 260.0, d: 24.0 }
    }
    fn with_stiffness(v: f32, k: f32, d: f32) -> Self {
        Spring { value: v, vel: 0.0, target: v, k, d }
    }
    fn update(&mut self, dt: f32) {
        let dt = dt.min(1.0 / 30.0);
        let f = (self.target - self.value) * self.k - self.vel * self.d;
        self.vel += f * dt;
        self.value += self.vel * dt;
    }
    fn bump(&mut self, v: f32) {
        self.vel += v;
    }
    fn set(&mut self, v: f32) {
        self.value = v;
        self.target = v;
        self.vel = 0.0;
    }
}

// -------------------------------------------------------------------- UI ----
struct View {
    vw: f32,
    vh: f32,
    scale: f32,
}

impl View {
    fn compute() -> Self {
        let sw = screen_width().max(1.0);
        let sh = screen_height().max(1.0);
        let short = 540.0;
        let (vw, vh) = if sw <= sh {
            (short, short * sh / sw)
        } else {
            (short * sw / sh, short)
        };
        View { vw, vh, scale: sw / vw }
    }
    /// Kamera layar (y tumbuh ke bawah).
    ///
    /// Catatan penting: `Camera2D::matrix()` macroquad mengalikan `zoom.y` dengan
    /// `invert_y = -1` bila `render_target` kosong. Jadi untuk menggambar ke layar
    /// dengan origin kiri-atas, `zoom.y` harus POSITIF — bukan negatif seperti
    /// `from_display_rect` (helper itu ditujukan untuk render target).
    fn ui_cam(&self) -> Camera2D {
        Camera2D {
            rotation: 0.0,
            zoom: vec2(2.0 / self.vw, 2.0 / self.vh),
            target: vec2(self.vw * 0.5, self.vh * 0.5),
            offset: Vec2::ZERO,
            render_target: None,
            viewport: None,
        }
    }
}

fn draw_round_rect(x: f32, y: f32, w: f32, h: f32, r: f32, c: Color) {
    let r = r.min(w * 0.5).min(h * 0.5);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    draw_rectangle(x + r, y, w - 2.0 * r, h, c);
    draw_rectangle(x, y + r, w, h - 2.0 * r, c);
    draw_circle(x + r, y + r, r, c);
    draw_circle(x + w - r, y + r, r, c);
    draw_circle(x + r, y + h - r, r, c);
    draw_circle(x + w - r, y + h - r, r, c);
}

fn draw_round_rect_border(x: f32, y: f32, w: f32, h: f32, r: f32, t: f32, c: Color, fill: Color) {
    draw_round_rect(x - t, y - t, w + 2.0 * t, h + 2.0 * t, r + t, c);
    draw_round_rect(x, y, w, h, r, fill);
}

fn text_w(t: &str, font: &Option<Font>, size: f32) -> f32 {
    measure_text(t, font.as_ref(), size as u16, 1.0).width
}

fn draw_txt(t: &str, x: f32, y: f32, size: f32, color: Color, font: &Option<Font>) {
    draw_text_ex(
        t,
        x,
        y,
        TextParams {
            font: font.as_ref(),
            font_size: size as u16,
            font_scale: 1.0,
            color,
            ..Default::default()
        },
    );
}

fn draw_txt_c(t: &str, x: f32, y: f32, size: f32, color: Color, font: &Option<Font>) {
    let m = measure_text(t, font.as_ref(), size as u16, 1.0);
    draw_txt(t, x - m.width * 0.5, y + m.height * 0.34, size, color, font);
}

/// Teks dengan tracking (jarak antar huruf) — dipakai untuk wordmark.
#[allow(dead_code)]
fn draw_tracked_c(t: &str, cx: f32, y: f32, size: f32, tracking: f32, color: Color, font: &Option<Font>) {
    let total: f32 = t.chars().map(|c| text_w(&c.to_string(), font, size) + tracking).sum::<f32>() - tracking;
    let mut x = cx - total * 0.5;
    for c in t.chars() {
        let s = c.to_string();
        draw_txt(&s, x, y, size, color, font);
        x += text_w(&s, font, size) + tracking;
    }
}

fn draw_heart(x: f32, y: f32, s: f32, c: Color) {
    draw_circle(x - s * 0.32, y - s * 0.18, s * 0.38, c);
    draw_circle(x + s * 0.32, y - s * 0.18, s * 0.38, c);
    draw_triangle(
        vec2(x - s * 0.68, y - s * 0.02),
        vec2(x + s * 0.68, y - s * 0.02),
        vec2(x, y + s * 0.78),
        c,
    );
}

/// Hati 3D: ekstrusi gelap di bawah, badan, lalu kilau kiri-atas.
fn draw_heart3d(x: f32, y: f32, s: f32, c: Color, a: f32) {
    if a <= 0.001 {
        return;
    }
    let d = shade(c, 0.60);
    draw_circle(x - s * 0.32 + 1.4, y - s * 0.18 + 2.4, s * 0.38, with_a(d, a));
    draw_circle(x + s * 0.32 + 1.4, y - s * 0.18 + 2.4, s * 0.38, with_a(d, a));
    draw_triangle(
        vec2(x - s * 0.68 + 1.4, y - s * 0.02 + 2.4),
        vec2(x + s * 0.68 + 1.4, y - s * 0.02 + 2.4),
        vec2(x + 1.4, y + s * 0.78 + 2.4),
        with_a(d, a),
    );
    draw_heart(x, y, s, with_a(c, a));
    draw_circle(x - s * 0.36, y - s * 0.24, s * 0.15, with_a(PAPER, 0.62 * a));
}

/// Kartu pil putih: bayangan lembut, stroke tinta, kilau atas.
/// Ini bahasa UI utama — semua HUD duduk di atas pil, bukan teks melayang.
fn pill_card(x: f32, y: f32, w: f32, h: f32, r: f32, a: f32) {
    if a <= 0.001 || w <= 0.0 || h <= 0.0 {
        return;
    }
    draw_round_rect(x + 2.0, y + 5.0, w, h, r, with_a(INK, 0.10 * a));
    draw_round_rect(x + 2.0, y + 2.0, w, h, r, with_a(INK, 0.05 * a));
    draw_round_rect_border(x, y, w, h, r, 2.0, with_a(INK, 0.82 * a), with_a(PAPER, 0.95 * a));
    draw_round_rect(
        x + 4.0,
        y + 2.5,
        (w - 8.0).max(0.0),
        (h * 0.40).max(0.0),
        (r * 0.6).min(h * 0.20),
        with_a(PAPER, 0.55 * a),
    );
}

// ------------------------------------------------------------------ audio ----
struct Sfx {
    whoosh: Option<Sound>,
    pop: Option<Sound>,
    ping: Option<Sound>,
    perfect: Option<Sound>,
    thud: Option<Sound>,
    combo: Option<Sound>,
    ui: Option<Sound>,
}

impl Sfx {
    async fn load() -> Self {
        Sfx {
            whoosh: load_sound_from_bytes(include_bytes!("../assets/whoosh.wav")).await.ok(),
            pop: load_sound_from_bytes(include_bytes!("../assets/pop.wav")).await.ok(),
            ping: load_sound_from_bytes(include_bytes!("../assets/ping.wav")).await.ok(),
            perfect: load_sound_from_bytes(include_bytes!("../assets/perfect.wav")).await.ok(),
            thud: load_sound_from_bytes(include_bytes!("../assets/thud.wav")).await.ok(),
            combo: load_sound_from_bytes(include_bytes!("../assets/combo.wav")).await.ok(),
            ui: load_sound_from_bytes(include_bytes!("../assets/ui.wav")).await.ok(),
        }
    }
    fn play(s: &Option<Sound>, vol: f32) {
        if let Some(s) = s {
            play_sound(s, PlaySoundParams { looped: false, volume: vol });
        }
    }
}

// ------------------------------------------------------------- best score ----
// Diekspor ke JS (web/index.html) supaya bisa disimpan di localStorage.
static mut BEST_BRIDGE: f32 = 0.0;
static mut FRAMES: u32 = 0;

/// Dipakai web shell untuk tahu kapan frame pertama benar-benar sudah tampil.
#[no_mangle]
pub extern "C" fn ayun_frames() -> u32 {
    unsafe { FRAMES }
}

#[no_mangle]
pub extern "C" fn ayun_get_best() -> f32 {
    unsafe { BEST_BRIDGE }
}

#[no_mangle]
pub extern "C" fn ayun_set_best(v: f32) {
    unsafe {
        BEST_BRIDGE = v;
    }
}

// ------------------------------------------------------------------ dunia ----
struct Anchor {
    pos: Vec2,
    r: f32,
    phase: f32,
    fresh: f32,
}

struct Fly {
    pos: Vec2,
    phase: f32,
    golden: bool,
    taken: bool,
    pop: f32,
}

struct Bolt {
    pos: Vec2,
    t: f32,     // waktu sejak muncul
    warn: f32,  // durasi peringatan
    struck: bool,
    life: f32,  // sisa waktu setelah menyambar
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    size: f32,
    color: Color,
    strip: bool,
    grav: f32,
}

struct FloatText {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    max: f32,
    text: String,
    color: Color,
    size: f32,
}

struct Trail {
    pos: Vec2,
    life: f32,
}

/// Gelombang lingkaran sekali pakai: lepas sempurna, petir, mati.
struct Ring {
    pos: Vec2,
    life: f32,
    max: f32,
    r0: f32,
    r1: f32,
    color: Color,
    w: f32,
}

#[derive(PartialEq, Clone, Copy)]
enum State {
    Title,
    Play,
    Dead,
}

struct Player {
    pos: Vec2,
    vel: Vec2,
    r: f32,
    attached: Option<usize>,
    rope: f32,
    attach_t: f32,
    squash: Spring,
    invuln: f32,
}

struct Game {
    rng: Rng,
    state: State,
    player: Player,
    anchors: Vec<Anchor>,
    flies: Vec<Fly>,
    bolts: Vec<Bolt>,
    parts: Vec<Particle>,
    texts: Vec<FloatText>,
    trail: Vec<Trail>,
    cam: Vec2,
    cam_look: Vec2,
    band_y: f32,
    last_anchor_x: f32,
    last_anchor_y: f32,
    void_y: f32,
    score: f32,
    score_shown: f32,
    best: f32,
    distance: f32,
    combo: u32,
    combo_timer: f32,
    combo_max: u32,
    hearts: i32,
    shake: f32,
    slow: f32,
    flash: f32,
    time: f32,
    run_time: f32,
    dead_t: f32,
    death_reason: u8, // 0 jatuh, 1 petir
    new_best: bool,
    bolt_cd: f32,
    tutorial: f32,
    bolt_flash: f32,
    font_d: Option<Font>,
    font_m: Option<Font>,
    sfx: Sfx,
    press: bool,
    release: bool,
    pointer: Vec2,
    pointer_down: bool,
    touch_id: Option<u64>,
    start_btn: Spring,
    again_btn: Spring,
    score_pop: Spring,
    combo_pop: Spring,
    ui_in: Spring,
    grain: Vec<(Vec2, f32, f32)>,
    hit_stop: f32,
    title_t: f32,
    milestone: i32,
    rings: Vec<Ring>,
    bolt_hits: u8,
    last_life: f32,
    grab_bonus: f32,
    /// Tekstur radial 64x64, dipakai ulang untuk SEMUA bayangan kontak.
    shadow_tex: Texture2D,
}

impl Game {
    fn new(font_d: Option<Font>, font_m: Option<Font>, sfx: Sfx, view: &View) -> Self {
        let mut rng = Rng::new(0xA7F3_19C2_5D41_88E1);
        let mut grain = Vec::new();
        for _ in 0..820 {
            grain.push((
                vec2(rng.range(0.0, view.vw), rng.range(0.0, view.vh)),
                rng.range(0.4, 1.6),
                rng.range(0.02, 0.07),
            ));
        }
        let best = unsafe { BEST_BRIDGE };
        let mut g = Game {
            rng,
            state: State::Title,
            player: Player {
                pos: Vec2::ZERO,
                vel: vec2(300.0, 0.0),
                r: 13.0,
                attached: None,
                rope: 0.0,
                attach_t: 0.0,
                squash: Spring::with_stiffness(1.0, 420.0, 22.0),
                invuln: 0.0,
            },
            anchors: Vec::new(),
            flies: Vec::new(),
            bolts: Vec::new(),
            parts: Vec::new(),
            texts: Vec::new(),
            trail: Vec::new(),
            cam: Vec2::ZERO,
            cam_look: Vec2::ZERO,
            band_y: 0.0,
            last_anchor_x: 0.0,
            last_anchor_y: -120.0,
            void_y: 0.0,
            score: 0.0,
            score_shown: 0.0,
            best,
            distance: 0.0,
            combo: 0,
            combo_timer: 0.0,
            combo_max: 0,
            hearts: 3,
            shake: 0.0,
            slow: 0.0,
            flash: 0.0,
            time: 0.0,
            run_time: 0.0,
            dead_t: 0.0,
            death_reason: 0,
            new_best: false,
            bolt_cd: 4.0,
            tutorial: 0.0,
            bolt_flash: 0.0,
            font_d,
            font_m,
            sfx,
            press: false,
            release: false,
            pointer: Vec2::ZERO,
            pointer_down: false,
            touch_id: None,
            start_btn: Spring::with_stiffness(1.0, 380.0, 26.0),
            again_btn: Spring::with_stiffness(1.0, 380.0, 26.0),
            score_pop: Spring::with_stiffness(1.0, 320.0, 20.0),
            combo_pop: Spring::with_stiffness(1.0, 380.0, 18.0),
            ui_in: Spring::with_stiffness(0.0, 120.0, 22.0),
            grain,
            hit_stop: 0.0,
            title_t: 0.0,
            milestone: 0,
            rings: Vec::new(),
            bolt_hits: 0,
            last_life: 0.0,
            grab_bonus: 0.0,
            shadow_tex: make_shadow_tex(),
        };
        g.seed_world(view);
        g
    }

    fn seed_world(&mut self, view: &View) {
        self.anchors.clear();
        self.flies.clear();
        self.last_anchor_x = 0.0;
        self.last_anchor_y = -120.0;
        self.band_y = -60.0;
        // jangkar pertama: sedikit di depan & atas, supaya pemain langsung terayun
        self.anchors.push(Anchor {
            pos: vec2(285.0, -175.0),
            r: 11.0,
            phase: 0.0,
            fresh: 1.0,
        });
        self.last_anchor_x = 285.0;
        self.last_anchor_y = -175.0;
        for _ in 0..14 {
            self.spawn_anchor();
        }
        self.player.pos = vec2(0.0, 0.0);
        self.player.vel = vec2(340.0, 40.0);
        self.player.attached = Some(0);
        self.player.rope = (self.anchors[0].pos - self.player.pos).length();
        self.player.attach_t = 0.0;
        self.cam = self.player.pos + vec2(170.0, -70.0);
        self.cam_look = self.cam;
        self.void_y = self.last_anchor_y + view.vh * 0.66;
    }

    fn reset_run(&mut self, view: &View) {
        let mut rng = Rng::new(
            (get_time() * 1_000_000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15,
        );
        self.rng = Rng::new(rng.next() as u64);
        self.state = State::Play;
        self.parts.clear();
        self.texts.clear();
        self.trail.clear();
        self.bolts.clear();
        self.score = 0.0;
        self.score_shown = 0.0;
        self.distance = 0.0;
        self.combo = 0;
        self.combo_timer = 0.0;
        self.combo_max = 0;
        self.hearts = 3;
        self.shake = 0.0;
        self.slow = 0.0;
        self.flash = 0.0;
        self.run_time = 0.0;
        self.dead_t = 0.0;
        self.new_best = false;
        self.bolt_cd = 4.5;
        self.tutorial = 7.0;
        self.hit_stop = 0.0;
        self.milestone = 0;
        self.rings.clear();
        self.bolt_hits = 0;
        self.last_life = 0.0;
        self.grab_bonus = 0.0;
        self.player.invuln = 1.0;
        self.player.squash.set(1.0);
        self.score_pop.set(1.0);
        self.combo_pop.set(1.0);
        self.seed_world(view);
        self.state = State::Play;
    }

    // ---------------------------------------------------------- pembentukan ----
    fn spawn_anchor(&mut self) {
        let gap = self.rng.range(232.0, 330.0);
        self.band_y = (self.band_y + self.rng.range(-26.0, 26.0)).clamp(-150.0, 130.0);
        let y = self.band_y + self.rng.range(-128.0, 74.0);
        let pos = vec2(self.last_anchor_x + gap, y);
        self.anchors.push(Anchor { pos, r: 11.0, phase: self.rng.range(0.0, 6.28), fresh: 1.0 });

        // kunang-kunang di lengkung antara jangkar lama & baru
        let a = vec2(self.last_anchor_x, self.last_anchor_y);
        let b = pos;
        let n = 3;
        for i in 0..n {
            let t = 0.34 + 0.26 * i as f32;
            let p = a.lerp(b, t) + vec2(0.0, -95.0 * (std::f32::consts::PI * t).sin());
            let golden = self.rng.chance(0.11);
            self.flies.push(Fly { pos: p, phase: self.rng.range(0.0, 6.28), golden, taken: false, pop: 0.0 });
        }

        self.last_anchor_x = pos.x;
        self.last_anchor_y = pos.y;
    }

    // --------------------------------------------------------------- input ----
    fn poll_input(&mut self, view: &View) {
        self.press = false;
        self.release = false;
        let mut pos = self.pointer;

        for t in touches() {
            let p = t.position / view.scale;
            match t.phase {
                TouchPhase::Started => {
                    if self.touch_id.is_none() {
                        self.touch_id = Some(t.id);
                        self.press = true;
                        pos = p;
                    }
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if self.touch_id == Some(t.id) {
                        pos = p;
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    if self.touch_id == Some(t.id) {
                        self.touch_id = None;
                        self.release = true;
                        pos = p;
                    }
                }
            }
        }

        if is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            self.press = true;
            pos = vec2(mx, my) / view.scale;
        }
        if is_mouse_button_released(MouseButton::Left) {
            self.release = true;
        }

        // keyboard untuk desktop
        if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::W) || is_key_pressed(KeyCode::Up) {
            self.press = true;
        }
        if is_key_released(KeyCode::Space) || is_key_released(KeyCode::W) || is_key_released(KeyCode::Up) {
            self.release = true;
        }

        self.pointer = pos;
        self.pointer_down = self.touch_id.is_some() || is_mouse_button_down(MouseButton::Left);
    }

    // ---------------------------------------------------------------- ayun ----
    fn best_anchor(&self) -> Option<usize> {
        let p = self.player.pos;
        let mut best: Option<(usize, f32)> = None;
        for (i, a) in self.anchors.iter().enumerate() {
            let d = a.pos - p;
            if d.x < 40.0 {
                continue;
            }
            let dist = d.length();
            // "Nyawa Terakhir" memberi jangkauan +20% selama 3,5 detik.
            let reach = REACH * (1.0 + self.grab_bonus);
            if dist > reach || dist < 70.0 {
                continue;
            }
            if d.y > 150.0 {
                continue;
            }
            // utamakan yang searah gerak dan berada di atas
            let cost = dist * 0.55 + (d.y + 40.0).abs() * 0.55 - self.player.vel.normalize_or_zero().dot(d.normalize_or_zero()) * 90.0;
            if best.map_or(true, |(_, c)| cost < c) {
                best = Some((i, cost));
            }
        }
        best.map(|(i, _)| i)
    }

    fn try_attach(&mut self) {
        if self.player.attached.is_some() {
            return;
        }
        if let Some(i) = self.best_anchor() {
            let a = &self.anchors[i];
            let d = a.pos - self.player.pos;
            let dist = d.length();
            self.player.attached = Some(i);
            self.player.rope = dist.clamp(80.0, REACH);
            self.player.attach_t = 0.0;
            self.player.squash.bump(-3.2);
            // sedikit dorongan supaya ayunan langsung hidup
            let tangent = vec2(-d.y, d.x).normalize_or_zero();
            if tangent.dot(self.player.vel) < 0.0 {
                self.player.vel += tangent * 40.0;
            } else {
                self.player.vel -= tangent * 40.0;
            }
            self.shake = self.shake.max(3.0);
            haptic(8);
            Sfx::play(&self.sfx.whoosh, 0.55);
            self.rings.push(Ring {
                pos: a.pos,
                life: 0.26,
                max: 0.26,
                r0: 14.0,
                r1: 46.0,
                color: INK,
                w: 2.0,
            });
            for _ in 0..10 {
                let ang = self.rng.range(0.0, 6.28);
                self.parts.push(Particle {
                    pos: a.pos,
                    vel: vec2(ang.cos(), ang.sin()) * self.rng.range(20.0, 70.0),
                    life: 0.3,
                    max: 0.3,
                    size: self.rng.range(1.5, 3.0),
                    color: INK_SOFT,
                    strip: false,
                    grav: 0.0,
                });
            }
        }
    }

    fn release(&mut self) {
        if self.player.attached.is_none() {
            return;
        }
        // tap kilat (tekan+lepas < 60ms) = ambil jangkar, bukan lepas.
        if self.player.attach_t < 0.06 {
            return;
        }
        self.player.attached = None;
        let sp = self.player.vel.length();
        let ang = self.player.vel.y.atan2(self.player.vel.x).abs();
        let held_long = self.player.attach_t > 0.14;
        self.player.squash.bump(4.0);
        if sp > 520.0 && ang < 0.30 && held_long {
            self.perfect_release();
        } else {
            self.hit_stop = 0.035;
            Sfx::play(&self.sfx.pop, 0.35);
        }
    }

    fn perfect_release(&mut self) {
        let mult = self.mult() as f32;
        let gain = 220.0 * mult;
        self.add_score(gain);
        self.combo += 2;
        self.combo_timer = 2.6;
        self.combo_max = self.combo_max.max(self.combo);
        self.combo_pop.bump(7.0);
        self.slow = 0.13;
        self.hit_stop = 0.055;
        self.shake = self.shake.max(7.0);
        self.flash = 0.35;
        haptic(14);
        Sfx::play(&self.sfx.perfect, 0.5);
        self.rings.push(Ring {
            pos: self.player.pos,
            life: 0.34,
            max: 0.34,
            r0: 16.0,
            r1: 96.0,
            color: VERMILION,
            w: 3.0,
        });
        self.texts.push(FloatText {
            pos: self.player.pos + vec2(0.0, -34.0),
            vel: vec2(0.0, -46.0),
            life: 0.9,
            max: 0.9,
            text: "PAS!".to_string(),
            color: VERMILION,
            size: 34.0,
        });
        for _ in 0..24 {
            let ang = self.rng.range(0.0, 6.28);
            let sp = self.rng.range(80.0, 320.0);
            self.parts.push(Particle {
                pos: self.player.pos,
                vel: vec2(ang.cos(), ang.sin()) * sp + self.player.vel * 0.15,
                life: self.rng.range(0.35, 0.75),
                max: 0.75,
                size: self.rng.range(2.0, 4.5),
                color: if self.rng.chance(0.5) { VERMILION } else { MUSTARD },
                strip: true,
                grav: 620.0,
            });
        }
    }

    fn mult(&self) -> u32 {
        1 + (self.combo / 4).min(5)
    }

    /// Warna badge kombo naik kelas seiring pengali.
    fn combo_color(&self) -> Color {
        match self.mult() {
            1 => VERMILION,
            2 => MUSTARD,
            3 => TEAL,
            4 => PLUM,
            _ => VERMILION,
        }
    }

    fn add_score(&mut self, v: f32) {
        self.score += v;
        self.score_pop.bump(2.6);
    }

    // -------------------------------------------------------------- fisika ----
    fn physics(&mut self, dt: f32) {
        self.player.vel.y += GRAVITY * dt;
        // gesekan udara tipis
        self.player.vel *= 1.0 - 0.05 * dt;
        self.player.pos += self.player.vel * dt;

        if let Some(i) = self.player.attached {
            if i < self.anchors.len() {
                let a = self.anchors[i].pos;
                let d = self.player.pos - a;
                let dist = d.length();
                if dist > self.player.rope && dist > 0.001 {
                    let n = d / dist;
                    self.player.pos = a + n * self.player.rope;
                    let vr = self.player.vel.dot(n);
                    if vr > 0.0 {
                        self.player.vel -= n * vr;
                    }
                }
            } else {
                self.player.attached = None;
            }
        }

        let sp = self.player.vel.length();
        if sp > MAX_SPEED {
            self.player.vel = self.player.vel / sp * MAX_SPEED;
        }
        if self.player.pos.y < self.band_y - 520.0 {
            // batas atas dunia: jangan biarkan pemain terbang ke luar angkasa
            self.player.pos.y = self.band_y - 520.0;
            if self.player.vel.y < 0.0 {
                self.player.vel.y *= -0.2;
            }
        }
    }

    // --------------------------------------------------------------- update ----
    fn update(&mut self, dt: f32, view: &View) {
        self.poll_input(view);
        self.time += dt;

        // JS bisa mengisi rekor kapan saja setelah wasm hidup.
        let bridged = unsafe { BEST_BRIDGE };
        if bridged > self.best {
            self.best = bridged;
        }

        // jam layar judul: dipakai untuk animasi masuk yang berurutan.
        if self.state == State::Title {
            self.title_t += dt;
        } else {
            self.title_t = 0.0;
        }

        self.start_btn.update(dt);
        self.again_btn.update(dt);
        self.score_pop.update(dt);
        self.combo_pop.update(dt);
        self.ui_in.update(dt);

        // skor yang tampil selalu mengejar skor asli — termasuk saat layar mati,
        // supaya angka di HUD tidak "beku" beda dengan angka di kartu.
        self.score_shown = smooth(self.score_shown, self.score, 9.0, dt);

        match self.state {
            State::Title => {
                self.start_btn.target = 1.0;
                if self.press {
                    self.ui_in.bump(6.0);
                    Sfx::play(&self.sfx.ui, 0.5);
                    self.start_btn.bump(-5.0);
                    self.reset_run(view);
                }
                self.cam = smooth_v(self.cam, self.cam + vec2(26.0, 0.0), 0.35, dt);
            }
            State::Play => self.update_play(dt, view),
            State::Dead => {
                self.dead_t += dt;
                self.again_btn.target = 1.0;
                if self.dead_t > 0.55 && self.press {
                    self.again_btn.bump(-5.0);
                    Sfx::play(&self.sfx.ui, 0.5);
                    self.reset_run(view);
                }
            }
        }

        self.update_fx(dt);
    }

    fn update_play(&mut self, dt: f32, view: &View) {
        // hit-stop: jeda mikro saat kejadian besar, terasa "nendang"
        if self.hit_stop > 0.0 {
            self.hit_stop -= dt;
            return;
        }

        let mut scale = 1.0;
        if self.slow > 0.0 {
            self.slow -= dt;
            scale = 0.55;
        }
        let dt = dt * scale;

        self.run_time += dt;
        self.tutorial = (self.tutorial - dt).max(0.0);
        self.player.invuln = (self.player.invuln - dt).max(0.0);
        self.player.attach_t += dt;
        self.player.squash.update(dt);

        if self.last_life > 0.0 {
            self.last_life = (self.last_life - dt).max(0.0);
            if self.last_life <= 0.0 {
                self.grab_bonus = 0.0;
            }
        }

        if self.press {
            self.try_attach();
        }
        if self.release {
            self.release();
        }

        // fisika fixed step
        let mut acc = dt;
        let mut guard = 0;
        while acc >= STEP && guard < 10 {
            self.physics(STEP);
            acc -= STEP;
            guard += 1;
        }

        // jarak & skor jarak
        let dx = (self.player.pos.x - self.distance - 0.0).max(0.0);
        if self.player.pos.x > self.distance {
            self.distance = self.player.pos.x;
            self.add_score(dx * 0.06);
        }

        // cap milestone tiap 100 m: hadiah kecil yang terasa resmi
        let stone = ((self.distance / 10.0) as i32) / 100;
        if stone > self.milestone {
            self.milestone = stone;
            self.texts.push(FloatText {
                pos: self.player.pos + vec2(0.0, -54.0),
                vel: vec2(0.0, -20.0),
                life: 1.15,
                max: 1.15,
                text: format!("{} M", stone * 100),
                color: PLUM,
                size: 30.0,
            });
            haptic(8);
            Sfx::play(&self.sfx.combo, 0.32);
        }

        self.update_flies(dt);
        self.update_bolts(dt);
        self.update_spawn(view);

        // kombo luruh
        if self.combo > 0 {
            self.combo_timer -= dt;
            if self.combo_timer <= 0.0 {
                self.combo = 0;
            }
        }

        // kamera
        self.cam_look = self.player.pos + vec2(185.0, -40.0 + self.player.vel.y * 0.055);
        self.cam = smooth_v(self.cam, self.cam_look, 5.2, dt);

        // garis kematian menyesuaikan band
        let target_void = self.last_anchor_y + view.vh * 0.70;
        self.void_y = smooth(self.void_y, target_void, 1.6, dt);

        // jatuh
        if self.player.pos.y > self.void_y {
            self.die(0);
        }
        if self.hearts <= 0 {
            self.die(1);
        }

    }

    fn update_flies(&mut self, dt: f32) {
        let p = self.player.pos;
        let mut picked: Vec<(Vec2, bool)> = Vec::new();
        for f in self.flies.iter_mut() {
            if f.taken {
                f.pop += dt * 4.0;
                continue;
            }
            f.phase += dt * 2.4;
            let d = p - f.pos;
            if d.length() < 64.0 {
                f.taken = true;
                picked.push((f.pos, f.golden));
            }
        }
        for (pos, golden) in picked {
            self.on_fly(pos, golden);
        }
        self.flies.retain(|f| !(f.taken && f.pop > 1.0));
        // buang yang jauh di belakang
        let cut = self.cam.x - 900.0;
        self.flies.retain(|f| f.pos.x > cut);
    }

    fn on_fly(&mut self, pos: Vec2, golden: bool) {
        self.combo += 1;
        self.combo_max = self.combo_max.max(self.combo);
        self.combo_timer = 2.5;
        let mult = self.mult() as f32;
        let base = if golden { 150.0 } else { 40.0 };
        self.add_score(base * mult);
        self.combo_pop.bump(5.0);
        let vol = (0.30 + self.combo.min(8) as f32 * 0.02).min(0.52);
        Sfx::play(&self.sfx.ping, if golden { vol + 0.08 } else { vol });
        let col = if golden { PLUM } else { MUSTARD };
        for _ in 0..if golden { 16 } else { 8 } {
            let ang = self.rng.range(0.0, 6.28);
            let sp = self.rng.range(40.0, 200.0);
            self.parts.push(Particle {
                pos,
                vel: vec2(ang.cos(), ang.sin()) * sp,
                life: self.rng.range(0.25, 0.55),
                max: 0.55,
                size: self.rng.range(1.5, 3.2),
                color: col,
                strip: false,
                grav: 300.0,
            });
        }
        if golden {
            self.texts.push(FloatText {
                pos: pos + vec2(0.0, -20.0),
                vel: vec2(0.0, -40.0),
                life: 0.8,
                max: 0.8,
                text: "+GOLD".to_string(),
                color: PLUM,
                size: 22.0,
            });
        }
        if self.combo % 5 == 0 {
            Sfx::play(&self.sfx.combo, 0.4);
        }

        // cap kombo besar: satu momen yang terasa resmi, bukan sekadar angka.
        if self.combo == 10 || self.combo == 25 || self.combo == 50 {
            let cc = self.combo_color();
            self.hit_stop = 0.09;
            self.shake = self.shake.max(13.0);
            self.flash = 0.18;
            haptic(18);
            Sfx::play(&self.sfx.combo, 0.5);
            self.texts.push(FloatText {
                pos: pos + vec2(0.0, -52.0),
                vel: vec2(0.0, -26.0),
                life: 1.0,
                max: 1.0,
                text: format!("KOMBO {}", self.combo),
                color: cc,
                size: 30.0,
            });
            self.rings.push(Ring {
                pos,
                life: 0.4,
                max: 0.4,
                r0: 20.0,
                r1: 120.0,
                color: cc,
                w: 2.5,
            });
        }
    }

    fn update_bolts(&mut self, dt: f32) {
        self.bolt_cd -= dt;
        if self.distance > 260.0 && self.bolt_cd <= 0.0 {
            self.bolt_cd = self.rng.range(2.8, 4.6);
            let x = self.player.pos.x + self.rng.range(320.0, 760.0);
            let y = self.band_y + self.rng.range(-110.0, 150.0);
            self.bolts.push(Bolt {
                pos: vec2(x, y),
                t: 0.0,
                warn: 1.75,
                struck: false,
                life: 0.0,
            });
        }
        let p = self.player.pos;
        let invuln = self.player.invuln;
        let mut strikes: Vec<(Vec2, bool, bool)> = Vec::new();
        for b in self.bolts.iter_mut() {
            b.t += dt;
            if !b.struck && b.t >= b.warn {
                b.struck = true;
                b.life = 0.45;
                let d = (p - b.pos).length();
                let hit = d < 62.0 && invuln <= 0.0;
                let near = !hit && d < 150.0;
                strikes.push((b.pos, hit, near));
            } else if b.struck {
                b.life -= dt;
            }
        }

        for (pos, hit, near) in strikes {
            self.bolt_flash = 0.22;
            self.shake = self.shake.max(9.0);
            self.rings.push(Ring {
                pos,
                life: 0.45,
                max: 0.45,
                r0: 26.0,
                r1: 150.0,
                color: MUSTARD,
                w: 2.5,
            });
            Sfx::play(&self.sfx.thud, 0.55);
            for _ in 0..20 {
                let ang = self.rng.range(0.0, 6.28);
                let sp = self.rng.range(90.0, 420.0);
                self.parts.push(Particle {
                    pos,
                    vel: vec2(ang.cos(), ang.sin()) * sp,
                    life: self.rng.range(0.3, 0.6),
                    max: 0.6,
                    size: self.rng.range(2.0, 4.0),
                    color: if self.rng.chance(0.6) { INK } else { MUSTARD },
                    strip: true,
                    grav: 500.0,
                });
            }
            if hit {
                haptic(30);
                self.hearts -= 1;
                self.bolt_hits = self.bolt_hits.saturating_add(1);
                self.player.invuln = 1.5;
                self.shake = self.shake.max(15.0);
                self.flash = 0.55;
                self.hit_stop = 0.09;
                self.combo = 0;
                self.texts.push(FloatText {
                    pos: p + vec2(0.0, -44.0),
                    vel: vec2(0.0, -40.0),
                    life: 0.9,
                    max: 0.9,
                    text: "-1 NYAWA".to_string(),
                    color: VERMILION,
                    size: 26.0,
                });
                // comeback: nyawa terakhir membuka jangkauan & warna emas.
                if self.hearts == 1 && self.last_life <= 0.0 {
                    self.last_life = 3.5;
                    self.grab_bonus = 0.20;
                    self.texts.push(FloatText {
                        pos: p + vec2(0.0, -78.0),
                        vel: vec2(0.0, -16.0),
                        life: 1.6,
                        max: 1.6,
                        text: "NYAWA TERAKHIR".to_string(),
                        color: MUSTARD,
                        size: 26.0,
                    });
                    self.rings.push(Ring {
                        pos: p,
                        life: 0.7,
                        max: 0.7,
                        r0: 20.0,
                        r1: 170.0,
                        color: MUSTARD,
                        w: 3.0,
                    });
                    Sfx::play(&self.sfx.combo, 0.45);
                }
            } else if near {
                self.slow = 0.12;
                self.flash = self.flash.max(0.12);
                self.shake = self.shake.max(3.0);
                self.add_score(60.0);
                self.texts.push(FloatText {
                    pos: p + vec2(0.0, -40.0),
                    vel: vec2(0.0, -50.0),
                    life: 0.8,
                    max: 0.8,
                    text: "MELEWAT!".to_string(),
                    color: TEAL,
                    size: 24.0,
                });
            }
        }

        self.bolts.retain(|b| !(b.struck && b.life <= 0.0));
        self.bolts.retain(|b| b.pos.x > self.cam.x - 700.0);
    }

    fn update_spawn(&mut self, view: &View) {
        let ahead = self.cam.x + view.vw * 1.2;
        let mut guard = 0;
        while self.last_anchor_x < ahead && guard < 8 {
            self.spawn_anchor();
            guard += 1;
        }
        let cut = self.cam.x - view.vw * 0.8;
        // jangan buang jangkar yang masih dipakai
        let used = self.player.attached;
        let mut keep: Vec<Anchor> = Vec::with_capacity(self.anchors.len());
        let mut remap = vec![0usize; self.anchors.len()];
        for (i, a) in self.anchors.iter().enumerate() {
            if a.pos.x > cut || used == Some(i) {
                remap[i] = keep.len();
                keep.push(Anchor { pos: a.pos, r: a.r, phase: a.phase, fresh: a.fresh });
            }
        }
        if let Some(u) = used {
            if u < remap.len() && keep.len() < self.anchors.len() {
                self.player.attached = Some(remap[u]);
            }
        }
        self.anchors = keep;
    }

    fn die(&mut self, reason: u8) {
        if self.state != State::Play {
            return;
        }
        self.state = State::Dead;
        self.death_reason = reason;
        self.dead_t = 0.0;
        self.shake = 26.0;
        self.flash = 0.7;
        self.hit_stop = 0.09;
        self.rings.push(Ring {
            pos: self.player.pos,
            life: 0.6,
            max: 0.6,
            r0: 18.0,
            r1: 230.0,
            color: INK,
            w: 3.0,
        });
        self.again_btn.set(1.0);
        haptic(40);
        Sfx::play(&self.sfx.thud, 0.7);
        self.new_best = self.score > self.best && self.score > 1.0;
        if self.score > self.best {
            self.best = self.score;
            unsafe {
                BEST_BRIDGE = self.best;
            }
        }
        for _ in 0..26 {
            let ang = self.rng.range(0.0, 6.28);
            let sp = self.rng.range(60.0, 340.0);
            self.parts.push(Particle {
                pos: self.player.pos,
                vel: vec2(ang.cos(), ang.sin()) * sp,
                life: self.rng.range(0.4, 0.9),
                max: 0.9,
                size: self.rng.range(2.0, 5.0),
                color: if self.rng.chance(0.5) { INK } else { VERMILION },
                strip: true,
                grav: 700.0,
            });
        }
    }

    fn update_fx(&mut self, dt: f32) {
        self.shake *= (-9.0 * dt).exp();
        self.flash = (self.flash - dt * 3.4).max(0.0);
        self.bolt_flash = (self.bolt_flash - dt * 4.0).max(0.0);

        // jejak syal
        if self.state == State::Play {
            self.trail.push(Trail { pos: self.player.pos, life: 0.42 });
            if self.trail.len() > 26 {
                self.trail.remove(0);
            }
        }
        for t in self.trail.iter_mut() {
            t.life -= dt;
        }
        self.trail.retain(|t| t.life > 0.0);

        for r in self.rings.iter_mut() {
            r.life -= dt;
        }
        self.rings.retain(|r| r.life > 0.0);

        for p in self.parts.iter_mut() {
            p.vel.y += p.grav * dt;
            p.pos += p.vel * dt;
            p.vel *= 1.0 - 1.2 * dt;
            p.life -= dt;
        }
        self.parts.retain(|p| p.life > 0.0);
        if self.parts.len() > 420 {
            let cut = self.parts.len() - 420;
            self.parts.drain(0..cut);
        }

        for t in self.texts.iter_mut() {
            t.pos += t.vel * dt;
            t.vel *= 1.0 - 2.2 * dt;
            t.life -= dt;
        }
        self.texts.retain(|t| t.life > 0.0);

        for a in self.anchors.iter_mut() {
            a.fresh = (a.fresh - dt * 2.2).max(0.0);
        }
    }

    // ----------------------------------------------------------------- draw ----
    fn draw(&mut self, view: &View) {
        clear_background(PAPER);

        // kamera dunia dengan getaran
        let s = self.shake;
        let off = if s > 0.01 {
            let nt = self.time * 20.0;
            vec2(shake_noise(nt), shake_noise(nt + 37.0)) * s
        } else {
            Vec2::ZERO
        };
        let cam = self.cam + off;
        // Dolly punch: makin cepat makin sempit sudut pandangnya.
        // Ini yang bikin kecepatan terasa "maju", bukan cuma garis-garis.
        let spd = self.player.vel.length();
        let punch = if self.state == State::Play {
            1.0 + ((spd - 540.0) / 1750.0).clamp(0.0, 0.15)
        } else {
            1.0
        };
        let world_cam = Camera2D {
            rotation: 0.0,
            zoom: vec2(2.0 * punch / view.vw, 2.0 * punch / view.vh),
            target: cam,
            offset: Vec2::ZERO,
            render_target: None,
            viewport: None,
        };
        set_camera(&world_cam);
        self.draw_world(view, cam);

        // UI
        set_camera(&view.ui_cam());
        self.draw_ui(view);

        set_default_camera();
    }

    fn draw_world(&mut self, view: &View, cam: Vec2) {
        let vw = view.vw;
        let vh = view.vh;
        let left = cam.x - vw * 0.5;
        let top = cam.y - vh * 0.5;
        let right = cam.x + vw * 0.5;

        // ================================================================ LANGIT
        sky_gradient(left, top, vw, vh);

        // ---- matahari: halo bertingkat, badan 3 nilai, sorot kiri-atas
        let sun = vec2(left + vw * 0.76, top + vh * 0.15);
        for i in 0..4 {
            let t = i as f32;
            draw_circle(
                sun.x,
                sun.y,
                68.0 + t * 26.0,
                with_a(MUSTARD, 0.055 - t * 0.011),
            );
        }
        draw_circle(sun.x + 2.5, sun.y + 3.5, 62.0, shade(MUSTARD, 0.78));
        draw_circle(sun.x, sun.y, 60.0, MUSTARD);
        draw_circle(sun.x - 13.0, sun.y - 15.0, 31.0, mix(MUSTARD, PAPER, 0.42));
        draw_circle(sun.x - 21.0, sun.y - 23.0, 15.0, mix(MUSTARD, PAPER, 0.72));

        // ---- sinar: 3 segitiga panjang, sangat tipis, ikut berdenyut
        for i in 0..3 {
            let a0 = 2.02 + i as f32 * 0.44 + (self.time * 0.10).sin() * 0.05;
            let spread = 0.075;
            let len = vw * 1.15;
            let p1 = sun + vec2(a0.cos(), a0.sin()) * 44.0;
            let p2 = sun + vec2((a0 - spread).cos(), (a0 - spread).sin()) * len;
            let p3 = sun + vec2((a0 + spread).cos(), (a0 + spread).sin()) * len;
            draw_triangle(p1, p2, p3, with_a(MUSTARD, 0.040));
        }

        // ---- awan: 3 lapis parallax, makin dekat makin besar & pekat
        let clouds: [(f32, i32, f32, f32, f32, f32); 3] = [
            (0.06, 3, 40.0, 5.0, 0.50, 0.11),
            (0.13, 3, 58.0, 8.0, 0.74, 0.21),
            (0.22, 2, 82.0, 12.0, 0.96, 0.32),
        ];
        for (plx, cnt, sc, spd, alpha, yoff) in clouds {
            for i in 0..cnt {
                let fi = i as f32;
                let span = vw + 460.0;
                let drift = cam.x * plx + self.time * spd + fi * span / cnt as f32 * 1.618;
                let x = left - 230.0 + drift.rem_euclid(span);
                let y = top + vh * yoff + (self.time * 0.55 + fi * 2.1).sin() * 7.0;
                cloud_at(x, y, sc, alpha);
            }
        }

        // ---- pulau melayang: kedalaman paling murah yang ada
        let isles: [(f32, f32, f32, f32, f32, u32, f32); 3] = [
            (0.34, 0.28, 74.0, 54.0, 0.10, 11, 0.58),
            (0.62, 0.40, 104.0, 76.0, 0.16, 29, 0.42),
            (0.15, 0.46, 58.0, 42.0, 0.20, 53, 0.32),
        ];
        for (fx, fy, iw, ih, plx, sd, haze) in isles {
            let span = vw + 700.0;
            let drift = cam.x * plx + self.time * (3.0 + plx * 10.0) + fx * span;
            let x = left - 350.0 + drift.rem_euclid(span);
            let y = top + vh * fy + (self.time * 0.7 + sd as f32).sin() * 6.0;
            island_at(x, y, iw, ih, sd, haze);
        }

        // ---- layang-layang: satu objek kecil, langit langsung terasa hidup
        {
            let kx = left + vw * 0.27 + (self.time * 0.35).sin() * 24.0;
            let ky = top + vh * 0.25 + (self.time * 0.9).sin() * 11.0;
            let tilt = (self.time * 0.7).sin() * 0.20;
            let c = tilt.cos();
            let sn = tilt.sin();
            let s = 14.0;
            let rot = |v: Vec2| vec2(kx + v.x * c - v.y * sn, ky + v.x * sn + v.y * c);

            // ekor
            for k in 0..8 {
                let t = k as f32 / 8.0;
                let tp = rot(vec2(
                    -t * 34.0,
                    s * 1.15 + t * 58.0 + (self.time * 3.0 + t * 5.5).sin() * 7.0 * t,
                ));
                draw_circle(tp.x, tp.y, 2.5 * (1.0 - t * 0.5), with_a(VERMILION, 0.55));
            }

            let p0 = rot(vec2(0.0, -s));
            let p1 = rot(vec2(s * 0.72, 0.0));
            let p2 = rot(vec2(0.0, s * 1.15));
            let p3 = rot(vec2(-s * 0.72, 0.0));
            draw_triangle(p0, p1, p3, mix(VERMILION, PAPER, 0.18));
            draw_triangle(p3, p1, p2, shade(VERMILION, 0.74));
            draw_line(p0.x, p0.y, p2.x, p2.y, 1.4, with_a(INK, 0.20));
            draw_line(p1.x, p1.y, p3.x, p3.y, 1.4, with_a(INK, 0.20));
        }

        // ---- balon: naik pelan, 3 nilai + simpul
        for i in 0..3 {
            let fi = i as f32;
            let span = vh + 460.0;
            let rise = (self.time * (13.0 + fi * 4.5) + fi * 173.0).rem_euclid(span);
            let bx = left + vw * (0.13 + fi * 0.30) + (self.time * 0.6 + fi * 1.9).sin() * 15.0;
            let by = top + vh + 70.0 - rise;
            let r = 14.0 + fi * 3.5;
            let col = [VERMILION, TEAL, MUSTARD][i as usize];

            let tx = bx + (self.time * 1.3 + fi * 2.2).sin() * 7.0;
            draw_line(bx, by + r * 1.30, tx, by + r * 2.7, 1.4, with_a(INK, 0.22));

            draw_circle(bx + 1.5, by + 2.5, r, shade(col, 0.76));
            draw_circle(bx, by, r, col);
            draw_circle(bx - r * 0.22, by - r * 0.24, r * 0.50, mix(col, PAPER, 0.38));
            draw_circle(bx - r * 0.36, by - r * 0.40, r * 0.17, with_a(PAPER, 0.85));
            draw_triangle(
                vec2(bx - 3.2, by + r * 0.94),
                vec2(bx + 3.2, by + r * 0.94),
                vec2(bx, by + r * 1.26),
                shade(col, 0.68),
            );
        }

        // ---- angin: garis tipis melaju, bikin udara terasa bergerak
        for i in 0..6 {
            let fi = i as f32;
            let span = vw + 320.0;
            let wx = left - 160.0 + (self.time * (62.0 + fi * 23.0) + fi * 137.0).rem_euclid(span);
            let wy = top + vh * (0.30 + fi * 0.076) + (self.time * 1.1 + fi).sin() * 5.0;
            let len = 26.0 + fi * 9.0;
            let a = 0.05 + 0.05 * (self.time * 2.0 + fi).sin().abs();
            draw_line(wx, wy, wx + len, wy - 3.0, 1.6, with_a(PAPER, a));
        }

        // ================================================================ BUKIT
        // Kamera mengikuti pemain, jadi bukit dipatok relatif kamera (backdrop).
        // Dijepit di atas jurang supaya tanah tidak "naik" menutupi lubang.
        let hz = (cam.y + vh * 0.44).min(self.void_y - vh * 0.12);
        hill_layer(cam.x, vw, hz, 0.10, HILL_FAR, 17, 200.0, 74.0);
        hill_layer(cam.x, vw, hz + 130.0, 0.20, HILL_MID, 41, 222.0, 104.0);
        hill_layer(cam.x, vw, hz + 250.0, 0.34, HILL_NEAR, 73, 250.0, 138.0);

        // =============================================================== JURANG
        // Bukan lapisan transparan: blok pekat yang benar-benar "turun".
        let v0 = self.void_y;
        let bottom = cam.y + vh * 0.5 + 60.0;
        let lip = mix(HILL_NEAR, PLUM, 0.62);
        if bottom > v0 {
            let depth = bottom - v0;
            let bands = 28;
            for i in 0..bands {
                let t = i as f32 / (bands - 1) as f32;
                let c = if t < 0.28 {
                    mix(lip, PLUM, t / 0.28)
                } else {
                    mix(PLUM, shade(PLUM, 0.28), (t - 0.28) / 0.72)
                };
                draw_rectangle(
                    left,
                    v0 + depth * i as f32 / bands as f32,
                    vw,
                    depth / bands as f32 + 2.0,
                    c,
                );
            }
        }
        // bibir jurang: kilau tipis + rumpur bergoyang + garis bahaya
        draw_rectangle(left, v0 - 6.0, vw, 7.0, mix(HILL_NEAR, PAPER, 0.34));
        draw_rectangle(left, v0 + 1.0, vw, 4.0, shade(HILL_NEAR, 0.62));
        let mut gx = left + 5.0;
        while gx < right {
            let wob = (gx * 0.11 + self.time * 0.8).sin();
            draw_line(
                gx,
                v0 - 5.0,
                gx + wob * 2.2,
                v0 - 5.0 - 8.0 - wob * 2.5,
                2.0,
                mix(HILL_NEAR, PAPER, 0.14),
            );
            gx += 11.0;
        }
        let mut hx = left;
        while hx < right {
            draw_line(hx, v0 + 4.0, hx + 12.0, v0 + 4.0, 2.0, with_a(VERMILION, 0.42));
            hx += 24.0;
        }

        // ============================================================== JANGKAR
        let target = self.best_anchor();
        for (i, a) in self.anchors.iter().enumerate() {
            let is_target = Some(i) == target && self.state == State::Play;
            let pulse = 1.0 + 0.06 * (self.time * 2.0 + a.phase).sin();
            let r = a.r * if is_target { 1.12 * pulse } else { 1.0 };

            // bayangan kontak lembut
            shadow_at(
                &self.shadow_tex,
                a.pos.x + 6.0,
                a.pos.y + 9.0,
                r * 2.0,
                r * 1.6,
                0.20,
            );

            if is_target {
                let hr = r * 2.9 * pulse;
                draw_circle(a.pos.x, a.pos.y, hr, with_a(VERMILION, 0.09));
                draw_circle(a.pos.x, a.pos.y, hr * 0.68, with_a(VERMILION, 0.08));
                draw_circle_lines(a.pos.x, a.pos.y, hr * 0.90, 2.5, with_a(VERMILION, 0.52));
            }

            // Cakram KREM + stroke tinta + lubang teal.
            // Versi teal-di-atas-bukit-hijau tadi tenggelam; krem selalu menang
            // kontras di latar apa pun, dan lubang gelap bikin tali terlihat
            // benar-benar masuk ke dalam benda.
            draw_circle(a.pos.x + 2.4, a.pos.y + 3.4, r + 3.6, with_a(INK, 0.20));
            draw_circle(a.pos.x, a.pos.y, r + 3.6, PAPER);
            // bibir bawah: satu nada lebih gelap → terbaca sebagai piringan tebal
            draw_circle(a.pos.x, a.pos.y + 1.8, r + 1.8, mix(PAPER_3, PAPER, 0.25));
            draw_circle(a.pos.x, a.pos.y, r - 2.2, PAPER);
            draw_circle_lines(a.pos.x, a.pos.y, r + 3.2, 2.6, INK);
            // lubang tali
            draw_circle(a.pos.x, a.pos.y, r - 4.0, shade(TEAL, 0.46));
            draw_circle_lines(a.pos.x, a.pos.y, r - 4.0, 1.8, shade(TEAL, 0.26));
            // kilau di dinding lubang (kiri-atas) → lubang terasa punya kedalaman
            draw_circle(
                a.pos.x - r * 0.20,
                a.pos.y - r * 0.24,
                r * 0.26,
                with_a(mix(TEAL, PAPER, 0.55), 0.55),
            );

            // cincin "baru muncul"
            if a.fresh > 0.0 {
                draw_circle_lines(
                    a.pos.x,
                    a.pos.y,
                    (r + 6.0) * (1.0 + (1.0 - a.fresh) * 1.6),
                    2.0,
                    with_a(TEAL, a.fresh * 0.5),
                );
            }
        }

        // ---- garis bantu ke target
        if self.state == State::Play && self.player.attached.is_none() {
            if let Some(i) = target {
                let a = self.anchors[i].pos;
                let p = self.player.pos;
                let n = 12;
                for k in 0..n {
                    let t0 = k as f32 / n as f32;
                    let t1 = t0 + 0.5 / n as f32;
                    let q0 = p.lerp(a, t0);
                    let q1 = p.lerp(a, t1);
                    draw_line(q0.x, q0.y, q1.x, q1.y, 2.2, with_a(INK, 0.16));
                }
            }
        }

        // ---- kunang-kunang
        for f in self.flies.iter() {
            if f.taken && f.pop > 0.9 {
                continue;
            }
            let tw = 0.6 + 0.4 * (self.time * 4.0 + f.phase).sin();
            let r = if f.golden { 6.5 } else { 4.5 } * (1.0 - f.pop * 0.8).max(0.0);
            let col = if f.golden { PLUM } else { MUSTARD };
            draw_circle(f.pos.x, f.pos.y, r * 3.6, with_a(col, 0.06 * tw));
            draw_circle(f.pos.x, f.pos.y, r * 2.0, with_a(col, 0.13 * tw));
            draw_circle(f.pos.x + 1.0, f.pos.y + 1.6, r, shade(col, 0.72));
            draw_circle(f.pos.x, f.pos.y, r, col);
            // rim gelap senada: mustard di atas bukit hijau tadinya lumer jadi
            // satu titik pucat. Rim 1,2 px cukup buat memisahkan siluet.
            draw_circle_lines(f.pos.x, f.pos.y, r, 1.2, with_a(shade(col, 0.38), 0.85));
            draw_circle(f.pos.x - r * 0.32, f.pos.y - r * 0.34, r * 0.36, with_a(PAPER, 0.85));
        }

        // ---- petir
        for b in self.bolts.iter() {
            if !b.struck {
                let p = (b.t / b.warn).clamp(0.0, 1.0);
                let r = 62.0;
                let a = 0.25 + 0.45 * (self.time * 12.0).sin().abs();
                draw_circle(b.pos.x, b.pos.y, r, with_a(VERMILION, 0.06 + 0.10 * p));
                draw_circle_lines(b.pos.x, b.pos.y, r, 2.5, with_a(VERMILION, a));
                draw_circle_lines(
                    b.pos.x,
                    b.pos.y,
                    r * (1.0 - p),
                    2.0,
                    with_a(VERMILION, 0.5 * (1.0 - p)),
                );
                draw_rectangle(
                    b.pos.x - 2.2,
                    b.pos.y - 21.0,
                    4.4,
                    18.0,
                    with_a(VERMILION, 0.85),
                );
                draw_circle(b.pos.x, b.pos.y + 4.0, 2.6, with_a(VERMILION, 0.85));
            } else {
                let a = (b.life / 0.45).clamp(0.0, 1.0);
                let mut y = b.pos.y - 320.0;
                let mut x = b.pos.x;
                let mut i = 0;
                while y < b.pos.y {
                    let nx = b.pos.x + ((i as f32 * 2.7).sin()) * 17.0;
                    draw_line(x + 2.0, y + 2.0, nx + 2.0, y + 42.0, 4.0, with_a(PLUM, a * 0.5));
                    draw_line(x, y, nx, y + 40.0, 3.0, with_a(MUSTARD, a));
                    x = nx;
                    y += 40.0;
                    i += 1;
                }
                draw_circle(b.pos.x, b.pos.y, 76.0 * (1.0 - a), with_a(MUSTARD, a * 0.22));
            }
        }

        // ---- gelombang lingkaran
        for r in self.rings.iter() {
            let k = 1.0 - (r.life / r.max).clamp(0.0, 1.0);
            let rr = r.r0 + (r.r1 - r.r0) * ease_out_cubic(k);
            draw_circle_lines(
                r.pos.x,
                r.pos.y,
                rr,
                r.w * (1.0 - k * 0.55),
                with_a(r.color, 1.0 - k),
            );
        }

        // ---- partikel
        for p in self.parts.iter() {
            let a = (p.life / p.max).clamp(0.0, 1.0);
            if p.strip {
                let d = p.vel.normalize_or_zero() * (p.size * 2.2);
                draw_line(
                    p.pos.x,
                    p.pos.y,
                    p.pos.x - d.x,
                    p.pos.y - d.y,
                    p.size * 0.8,
                    with_a(p.color, a),
                );
            } else {
                draw_circle(
                    p.pos.x,
                    p.pos.y,
                    p.size * a.max(0.2),
                    with_a(p.color, a),
                );
            }
        }

        // ---- pemain (tidak di layar judul)
        if self.state != State::Title {
            self.draw_player();
        }

        // ---- teks melayang
        for t in self.texts.iter() {
            let a = (t.life / t.max).clamp(0.0, 1.0);
            draw_txt_c(&t.text, t.pos.x, t.pos.y + 2.5, t.size, with_a(INK, a * 0.20), &self.font_d);
            draw_txt_c(&t.text, t.pos.x, t.pos.y, t.size, with_a(t.color, a), &self.font_d);
        }
    }

    fn draw_player(&mut self) {
        let tc = if self.last_life > 0.0 { MUSTARD } else { VERMILION };

        // ---- jejak syal: 3 nilai (bayangan, badan, kilau)
        let n = self.trail.len();
        for i in 1..n {
            let a = self.trail[i].pos;
            let b = self.trail[i - 1].pos;
            let t = i as f32 / n as f32;
            let life = (self.trail[i].life / 0.42).clamp(0.0, 1.0);
            let w = 10.0 * t * life + 1.2;
            draw_line(
                a.x + 1.6,
                a.y + 2.6,
                b.x + 1.6,
                b.y + 2.6,
                w,
                with_a(shade(tc, 0.55), 0.26 * t * life),
            );
            draw_line(a.x, a.y, b.x, b.y, w, with_a(tc, 0.58 * t * life));
            draw_line(
                a.x - 0.8,
                a.y - 1.2,
                b.x - 0.8,
                b.y - 1.2,
                w * 0.42,
                with_a(mix(tc, PAPER, 0.45), 0.32 * t * life),
            );
        }

        // ---- tali: bayangan → badan → kilau
        if let Some(i) = self.player.attached {
            if i < self.anchors.len() {
                let a = self.anchors[i].pos;
                let p = self.player.pos;
                let d = p - a;
                let dist = d.length();
                let slack = (self.player.rope - dist).max(0.0);
                let seg = 10;
                let mut pts = [Vec2::ZERO; 11];
                for k in 0..=seg {
                    let t = k as f32 / seg as f32;
                    let mut q = a.lerp(p, t);
                    q.y += (std::f32::consts::PI * t).sin() * slack * 0.28;
                    pts[k] = q;
                }
                for k in 1..=seg {
                    draw_line(
                        pts[k - 1].x + 2.0,
                        pts[k - 1].y + 3.0,
                        pts[k].x + 2.0,
                        pts[k].y + 3.0,
                        2.6,
                        with_a(INK, 0.16),
                    );
                }
                for k in 1..=seg {
                    draw_line(
                        pts[k - 1].x,
                        pts[k - 1].y,
                        pts[k].x,
                        pts[k].y,
                        2.9,
                        shade(BODY, 0.92),
                    );
                }
                for k in 1..=seg {
                    draw_line(
                        pts[k - 1].x - 0.7,
                        pts[k - 1].y - 0.9,
                        pts[k].x - 0.7,
                        pts[k].y - 0.9,
                        1.0,
                        with_a(PAPER, 0.32),
                    );
                }
            }
        }

        let sq = self.player.squash.value.clamp(0.55, 1.7);
        let r = self.player.r * sq;
        let p = self.player.pos;

        // aura emas saat nyawa tinggal satu
        if self.last_life > 0.0 {
            let pulse = 0.5 + 0.5 * (self.time * 6.0).sin();
            draw_circle(p.x, p.y, r + 13.0 + pulse * 5.0, with_a(MUSTARD, 0.09 + 0.06 * pulse));
            draw_circle_lines(
                p.x,
                p.y,
                r + 7.0 + pulse * 3.0,
                2.0,
                with_a(MUSTARD, 0.35 + 0.25 * pulse),
            );
        }

        // ---- indikator jendela "PAS": cincin mengisi saat ayunan siap dilepas
        if self.player.attached.is_some() && self.state == State::Play {
            let sp = self.player.vel.length();
            let ang = self.player.vel.y.atan2(self.player.vel.x).abs();
            let ready = sp > 520.0 && ang < 0.30;
            let charge =
                ((sp - 420.0) / 140.0).clamp(0.0, 1.0) * if ang < 0.50 { 1.0 } else { 0.25 };
            let ring = r + 9.0;
            if ready {
                draw_circle_lines(p.x, p.y, ring + 1.6, 4.6, with_a(VERMILION, 0.28));
                draw_circle_lines(p.x, p.y, ring, 3.0, VERMILION);
                draw_circle(p.x - ring, p.y, 3.0, mix(VERMILION, PAPER, 0.55));
                draw_circle(p.x + ring, p.y, 3.0, shade(VERMILION, 0.68));
            } else {
                draw_circle_lines(p.x, p.y, ring, 2.2, with_a(INK, 0.14 + 0.26 * charge));
                if charge > 0.30 {
                    draw_circle_lines(p.x, p.y, ring, 2.6, with_a(MUSTARD, charge * 0.80));
                }
            }
        }

        // ---- garis kecepatan: hanya saat benar-benar ngebut
        let spd = self.player.vel.length();
        if spd > 640.0 && self.state == State::Play {
            let k = ((spd - 640.0) / 520.0).clamp(0.0, 1.0);
            let dir = self.player.vel.normalize_or_zero();
            let nrm = vec2(-dir.y, dir.x);
            for i in 0..4 {
                let o = (i as f32 - 1.5) * 9.5;
                let back = 28.0 + i as f32 * 11.0;
                let a0 = p - dir * back + nrm * o;
                let a1 = a0 - dir * (20.0 + 26.0 * k);
                draw_line(a0.x, a0.y, a1.x, a1.y, 1.8, with_a(PLUM, 0.10 + 0.20 * k));
            }
        }

        // ---- bayangan kontak lembut (kanan-bawah, sesuai arah cahaya)
        shadow_at(&self.shadow_tex, p.x + 4.0, p.y + 7.0, r * 1.95, r * 1.60, 0.24);

        let blink = self.player.invuln > 0.0 && ((self.time * 16.0) as i32 % 2 == 0);
        let body = if blink { with_a(BODY, 0.45) } else { BODY };

        // ekstrusi → badan → bidang terang → rim light
        draw_circle(p.x + 1.8, p.y + 3.0, r, shade(body, 0.60));
        draw_circle(p.x, p.y, r, body);
        draw_circle(p.x - r * 0.09, p.y - r * 0.11, r * 0.87, mix(body, PAPER, 0.15));
        draw_circle(
            p.x - r * 0.26,
            p.y - r * 0.30,
            r * 0.30,
            with_a(PAPER, 0.50),
        );
        draw_circle(
            p.x - r * 0.34,
            p.y - r * 0.40,
            r * 0.13,
            with_a(PAPER, 0.85),
        );

        // mata: ikut arah gerak, ada soket + kilau
        let dir = self.player.vel.normalize_or_zero();
        let look = if dir.length_squared() > 0.01 {
            dir
        } else {
            vec2(1.0, 0.0)
        };
        let ex = p.x + look.x * r * 0.34;
        let ey = p.y + look.y * r * 0.34;
        draw_circle(ex + 1.0, ey + 1.6, r * 0.34, with_a(INK, 0.16));
        draw_circle(ex, ey, r * 0.30, PAPER);
        draw_circle(ex + look.x * r * 0.10, ey + look.y * r * 0.10, r * 0.16, INK);
        draw_circle(
            ex + look.x * r * 0.06 - r * 0.08,
            ey + look.y * r * 0.06 - r * 0.09,
            r * 0.055,
            PAPER,
        );

        // simpul syal
        let kx = p.x - look.x * r * 0.92;
        let ky = p.y - look.y * r * 0.92;
        draw_circle(kx + 1.2, ky + 2.0, r * 0.33, shade(tc, 0.62));
        draw_circle(kx, ky, r * 0.31, tc);
        draw_circle(
            kx - r * 0.10,
            ky - r * 0.12,
            r * 0.12,
            mix(tc, PAPER, 0.48),
        );
    }

    fn draw_ui(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;

        match self.state {
            State::Title => self.draw_title(view),
            State::Play => self.draw_hud(view),
            State::Dead => {
                self.draw_hud(view);
                self.draw_dead(view);
            }
        }

        // ---- vignette: pita tepi bertingkat, fokus ke tengah
        for i in 0..10 {
            let t = i as f32 / 9.0;
            let a = 0.030 * (1.0 - t);
            let inset = t * 24.0;
            draw_rectangle(0.0, inset, vw, 2.8, with_a(INK, a));
            draw_rectangle(0.0, vh - inset - 2.8, vw, 2.8, with_a(INK, a));
            draw_rectangle(inset, 0.0, 2.8, vh, with_a(INK, a));
            draw_rectangle(vw - inset - 2.8, 0.0, 2.8, vh, with_a(INK, a));
        }

        // grain kertas: di atas semua, seperti tekstur cetak
        for (p, s, a) in self.grain.iter() {
            draw_rectangle(p.x, p.y, *s, *s, with_a(INK, *a));
        }

        // kilatan
        if self.flash > 0.0 {
            draw_rectangle(0.0, 0.0, vw, vh, with_a(VERMILION, self.flash * 0.22));
        }
        if self.bolt_flash > 0.0 {
            draw_rectangle(0.0, 0.0, vw, vh, with_a(MUSTARD, self.bolt_flash * 0.30));
        }
    }

    fn draw_title(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;
        let f = self.font_d.clone();
        let fm = self.font_m.clone();
        let t = self.time;
        let tt = self.title_t;

        // ---- wash: diorama tetap terlihat, tipografi tetap bersih
        draw_rectangle(0.0, 0.0, vw, vh, with_a(PAPER, 0.58));
        // pita kertas lembut di belakang wordmark
        let band_top = vh * 0.08;
        let band_h = vh * 0.34;
        for i in 0..14 {
            let k = i as f32 / 13.0;
            let edge = 1.0 - ((k - 0.5).abs() * 2.0);
            draw_rectangle(
                0.0,
                band_top + k * band_h,
                vw,
                band_h / 13.0 + 2.0,
                with_a(PAPER, 0.34 * edge.max(0.0)),
            );
        }

        // roller mark: satu sapuan tipis saat poster "dicetak"
        let sweep = (tt / 1.1).clamp(0.0, 1.0);
        if sweep < 1.0 {
            draw_rectangle(
                0.0,
                sweep * vh - 2.0,
                vw,
                4.0,
                with_a(INK, 0.045 * (1.0 - sweep)),
            );
        }

        // ---- kepala poster
        draw_rectangle(0.0, 0.0, vw, 7.0, VERMILION);
        draw_rectangle(0.0, 7.0, vw, 1.0, with_a(INK, 0.45));

        // ---- tanda registrasi (khas cetak)
        let rm = ease_out_cubic((tt / 0.55).clamp(0.0, 1.0));
        if rm > 0.01 {
            let inset = 20.0;
            let arm = 9.0;
            let col = with_a(INK, 0.22 * rm);
            let slip = (1.0 - rm) * 6.0;
            let marks = [
                (inset - slip, inset + 14.0 - slip),
                (vw - inset + slip, inset + 14.0 - slip),
                (inset - slip, vh - inset - 4.0 + slip),
                (vw - inset + slip, vh - inset - 4.0 + slip),
            ];
            for (cx, cy) in marks {
                draw_circle_lines(cx, cy, 5.5, 1.1, col);
                draw_line(cx - arm, cy, cx - 2.0, cy, 1.1, col);
                draw_line(cx + 2.0, cy, cx + arm, cy, 1.1, col);
                draw_line(cx, cy - arm, cx, cy - 2.0, 1.1, col);
                draw_line(cx, cy + 2.0, cx, cy + arm, 1.1, col);
            }
        }

        let bob = (t * 1.2).sin() * 4.0;
        let wm_y = vh * 0.215 + bob;

        // ---- wordmark: 3 lapis ekstrusi (gelap → mid → tinta) = pelat 3D
        let letters = ['A', 'Y', 'U', 'N'];
        let size = 100.0;
        let track = 2.0;
        let widths: [f32; 4] = [
            text_w("A", &f, size),
            text_w("Y", &f, size),
            text_w("U", &f, size),
            text_w("N", &f, size),
        ];
        let total = widths.iter().sum::<f32>() + track * 3.0;
        let base_x = vw * 0.5 - total * 0.5;

        for (dy, col) in [(7.5f32, shade(INK, 0.55)), (3.5, PLUM)] {
            let mut x = base_x;
            for i in 0..4 {
                let p = ease_out_cubic(((tt - 0.10 - i as f32 * 0.075) / 0.55).clamp(0.0, 1.0));
                if p > 0.001 {
                    let s = letters[i].to_string();
                    draw_txt(&s, x, wm_y + (1.0 - p) * 54.0 + dy, size, with_a(col, p), &f);
                }
                x += widths[i] + track;
            }
        }
        let mut x = base_x;
        for i in 0..4 {
            let p = ease_out_cubic(((tt - 0.10 - i as f32 * 0.075) / 0.55).clamp(0.0, 1.0));
            if p > 0.001 {
                let s = letters[i].to_string();
                draw_txt(&s, x, wm_y + (1.0 - p) * 54.0, size, with_a(INK, p), &f);
            }
            x += widths[i] + track;
        }

        // ---- garis bawah wordmark: pelat tebal
        let rw = 200.0 * ease_out_cubic(((tt - 0.42) / 0.6).clamp(0.0, 1.0));
        if rw > 1.0 {
            let ry = wm_y + 28.0;
            draw_round_rect(
                vw * 0.5 - rw * 0.5 + 2.0,
                ry + 4.5,
                rw,
                7.0,
                3.5,
                with_a(shade(VERMILION, 0.58), 0.9),
            );
            draw_round_rect(vw * 0.5 - rw * 0.5, ry, rw, 7.0, 3.5, VERMILION);
            draw_round_rect(
                vw * 0.5 - rw * 0.5 + 4.0,
                ry + 1.2,
                (rw - 8.0).max(0.0),
                2.4,
                1.2,
                with_a(mix(VERMILION, PAPER, 0.55), 0.75),
            );
        }

        let tp = ease_out_cubic(((tt - 0.58) / 0.6).clamp(0.0, 1.0));
        draw_txt_c(
            "satu jempol. jangan jatuh.",
            vw * 0.5,
            wm_y + 66.0,
            19.0,
            with_a(INK_SOFT, tp),
            &fm,
        );

        // ---- skor terbaik
        let bp = ease_out_cubic(((tt - 0.74) / 0.6).clamp(0.0, 1.0));
        if self.best > 1.0 && bp > 0.01 {
            let s = format!("TERBAIK  {}", self.best as i32);
            draw_txt_c(&s, vw * 0.5, vh * 0.385, 20.0, with_a(INK, bp), &fm);
        }

        // ---- tombol mulai
        let ep = ease_out_cubic(((tt - 0.86) / 0.5).clamp(0.0, 1.0));
        let bw = 236.0;
        let bh = 78.0;
        let bx = vw * 0.5 - bw * 0.5;
        let by = vh * 0.465;
        let sc = self.start_btn.value.clamp(0.9, 1.06) * (0.94 + 0.06 * ep);
        let rect = Rect::new(bx, by, bw, bh);
        self.draw_button(rect, "MULAI", sc, &f);

        // ---- cara main
        let y0 = vh * 0.615;
        let rows = [
            "TAHAN  =  AYUN",
            "LEPAS  =  TERBANG",
            "KUNANG = SKOR   •   PETIR = NYAWA",
        ];
        for (i, line) in rows.iter().enumerate() {
            let p = ease_out_cubic(((tt - 1.0 - i as f32 * 0.09) / 0.55).clamp(0.0, 1.0));
            if p > 0.001 {
                draw_txt_c(
                    line,
                    vw * 0.5 + 1.0,
                    y0 + i as f32 * 28.0 + 1.5,
                    18.0,
                    with_a(INK, 0.10 * p),
                    &fm,
                );
                draw_txt_c(
                    line,
                    vw * 0.5,
                    y0 + i as f32 * 28.0,
                    18.0,
                    with_a(INK_SOFT, p),
                    &fm,
                );
            }
        }

        // ---- credit: by Bagus / GosGilA — stagger per huruf
        let cp = ease_out_cubic(((tt - 1.30) / 0.9).clamp(0.0, 1.0));
        if cp > 0.001 {
            let cy = vh * 0.775 + (1.0 - cp) * 16.0;
            let lw = 84.0 * ease_out_cubic(((tt - 1.30) / 0.55).clamp(0.0, 1.0));
            draw_rectangle(
                vw * 0.5 - lw * 0.5,
                cy - 24.0,
                lw,
                1.5,
                with_a(INK, 0.30 * cp),
            );

            let by_txt = "by";
            let nm = "Bagus / GosGilA";
            let s_by = 13.0;
            let s_nm = 23.0;
            let tr = 5.0 * (1.0 - cp) + 1.2;
            let w_by = text_w(by_txt, &fm, s_by);
            let w_nm: f32 = nm
                .chars()
                .map(|c| text_w(&c.to_string(), &f, s_nm) + tr)
                .sum::<f32>()
                - tr;
            let gap = 11.0;
            let mut cx = vw * 0.5 - (w_by + gap + w_nm) * 0.5;
            draw_txt(by_txt, cx, cy + 2.0, s_by, with_a(INK_SOFT, 0.9 * cp), &fm);
            cx += w_by + gap;
            for (i, c) in nm.chars().enumerate() {
                let lp = ease_out_cubic(((tt - 1.30 - i as f32 * 0.022) / 0.45).clamp(0.0, 1.0));
                let ch = c.to_string();
                draw_txt(
                    &ch,
                    cx,
                    cy + (1.0 - lp) * 10.0 + 1.4,
                    s_nm,
                    with_a(INK, 0.18 * lp),
                    &f,
                );
                draw_txt(&ch, cx, cy + (1.0 - lp) * 10.0, s_nm, with_a(INK, lp), &f);
                cx += text_w(&ch, &f, s_nm) + tr;
            }
            draw_circle(cx + 5.0, cy - 7.0, 3.2, with_a(VERMILION, cp));
        }

        // ---- ornamen ayunan di sudut kanan atas: 3 nilai + bayangan
        if vh > 760.0 {
            let op = ease_out_cubic(((tt - 0.9) / 0.7).clamp(0.0, 1.0));
            let pivot = vec2(vw - 74.0, 150.0);
            let ang = (t * 1.4).sin() * 0.5;
            let len = 74.0;
            let bob2 = vec2(pivot.x + ang.sin() * len, pivot.y + ang.cos() * len);
            shadow_at(
                &self.shadow_tex,
                bob2.x + 4.0,
                pivot.y + len + 12.0,
                16.0,
                7.0,
                0.16 * op,
            );
            draw_circle_lines(pivot.x, pivot.y, 7.0, 2.4, with_a(INK, 0.42 * op));
            draw_circle(pivot.x, pivot.y, 3.0, with_a(INK, 0.30 * op));
            draw_line(pivot.x + 1.5, pivot.y + 2.0, bob2.x + 1.5, bob2.y + 2.0, 2.0, with_a(INK, 0.14 * op));
            draw_line(pivot.x, pivot.y, bob2.x, bob2.y, 1.8, with_a(INK, 0.34 * op));
            draw_circle(bob2.x + 1.2, bob2.y + 2.0, 9.0, with_a(shade(INK, 0.5), 0.5 * op));
            draw_circle(bob2.x, bob2.y, 9.0, with_a(INK, 0.52 * op));
            draw_circle(bob2.x - 2.4, bob2.y - 2.6, 3.2, with_a(PAPER, 0.45 * op));
        }

        // ---- color bar: 5 chip, satu overprint opaque (khas riso)
        let cbp = ease_out_cubic(((tt - 1.45) / 0.7).clamp(0.0, 1.0));
        if cbp > 0.01 {
            let cbw = 27.0;
            let cby = vh * 0.885;
            let cbx = vw * 0.5 - cbw * 2.5;
            let sw = [VERMILION, TEAL, MUSTARD, PLUM, OVERPRINT];
            for (i, c) in sw.iter().enumerate() {
                let x = cbx + i as f32 * cbw;
                let ch = 10.0;
                let cy2 = cby + (1.0 - cbp) * 8.0;
                draw_round_rect(
                    x + 1.0,
                    cy2 + 3.0,
                    (cbw - 7.0) * cbp,
                    ch,
                    5.0,
                    with_a(shade(*c, 0.6), cbp),
                );
                draw_round_rect(x, cy2, (cbw - 7.0) * cbp, ch, 5.0, with_a(*c, cbp));
                draw_round_rect(
                    x + 2.0,
                    cy2 + 1.6,
                    ((cbw - 7.0) * cbp - 4.0).max(0.0),
                    3.0,
                    1.5,
                    with_a(PAPER, 0.35 * cbp),
                );
            }
        }

        // ---- kaki poster
        let fp = ease_out_cubic(((tt - 1.55) / 0.8).clamp(0.0, 1.0));
        draw_txt_c(
            "RUST  ·  ANDROID  ·  OFFLINE",
            vw * 0.5,
            vh * 0.952,
            12.0,
            with_a(INK_SOFT, 0.55 * fp),
            &fm,
        );
    }

    fn draw_button(&self, r: Rect, label: &str, sc: f32, f: &Option<Font>) {
        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        let w = r.w * sc;
        let h = r.h * sc;
        let x = cx - w * 0.5;
        let y = cy - h * 0.5;
        let rad = h * 0.5;

        // bayangan lembut
        shadow_at(&self.shadow_tex, cx + 2.0, y + h + 10.0, w * 0.62, 14.0, 0.22);
        // alas gelap = ekstrusi
        draw_round_rect(x, y + 6.0, w, h, rad, shade(VERMILION, 0.58));
        // badan
        draw_round_rect_border(x, y, w, h, rad, 2.4, INK, VERMILION);
        // kilau atas
        draw_round_rect(
            x + 6.0,
            y + 4.5,
            w - 12.0,
            h * 0.34,
            rad * 0.55,
            with_a(mix(VERMILION, PAPER, 0.55), 0.55),
        );
        // label
        draw_txt_c(label, cx, cy + 2.0, h * 0.38, with_a(INK, 0.22), f);
        draw_txt_c(label, cx, cy, h * 0.38, INK, f);
        // panah
        let ax = x + w - h * 0.46;
        draw_triangle(
            vec2(ax, cy - h * 0.15),
            vec2(ax, cy + h * 0.15),
            vec2(ax + h * 0.18, cy),
            shade(VERMILION, 0.45),
        );
    }

    fn draw_hud(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;
        let f = self.font_d.clone();
        let fm = self.font_m.clone();

        // masuknya HUD: naik + fade cepat di awal lari
        let ui = ease_out_cubic((self.run_time / 0.40).clamp(0.0, 1.0));
        if ui <= 0.01 {
            return;
        }
        // aman dari notch / status bar Android edge-to-edge
        let safe = (vh * 0.042).clamp(26.0, 58.0);
        let y0 = safe + (1.0 - ui) * 12.0;

        // ---- pil skor (kiri atas)
        let pop = self.score_pop.value.clamp(0.9, 1.25);
        let s = format!("{}", self.score_shown as i32);
        let sw = text_w(&s, &f, 34.0 * pop);
        let pw = (sw + 44.0).max(106.0);
        pill_card(18.0, y0, pw, 70.0, 24.0, ui);
        draw_txt("SKOR", 34.0, y0 + 25.0, 11.0, with_a(INK_SOFT, 0.9 * ui), &fm);
        draw_txt(
            &s,
            34.0,
            y0 + 60.0 + (1.0 - pop) * 7.0,
            34.0 * pop,
            with_a(INK, ui),
            &f,
        );

        // ---- pil jarak (tengah atas)
        let d = format!("{} M", (self.distance / 10.0) as i32);
        let dw = text_w(&d, &fm, 14.0);
        let dpx = vw * 0.5 - (dw + 34.0) * 0.5;
        pill_card(dpx, y0 + 6.0, dw + 34.0, 34.0, 17.0, ui);
        draw_txt_c(&d, vw * 0.5, y0 + 23.0, 14.0, with_a(INK, ui), &fm);

        // ---- nyawa: hati 3D (kanan atas)
        for i in 0..3 {
            let hx = vw - 30.0 - i as f32 * 28.0;
            let alive = (self.hearts as i32) > i as i32;
            if alive {
                draw_heart3d(hx, y0 + 22.0, 19.0, VERMILION, ui);
            } else {
                draw_heart(hx, y0 + 22.0, 19.0, with_a(INK, 0.11 * ui));
            }
        }

        // ---- terbaik
        let b = format!("TERBAIK {}", self.best as i32);
        let bw = text_w(&b, &fm, 12.0);
        draw_txt(&b, vw - 22.0 - bw, y0 + 60.0, 12.0, with_a(INK_SOFT, 0.72 * ui), &fm);

        // ---- kombo
        if self.mult() > 1 {
            let mult = self.mult();
            let cp = self.combo_pop.value.clamp(0.85, 1.3);
            let label = format!("x{}", mult);
            let cc = self.combo_color();
            let lw = text_w(&label, &f, 20.0 * cp) + 32.0;
            let ccx = 18.0 + pw * 0.5;
            let ccy = y0 + 70.0 + 30.0;
            let chh = 32.0 * cp;
            draw_round_rect(
                ccx - lw * 0.5 + 1.5,
                ccy - chh * 0.5 + 4.0,
                lw,
                chh,
                16.0,
                with_a(INK, 0.16 * ui),
            );
            draw_round_rect(
                ccx - lw * 0.5,
                ccy - chh * 0.5 + 3.0,
                lw,
                chh,
                16.0,
                shade(cc, 0.58),
            );
            draw_round_rect_border(
                ccx - lw * 0.5,
                ccy - chh * 0.5,
                lw,
                chh,
                16.0,
                1.8,
                INK,
                cc,
            );
            draw_round_rect(
                ccx - lw * 0.5 + 5.0,
                ccy - chh * 0.5 + 3.0,
                (lw - 10.0).max(0.0),
                chh * 0.32,
                8.0,
                with_a(mix(cc, PAPER, 0.5), 0.45),
            );
            let tc = if mult == 2 { INK } else { PAPER };
            draw_txt_c(&label, ccx, ccy, 20.0 * cp, tc, &f);
            // timer
            let cw = (self.combo_timer / 2.5).clamp(0.0, 1.0);
            draw_round_rect(
                ccx - lw * 0.5,
                ccy + chh * 0.5 + 6.0,
                lw * cw,
                4.0,
                2.0,
                with_a(cc, 0.85 * ui),
            );
        }

        // ---- petunjuk awal
        if self.tutorial > 0.0 {
            let a = (self.tutorial / 1.2).clamp(0.0, 1.0) * 0.95;
            let hint = if self.player.attached.is_some() {
                "LEPAS untuk terbang"
            } else {
                "TAHAN untuk ayun"
            };
            let hw = text_w(hint, &fm, 16.0) + 42.0;
            let hx = vw * 0.5 - hw * 0.5;
            let hy = vh - 104.0;
            pill_card(hx, hy, hw, 44.0, 22.0, a);
            draw_txt_c(hint, vw * 0.5, hy + 22.0, 16.0, with_a(INK, a), &fm);
        }

        // ---- indikator kecepatan (garis tepi)
        let sp = self.player.vel.length();
        let heat = ((sp - 700.0) / 800.0).clamp(0.0, 1.0);
        if heat > 0.0 {
            draw_rectangle(0.0, 0.0, vw, 5.0, with_a(VERMILION, heat * 0.55));
        }

        // ---- vignette berdenyut saat petir sedang mengisi
        let mut charge = 0.0f32;
        for b in self.bolts.iter() {
            if !b.struck {
                charge = charge.max((b.t / b.warn).clamp(0.0, 1.0));
            }
        }
        if charge > 0.05 {
            let a = charge * (0.05 + 0.05 * (self.time * 14.0).sin().abs());
            draw_rectangle(0.0, 0.0, vw, 26.0, with_a(VERMILION, a));
            draw_rectangle(0.0, vh - 26.0, vw, 26.0, with_a(VERMILION, a));
            draw_rectangle(0.0, 0.0, 18.0, vh, with_a(VERMILION, a * 0.8));
            draw_rectangle(vw - 18.0, 0.0, 18.0, vh, with_a(VERMILION, a * 0.8));
        }
    }

    fn draw_dead(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;
        let f = self.font_d.clone();
        let fm = self.font_m.clone();

        let t = (self.dead_t / 0.42).clamp(0.0, 1.0);
        let e = ease_out_cubic(t);

        // scrim kertas
        draw_rectangle(0.0, 0.0, vw, vh, with_a(PAPER, 0.90 * e));

        let cw = 306.0;
        let ch = 430.0;
        let cx = vw * 0.5;
        let cy = vh * 0.46 + (1.0 - e) * 62.0;

        // kartu berlapis: bayangan lembut → alas gelap → badan → kilau
        let x0 = cx - cw * 0.5;
        let y0 = cy - ch * 0.5;
        shadow_at(&self.shadow_tex, cx + 4.0, y0 + ch + 16.0, cw * 0.62, 26.0, 0.26 * e);
        draw_round_rect(x0 + 3.0, y0 + 10.0, cw, ch, 24.0, with_a(shade(PAPER, 0.80), e));
        draw_round_rect_border(x0, y0, cw, ch, 24.0, 2.6, INK, PAPER);
        draw_round_rect(
            x0 + 8.0,
            y0 + 6.0,
            cw - 16.0,
            22.0,
            11.0,
            with_a(PAPER_2, 0.65 * e),
        );
        // pita warna di kepala kartu
        draw_round_rect(x0 + 2.0, y0 + 2.0, cw - 4.0, 46.0, 22.0, with_a(VERMILION, e));
        draw_rectangle(x0 + 2.0, y0 + 26.0, cw - 4.0, 22.0, with_a(VERMILION, e));
        draw_round_rect(
            x0 + 12.0,
            y0 + 7.0,
            cw - 24.0,
            12.0,
            6.0,
            with_a(mix(VERMILION, PAPER, 0.55), 0.5 * e),
        );

        let top = y0;
        let head = if self.death_reason == 0 { "JATUH" } else { "HABIS" };
        draw_txt_c(head, cx, top + 26.0, 30.0, with_a(PAPER, e), &f);

        let sub = if self.death_reason == 0 {
            "jatuh ke jurang".to_string()
        } else if self.bolt_hits > 1 {
            format!("petir x{}", self.bolt_hits)
        } else {
            "petir".to_string()
        };
        draw_txt_c(&sub, cx, top + 78.0, 15.0, INK_SOFT, &fm);

        // skor besar: 3 nilai
        draw_txt_c("SKOR", cx, top + 112.0, 13.0, with_a(INK_SOFT, 0.8), &fm);
        let score_txt = format!("{}", self.score as i32);
        draw_txt_c(&score_txt, cx, top + 160.0, 54.0, with_a(shade(INK, 0.55), e), &f);
        draw_txt_c(&score_txt, cx, top + 156.0, 54.0, with_a(PLUM, e), &f);
        draw_txt_c(&score_txt, cx, top + 152.0, 54.0, with_a(INK, e), &f);

        // rincian
        let combo_txt = if self.combo_max > 1 {
            format!("x{}", self.combo_max)
        } else {
            "—".to_string()
        };
        let rows = [
            ("JARAK", format!("{} m", (self.distance / 10.0) as i32)),
            ("KOMBO", combo_txt),
            ("TERBAIK", format!("{}", self.best as i32)),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = top + 200.0 + i as f32 * 28.0;
            draw_round_rect(x0 + 24.0, y - 15.0, cw - 48.0, 24.0, 12.0, with_a(PAPER_2, 0.55 * e));
            draw_txt(k, cx - 106.0, y, 14.0, with_a(INK_SOFT, 0.9), &fm);
            let vw_ = text_w(v, &fm, 14.0);
            draw_txt(v, cx + 106.0 - vw_, y, 14.0, INK, &fm);
        }

        if self.new_best {
            draw_txt_c("REKOR BARU", cx, top + 288.0, 15.0, VERMILION, &fm);
        } else if self.best > 1.0 {
            let gap = (self.best - self.score).max(0.0) as i32;
            let line = if gap <= 3 {
                format!("SELISIH {}. ANJIR.", gap)
            } else {
                format!("SELISIH {}", gap)
            };
            let col = if gap <= 3 { VERMILION } else { INK_SOFT };
            draw_txt_c(&line, cx, top + 288.0, 15.0, col, &fm);
        }

        // tombol lagi
        let bw = 214.0;
        let bh = 64.0;
        let bx = cx - bw * 0.5;
        let by = top + ch - 108.0;
        let sc = self.again_btn.value.clamp(0.9, 1.06);
        self.draw_button(Rect::new(bx, by, bw, bh), "LAGI", sc, &f);
        draw_txt_c(
            "ketuk di mana saja",
            cx,
            top + ch - 22.0,
            13.0,
            with_a(INK_SOFT, 0.6),
            &fm,
        );
    }
}

fn hash01(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x as f32 / u32::MAX as f32
}

fn window_conf() -> Conf {
    Conf {
        window_title: "AYUN".to_owned(),
        window_width: 540,
        window_height: 960,
        high_dpi: true,
        // MSAA 4x di layar HP 1080x2400 terlalu mahal; high_dpi sudah memberi
        // tepi halus alami. Di desktop tetap 4x biar sama seperti versi web.
        sample_count: if cfg!(target_os = "android") { 1 } else { 4 },
        window_resizable: !cfg!(target_os = "android"),
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font_d = load_ttf_font_from_bytes(include_bytes!("../assets/display.ttf")).ok();
    let font_m = load_ttf_font_from_bytes(include_bytes!("../assets/mono.ttf")).ok();
    let sfx = Sfx::load().await;
    let view = View::compute();
    let mut game = Game::new(font_d, font_m, sfx, &view);

    loop {
        let view = View::compute();
        let dt = get_frame_time().min(1.0 / 15.0);
        game.update(dt, &view);
        game.draw(&view);
        unsafe {
            FRAMES = FRAMES.wrapping_add(1);
        }
        next_frame().await;
    }
}
