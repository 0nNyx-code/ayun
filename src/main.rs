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

mod world3d;
use world3d::*;

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
/// Jangkauan tangkap. Dinaikkan dari 430 → 510 supaya jangkar yang lebih
/// tinggi tetap bisa dicapai; ditambah bonus vertikal di `best_anchor`.
const REACH: f32 = 510.0;
/// Tap = lompat. Di bawah durasi ini, lepas jari dianggap tap, bukan lepas ayun.
const TAP_MAX: f32 = 0.13;
/// Tahan selama ini = tali menangkap. Di bawahnya, jari dianggap tap.
/// 85 ms cukup kecil untuk tidak terasa lambat, cukup besar untuk
/// membedakan "ketuk" dari "tahan" di layar sentuh.
const HOLD_ATTACH: f32 = 0.085;
/// Lompatan dari tanah/udara dan lompatan kedua.
const JUMP_V: f32 = 800.0;
const AIR_JUMP_V: f32 = 690.0;
/// Winch: tali memendek selama pemain benar-benar mati (hampir diam) →
/// pemain tertarik ke atas. Ini yang membunuh rasa "stuck" saat ayunan mati.
/// Gerbangnya sengaja sangat ketat: kalau winch bekerja sambil berayun,
/// panjang tali yang berubah terus-menerus malah memompa energi tanpa henti
/// dan pemain jadi muter lewat atas jangkar.
const REEL: f32 = 70.0;
const REEL_GATE: f32 = 80.0;
const ROPE_MIN: f32 = 150.0;
/// Auto-pump. Hanya bekerja di "zona tenaga": di bawah jangkar DAN sedang
/// bergerak maju. Arah dorongnya selalu ke depan, jadi tidak pernah melawan
/// gerak — dan karena butuh vt > 0, tidak ada posisi setimbang yang bikin
/// pemain menggantung miring seperti patung.
const PUMP_FORCE: f32 = 900.0;
const PUMP_TARGET: f32 = 900.0;
/// Batas kecepatan selama bergantung. Tanpa ini pump terus menambah energi
/// tiap putaran dan pemain bisa lewat di atas jangkar.
const ATTACH_CAP: f32 = 980.0;
/// Dorongan sangat kecil saat benar-benar diam, untuk memecah simetri:
/// tanpa ini, pemain yang menggantung tepat di bawah jangkar tidak punya
/// komponen tangensial sama sekali, jadi gaya sebesar apa pun tidak
/// mengayunkannya.
const NUDGE: f32 = 130.0;
/// Kunci input sesaat setelah pindah layar. Tanpa ini, tap yang dipakai
/// meluncurkan aplikasi langsung menembus menu (keluhan "nggak ada UI start").
const INPUT_LOCK: f32 = 0.42;

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

/// Bukit bulat berlapis: 4 pass (bayangan jatuh, dasar gelap, badan, highlight).
/// Pass bayangan inilah yang membuat lapisan depan terlihat MELAYANG di atas
/// lapisan belakang — depth cue paling murah yang ada.


/// Foreground: lapisan paling dekat (parallax > 1). Digambar PALING AKHIR,
/// jadi ia memotong semua yang ada di belakangnya. Ini depth cue yang bikin
/// diorama terasa punya ruang — bukan gambar datar.


/// Pulau melayang: batu trapesium + rumput 3 nilai + pohon kecil.
/// Ini yang bikin langit terasa punya KEDALAMAN, bukan wallpaper.


/// Awan: 3-5 gumpalan, versi gelap digambar dulu sebagai alas bawah.


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
        Rng(seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407)
            | 1)
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
        Spring {
            value: v,
            vel: 0.0,
            target: v,
            k: 260.0,
            d: 24.0,
        }
    }
    fn with_stiffness(v: f32, k: f32, d: f32) -> Self {
        Spring {
            value: v,
            vel: 0.0,
            target: v,
            k,
            d,
        }
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
        View {
            vw,
            vh,
            scale: sw / vw,
        }
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
fn draw_tracked_c(
    t: &str,
    cx: f32,
    y: f32,
    size: f32,
    tracking: f32,
    color: Color,
    font: &Option<Font>,
) {
    let total: f32 = t
        .chars()
        .map(|c| text_w(&c.to_string(), font, size) + tracking)
        .sum::<f32>()
        - tracking;
    let mut x = cx - total * 0.5;
    for c in t.chars() {
        let s = c.to_string();
        draw_txt(&s, x, y, size, color, font);
        x += text_w(&s, font, size) + tracking;
    }
}

/// Satu glif dengan skala + rotasi, dipusatkan di (cx, cy).
/// Dipakai layar kredit: huruf bisa "menghantam" tanpa blur, karena
/// ukuran raster font tetap dan skalanya lewat font_scale.
fn draw_glyph(
    ch: &str,
    cx: f32,
    cy: f32,
    size: f32,
    scale: f32,
    rot: f32,
    color: Color,
    font: &Option<Font>,
) {
    if color.a <= 0.002 || scale <= 0.02 {
        return;
    }
    let s = scale.clamp(0.05, 4.0);
    let base = (size / s).clamp(8.0, 360.0);
    let m = measure_text(ch, font.as_ref(), base as u16, 1.0);
    let w = m.width * s;
    let h = m.height * s;
    draw_text_ex(
        ch,
        cx - w * 0.5,
        cy + h * 0.34,
        TextParams {
            font: font.as_ref(),
            font_size: base as u16,
            font_scale: s,
            rotation: rot,
            color,
            ..Default::default()
        },
    );
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
    draw_circle(
        x - s * 0.32 + 1.4,
        y - s * 0.18 + 2.4,
        s * 0.38,
        with_a(d, a),
    );
    draw_circle(
        x + s * 0.32 + 1.4,
        y - s * 0.18 + 2.4,
        s * 0.38,
        with_a(d, a),
    );
    draw_triangle(
        vec2(x - s * 0.68 + 1.4, y - s * 0.02 + 2.4),
        vec2(x + s * 0.68 + 1.4, y - s * 0.02 + 2.4),
        vec2(x + 1.4, y + s * 0.78 + 2.4),
        with_a(d, a),
    );
    draw_heart(x, y, s, with_a(c, a));
    draw_circle(
        x - s * 0.36,
        y - s * 0.24,
        s * 0.15,
        with_a(PAPER, 0.62 * a),
    );
}

/// Kartu pil putih: bayangan lembut, stroke tinta, kilau atas.
/// Ini bahasa UI utama — semua HUD duduk di atas pil, bukan teks melayang.
fn pill_card(x: f32, y: f32, w: f32, h: f32, r: f32, a: f32) {
    if a <= 0.001 || w <= 0.0 || h <= 0.0 {
        return;
    }
    draw_round_rect(x + 2.0, y + 5.0, w, h, r, with_a(INK, 0.10 * a));
    draw_round_rect(x + 2.0, y + 2.0, w, h, r, with_a(INK, 0.05 * a));
    draw_round_rect_border(
        x,
        y,
        w,
        h,
        r,
        2.0,
        with_a(INK, 0.82 * a),
        with_a(PAPER, 0.95 * a),
    );
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
            whoosh: load_sound_from_bytes(include_bytes!("../assets/whoosh.wav"))
                .await
                .ok(),
            pop: load_sound_from_bytes(include_bytes!("../assets/pop.wav"))
                .await
                .ok(),
            ping: load_sound_from_bytes(include_bytes!("../assets/ping.wav"))
                .await
                .ok(),
            perfect: load_sound_from_bytes(include_bytes!("../assets/perfect.wav"))
                .await
                .ok(),
            thud: load_sound_from_bytes(include_bytes!("../assets/thud.wav"))
                .await
                .ok(),
            combo: load_sound_from_bytes(include_bytes!("../assets/combo.wav"))
                .await
                .ok(),
            ui: load_sound_from_bytes(include_bytes!("../assets/ui.wav"))
                .await
                .ok(),
        }
    }
    fn play(s: &Option<Sound>, vol: f32) {
        if let Some(s) = s {
            play_sound(
                s,
                PlaySoundParams {
                    looped: false,
                    volume: vol,
                },
            );
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

/// Debug: baca satu nilai state (dipakai untuk verifikasi otomatis di web).
/// 0=px 1=py 2=vx 3=vy 4=rope 5=attached 6=score 7=distance 8=state 9=air_jumps
/// 10=press_t 11=hold_done 12=input_lock 13=hearts 14=splash_t 15=title_t
#[no_mangle]
pub extern "C" fn ayun_debug(i: u32) -> f32 {
    unsafe {
        let g = match DEBUG_GAME {
            Some(p) => &*p,
            None => return -1.0,
        };
        match i {
            0 => g.player.pos.x,
            1 => g.player.pos.y,
            2 => g.player.vel.x,
            3 => g.player.vel.y,
            4 => g.player.rope,
            5 => match g.player.attached {
                Some(k) => k as f32,
                None => -1.0,
            },
            6 => g.score,
            7 => g.distance,
            8 => match g.state {
                State::Splash => 0.0,
                State::Title => 1.0,
                State::Play => 2.0,
                State::Dead => 3.0,
            },
            9 => g.air_jumps as f32,
            10 => g.press_t,
            11 => {
                if g.hold_done {
                    1.0
                } else {
                    0.0
                }
            }
            12 => g.input_lock,
            13 => g.hearts as f32,
            14 => g.splash_t,
            15 => g.title_t,
            16 => g.time,
            17 => JUMP_COUNT as f32,
            18 => g.pump,
            19 => g.chain as f32,
            20 => g.threads as f32,
            21 => g.style,
            22 => g.skin as f32,
            23 => g.near_miss,
            _ => -1.0,
        }
    }
}

static mut DEBUG_GAME: Option<*const Game> = None;
static mut JUMP_COUNT: u32 = 0;

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
    t: f32,    // waktu sejak muncul
    warn: f32, // durasi peringatan
    struck: bool,
    life: f32, // sisa waktu setelah menyambar
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
    /// Layar kredit: nama muncul sebagai kinetic typography, baru masuk menu.
    Splash,
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
    /// Jam layar kredit (State::Splash).
    splash_t: f32,
    /// Sisa waktu kunci input setelah ganti layar.
    input_lock: f32,
    /// Berapa lama jari sudah menahan.
    press_t: f32,
    /// Lepas jari dengan durasi < TAP_MAX = tap (lompat), bukan lepas ayun.
    tap: bool,
    /// Sudah memproses ambang tahan untuk tekanan jari saat ini.
    hold_done: bool,
    /// Sisa lompatan udara (double jump). Diisi ulang setiap menangkap jangkar.
    air_jumps: u8,
    /// Jump buffering: tap yang belum terpakai tetap hidup sebentar.
    jump_buf: f32,
    /// Coyote time setelah lepas tali.
    coyote: f32,
    /// 0..1 — seberapa keras winch sedang menarik, untuk animasi tali.
    pump: f32,
    milestone: i32,
    rings: Vec<Ring>,
    bolt_hits: u8,
    last_life: f32,
    grab_bonus: f32,
    /// Tekstur radial 64x64, dipakai ulang untuk SEMUA bayangan kontak.
    shadow_tex: Texture2D,
    /// Mesh low-poly yang dipakai ulang (dibangun sekali saat startup).
    meshes: MeshBook,
    /// Chain perfect-release. Hilang setelah 1,6 detik idle.
    chain: u32,
    chain_t: f32,
    /// Skor gaya (mastery expression), terpisah dari jarak.
    style: f32,
    /// Mata uang meta-progresi — tiap run tetap dapat.
    threads: u32,
    /// Benang yang didapat run terakhir (dipakai layar mati).
    last_threads: u32,
    /// Skin yang dipakai & sudah dibuka (bitmask).
    skin: u8,
    unlocked: u8,
    /// Seed harian untuk "Ayunan Hari Ini".
    daily_seed: u32,
    /// Timer efek near-miss "SO CLOSE!".
    near_miss: f32,
    /// Jejak run terbaik untuk ghost.
    ghost: Vec<Vec2>,
    ghost_on: bool,
    /// Animasi masuk: pemain sudah dalam ayunan sejak frame pertama.
    intro: f32,
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
            state: State::Splash,
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
            splash_t: 0.0,
            input_lock: 0.0,
            press_t: 0.0,
            tap: false,
            hold_done: false,
            air_jumps: 2,
            jump_buf: 0.0,
            coyote: 0.0,
            pump: 0.0,
            milestone: 0,
            rings: Vec::new(),
            bolt_hits: 0,
            last_life: 0.0,
            grab_bonus: 0.0,
            shadow_tex: make_shadow_tex(),
            meshes: MeshBook::build(),
            chain: 0,
            chain_t: 0.0,
            style: 0.0,
            threads: 0,
            last_threads: 0,
            skin: 0,
            unlocked: 1,
            daily_seed: (get_time() / 86400.0) as u32,
            near_miss: 0.0,
            ghost: Vec::new(),
            ghost_on: false,
            intro: 0.0,
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
        let mut rng = Rng::new((get_time() * 1_000_000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15);
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
        self.air_jumps = 2;
        self.jump_buf = 0.0;
        self.coyote = 0.0;
        self.pump = 0.0;
        self.press_t = 0.0;
        self.tap = false;
        self.hold_done = false;
        self.chain = 0;
        self.chain_t = 0.0;
        self.style = 0.0;
        self.near_miss = 0.0;
        self.intro = 0.0;
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
        self.anchors.push(Anchor {
            pos,
            r: 11.0,
            phase: self.rng.range(0.0, 6.28),
            fresh: 1.0,
        });

        // kunang-kunang di lengkung antara jangkar lama & baru
        let a = vec2(self.last_anchor_x, self.last_anchor_y);
        let b = pos;
        let n = 3;
        for i in 0..n {
            let t = 0.34 + 0.26 * i as f32;
            let p = a.lerp(b, t) + vec2(0.0, -95.0 * (std::f32::consts::PI * t).sin());
            let golden = self.rng.chance(0.11);
            self.flies.push(Fly {
                pos: p,
                phase: self.rng.range(0.0, 6.28),
                golden,
                taken: false,
                pop: 0.0,
            });
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
        if is_key_pressed(KeyCode::Space)
            || is_key_pressed(KeyCode::W)
            || is_key_pressed(KeyCode::Up)
        {
            self.press = true;
        }
        if is_key_released(KeyCode::Space)
            || is_key_released(KeyCode::W)
            || is_key_released(KeyCode::Up)
        {
            self.release = true;
        }

        // ---- klasifikasi tap vs tahan -------------------------------------
        // Tap pendek = LOMPAT. Tahan = ayun (dan lepas = terlempar).
        // Durasi dihitung dari `pointer_down` frame sebelumnya, jadi tetap
        // akurat walau frame-nya panjang di HP lambat.
        self.tap = false;
        if self.press {
            self.press_t = 0.0;
            self.hold_done = false;
        }
        if self.release {
            if self.press_t <= TAP_MAX {
                self.tap = true;
            }
            self.press_t = 0.0;
            self.hold_done = false;
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
            // Sedikit ke belakang masih boleh: jangkar tinggi sering baru
            // "sejajar" setelah pemain melewatinya. Dulu d.x < 40 membuat
            // jangkar itu hilang tepat saat paling dibutuhkan.
            if d.x < -70.0 {
                continue;
            }
            let dist = d.length();
            // "Nyawa Terakhir" memberi jangkauan +20% selama 3,5 detik.
            let reach = REACH * (1.0 + self.grab_bonus);
            // Jangkar di ATAS dapat bonus jangkauan — inilah arah yang
            // paling sering dicari pemain.
            let reach_v = if d.y < 0.0 {
                reach * 1.18
            } else {
                reach * 0.92
            };
            if dist > reach_v || dist < 64.0 {
                continue;
            }
            // Batas "jangan ambil yang jauh di bawah" dilonggarkan 150 → 230.
            if d.y > 230.0 {
                continue;
            }
            // utamakan yang searah gerak dan berada di atas
            let cost = dist * 0.52 + (d.y + 40.0).abs() * 0.52
                - self
                    .player
                    .vel
                    .normalize_or_zero()
                    .dot(d.normalize_or_zero())
                    * 100.0
                + if d.x < 0.0 { -d.x * 0.85 } else { 0.0 };
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
            // Pegang tali = napas baru: dua lompatan udara kembali penuh.
            self.air_jumps = 2;
            self.coyote = 0.0;
            self.jump_buf = 0.0;
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
        // TAP = LOMPAT. Lepas cepat saat menggantung bukan sekadar jatuh,
        // tapi melompat naik — ini yang bikin kontrol terasa hidup.
        if self.tap || self.player.attach_t < TAP_MAX {
            self.hop_off();
            return;
        }
        self.player.attached = None;
        self.jump_buf = 0.0;
        self.coyote = 0.14;
        // Minimal satu lompatan udara tersisa untuk koreksi arah.
        self.air_jumps = self.air_jumps.max(1);
        let sp = self.player.vel.length();
        let ang = self.player.vel.y.atan2(self.player.vel.x).abs();
        let held_long = self.player.attach_t > 0.14;
        self.player.squash.bump(4.0);
        if sp > 520.0 && ang < 0.30 && held_long {
            self.perfect_release();
        } else {
            // Near-miss = hampir sukses. Otak membacanya sebagai menang,
            // bukan kalah — jadikan momen singkat, lalu lanjut.
            let near = sp > 360.0 && ang < 0.50 && held_long;
            if near {
                self.near_miss = 0.55;
                self.slow = self.slow.max(0.10);
                self.hit_stop = 0.07;
                self.shake = self.shake.max(3.4);
                self.style += 24.0;
                haptic(8);
                Sfx::play(&self.sfx.ping, 0.30);
            } else {
                self.hit_stop = 0.045;
                Sfx::play(&self.sfx.pop, 0.35);
            }
        }
    }

    fn perfect_release(&mut self) {
        let mult = self.mult() as f32;
        // Chain = ekspresi mastery. Tidak menggantikan skor jarak.
        self.chain += 1;
        self.chain_t = 1.6;
        let tier = self.chain;
        self.style += 55.0 * mult * (1.0 + (tier.min(8) as f32) * 0.25);
        let gain = 220.0 * mult;
        self.add_score(gain);
        self.combo += 2;
        self.combo_timer = 2.6;
        self.combo_max = self.combo_max.max(self.combo);
        self.combo_pop.bump(7.0);
        self.slow = 0.13;
        self.hit_stop = 0.070;
        // Shake hanya di tier tinggi, supaya tetap terasa langka.
        if tier >= 3 {
            self.shake = self.shake.max(6.0 + tier.min(6) as f32 * 1.6);
        }
        self.flash = (0.25 + tier.min(6) as f32 * 0.035).min(0.48);
        haptic(10 + tier.min(6) as i32 * 2);
        Sfx::play(
            if tier >= 3 { &self.sfx.combo } else { &self.sfx.perfect },
            (0.42 + tier.min(6) as f32 * 0.04).min(0.72),
        );
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
            text: if tier >= 5 {
                format!("FLOW ×{}", tier)
            } else if tier >= 3 {
                "PAS! PAS!".to_string()
            } else {
                "PAS!".to_string()
            },
            color: if tier >= 5 { MUSTARD } else { VERMILION },
            size: 34.0 + tier.min(6) as f32 * 2.0,
        });
        let pcnt = (16 + tier.min(8) as usize * 5).min(56);
        for _ in 0..pcnt {
            let ang = self.rng.range(0.0, 6.28);
            let sp = self.rng.range(80.0, 320.0);
            self.parts.push(Particle {
                pos: self.player.pos,
                vel: vec2(ang.cos(), ang.sin()) * sp + self.player.vel * 0.15,
                life: self.rng.range(0.35, 0.75),
                max: 0.75,
                size: self.rng.range(2.0, 4.5),
                color: if self.rng.chance(0.5) {
                    VERMILION
                } else {
                    MUSTARD
                },
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

        // ---- AUTO-PUMP + WINCH --------------------------------------------
        // Dua hal inilah yang membunuh keluhan "saat diam tali tidak narik
        // ke atas, jadi stuck":
        //   1. winch : tali memendek saat pemain benar-benar mati
        //   2. pump  : dorongan ke depan di zona tenaga, jadi ayunan tidak
        //              pernah kehabisan energi
        if let Some(i) = self.player.attached {
            if i < self.anchors.len() {
                let a = self.anchors[i].pos;
                let d = self.player.pos - a;
                let dist = d.length().max(0.001);
                let n = d / dist;
                let sp = self.player.vel.length();

                // winch: hanya saat hampir diam DAN hampir menggantung tegak.
                // Gerbang `n.y` penting: kalau winch ikut bekerja di ujung
                // ayunan (saat kecepatan sesaat nol), panjang tali menyusut
                // terus dan itu memompa energi tanpa henti.
                if self.player.rope > ROPE_MIN && sp < REEL_GATE && n.y > 0.86 {
                    self.player.rope = (self.player.rope - REEL * dt).max(ROPE_MIN);
                }

                // tangent yang SELALU menunjuk ke depan (+x)
                let mut tangent = vec2(n.y, -n.x);
                if tangent.x < 0.0 {
                    tangent = -tangent;
                }
                let vt = self.player.vel.dot(tangent);

                // pecah simetri saat benar-benar menggantung mati
                if sp < 70.0 && n.y > 0.5 {
                    self.player.vel += tangent * NUDGE * dt;
                }

                // zona tenaga: di bawah jangkar dan sedang bergerak maju
                let mut strength = 0.0;
                if n.y > 0.15 && vt > 0.0 {
                    strength = ((PUMP_TARGET - vt) / PUMP_TARGET).clamp(0.0, 1.0);
                    self.player.vel += tangent * PUMP_FORCE * strength * dt;
                }
                self.pump = smooth(self.pump, strength, 9.0, dt);

                // batasi energi ayunan
                let sp2 = self.player.vel.length();
                if sp2 > ATTACH_CAP {
                    self.player.vel = self.player.vel / sp2 * ATTACH_CAP;
                }
            }
        } else {
            self.pump = smooth(self.pump, 0.0, 9.0, dt);
        }

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

    /// Lompat dari udara (tap). Dua kali per "napas", diisi ulang saat
    /// menangkap jangkar — jadi tap tidak pernah terasa sia-sia.
    fn do_jump(&mut self, second: bool) {
        self.jump_buf = 0.0;
        self.air_jumps = self.air_jumps.saturating_sub(1);
        unsafe {
            JUMP_COUNT = JUMP_COUNT.wrapping_add(1);
        }
        let v = if second { AIR_JUMP_V } else { JUMP_V };
        self.player.vel.y = -v;
        self.player.squash.bump(-4.6);
        self.shake = self.shake.max(2.2);
        haptic(10);
        Sfx::play(&self.sfx.whoosh, 0.34);
        let p = self.player.pos;
        self.rings.push(Ring {
            pos: p + vec2(0.0, 15.0),
            life: 0.28,
            max: 0.28,
            r0: 10.0,
            r1: if second { 62.0 } else { 42.0 },
            color: if second { MUSTARD } else { PAPER },
            w: 2.4,
        });
        for k in 0..if second { 14 } else { 8 } {
            let ang = -1.35 + k as f32 * 0.19;
            self.parts.push(Particle {
                pos: p + vec2(0.0, 13.0),
                vel: vec2(ang.cos(), ang.sin()) * self.rng.range(60.0, 190.0),
                life: self.rng.range(0.22, 0.46),
                max: 0.46,
                size: self.rng.range(1.6, 3.2),
                color: if second { MUSTARD } else { PAPER_3 },
                strip: false,
                grav: 260.0,
            });
        }
    }

    /// Tap saat menggantung = lepas + lompat. Bukan sekadar jatuh.
    fn hop_off(&mut self) {
        self.player.attached = None;
        self.jump_buf = 0.0;
        self.coyote = 0.14;
        // lompat lepas ini sudah memakai satu "napas"; sisa satu untuk koreksi
        self.air_jumps = self.air_jumps.saturating_sub(1);
        self.player.vel.y = self.player.vel.y.min(-180.0) - 470.0;
        self.player.vel.x += 60.0;
        self.player.squash.bump(3.4);
        self.shake = self.shake.max(2.6);
        haptic(9);
        Sfx::play(&self.sfx.pop, 0.42);
        let p = self.player.pos;
        self.rings.push(Ring {
            pos: p,
            life: 0.26,
            max: 0.26,
            r0: 12.0,
            r1: 54.0,
            color: TEAL,
            w: 2.2,
        });
    }

    // --------------------------------------------------------------- update ----
    fn update(&mut self, dt: f32, view: &View) {
        self.poll_input(view);
        self.time += dt;

        // durasi tahan jari (dipakai untuk bedakan tap vs hold)
        if self.pointer_down {
            self.press_t += dt;
        }
        self.input_lock = (self.input_lock - dt).max(0.0);
        self.jump_buf = (self.jump_buf - dt).max(0.0);
        self.coyote = (self.coyote - dt).max(0.0);
        self.near_miss = (self.near_miss - dt).max(0.0);
        self.intro = (self.intro - dt).max(0.0);
        if self.chain_t > 0.0 {
            self.chain_t = (self.chain_t - dt).max(0.0);
            if self.chain_t <= 0.0 {
                self.chain = 0;
            }
        }
        let today = (get_time() / 86400.0) as u32;
        if today != self.daily_seed {
            self.daily_seed = today;
        }

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
        if self.state == State::Splash {
            self.splash_t += dt;
        } else {
            self.splash_t = 0.0;
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
            State::Splash => {
                // Kredit tampil sendiri; tap mempercepat, tapi tidak bisa
                // membunuh animasi sebelum sempat terbaca (0,8 dtk).
                let done = self.splash_t > 3.15
                    || (self.splash_t > 0.85 && self.press && self.input_lock <= 0.0);
                if done {
                    self.state = State::Title;
                    self.title_t = 0.0;
                    self.input_lock = INPUT_LOCK;
                    self.start_btn.set(0.0);
                    self.start_btn.target = 0.0;
                }
                self.cam = smooth_v(self.cam, self.cam + vec2(26.0, 0.0), 0.35, dt);
            }
            State::Title => {
                self.start_btn.target = 1.0;
                if self.press && self.input_lock <= 0.0 {
                    // Skin cycle: ketuk area bawah kartu kredit.
                    if self.pointer.y > view.vh * 0.83 && self.unlocked > 1 {
                        for _ in 0..4 {
                            self.skin = (self.skin + 1) % 4;
                            if self.unlocked & (1 << self.skin) != 0 {
                                break;
                            }
                        }
                        Sfx::play(&self.sfx.ping, 0.42);
                    } else {
                        self.ui_in.bump(6.0);
                        Sfx::play(&self.sfx.ui, 0.5);
                        self.start_btn.bump(-5.0);
                        self.reset_run(view);
                        self.input_lock = INPUT_LOCK;
                    }
                }
                self.cam = smooth_v(self.cam, self.cam + vec2(26.0, 0.0), 0.35, dt);
            }
            State::Play => self.update_play(dt, view),
            State::Dead => {
                self.dead_t += dt;
                self.again_btn.target = 1.0;
                // Restart instan = mekanik retensi, bukan kenyamanan.
                if self.dead_t > 0.18 && self.press && self.input_lock <= 0.0 {
                    self.again_btn.bump(-5.0);
                    Sfx::play(&self.sfx.ui, 0.5);
                    self.reset_run(view);
                    self.intro = 0.45;
                    self.input_lock = INPUT_LOCK;
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

        // ---- INPUT: TAP = LOMPAT, TAHAN = TANGKAP --------------------------
        // Tap tidak pernah menangkap tali. Tali baru menangkap setelah jari
        // ditahan HOLD_ATTACH detik. Jadi "ketuk" selalu berarti lompat,
        // persis seperti yang diminta — dan ayunan tetap satu jempol.
        //
        // input_lock: tap yang dipakai menekan MULAI tidak boleh ikut
        // terhitung sebagai lepas tali di frame pertama permainan.
        if self.input_lock <= 0.0 {
            if self.press {
                self.hold_done = false;
                self.jump_buf = 0.0;
            }
            if self.pointer_down && !self.hold_done && self.press_t >= HOLD_ATTACH {
                self.hold_done = true;
                if self.player.attached.is_none() {
                    self.try_attach();
                }
                // Tahan tapi tidak ada jangkar = tetap lompat, jangan diam saja.
                if self.player.attached.is_none() && self.air_jumps > 0 {
                    self.jump_buf = 0.14;
                }
            }
            if self.release {
                if self.player.attached.is_some() {
                    self.release();
                } else if self.tap {
                    self.jump_buf = 0.14;
                }
            }
        }

        // ---- LOMPAT / DOUBLE JUMP -----------------------------------------
        // Dua lompatan per "napas"; menangkap jangkar mengisi ulang.
        if self.jump_buf > 0.0 && self.player.attached.is_none() && self.air_jumps > 0 {
            let second = self.air_jumps == 1;
            self.do_jump(second);
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
                keep.push(Anchor {
                    pos: a.pos,
                    r: a.r,
                    phase: a.phase,
                    fresh: a.fresh,
                });
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
        // Meta-progresi: TIDAK ada run yang sia-sia, termasuk run 2 detik.
        let gained = (3 + (self.score / 120.0) as u32 + self.chain.min(6)).min(28);
        self.last_threads = gained;
        self.threads = self.threads.saturating_add(gained);
        if self.threads >= 40 {
            self.unlocked |= 1 << 1;
        }
        if self.threads >= 110 {
            self.unlocked |= 1 << 2;
        }
        if self.threads >= 240 {
            self.unlocked |= 1 << 3;
        }
        if self.unlocked > 1 {
            self.texts.push(FloatText {
                pos: self.player.pos + vec2(0.0, -54.0),
                vel: vec2(0.0, -30.0),
                life: 1.2,
                max: 1.2,
                text: format!("+{} BENANG", gained),
                color: MUSTARD,
                size: 22.0,
            });
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
            self.trail.push(Trail {
                pos: self.player.pos,
                life: 0.42,
            });
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
        // kamera dunia dengan getaran
        let s = self.shake;
        let off = if s > 0.01 {
            let nt = self.time * 20.0;
            vec2(shake_noise(nt), shake_noise(nt + 37.0)) * s
        } else {
            Vec2::ZERO
        };
        let cam = self.cam + off;
        self.draw_world(view, cam);

        // UI (2D, depth mati)
        set_camera(&view.ui_cam());
        self.draw_ui(view);
        set_default_camera();
    }

    fn draw_world(&mut self, view: &View, cam: Vec2) {
        let vw = view.vw;
        let vh = view.vh;
        let t = self.time;

        // ---- LANGIT: latar kertas. Kamera 3D tidak clear background,
        // jadi gradient ini tetap jadi backdrop di belakang semua mesh.
        set_camera(&view.ui_cam());
        clear_background(PALETTE[C_CREAM]);
        sky_gradient(0.0, 0.0, vw, vh);

        // ---- KAMERA 3D: sudut 3/4 ringan supaya permukaan pulau terlihat,
        // tapi bidang main tetap sejajar layar (readability ayunan).
        let cx = cam.x * W3;
        let cy = -cam.y * W3;
        let cam3 = Camera3D {
            position: vec3(cx + 1.6, cy + 2.6, 19.0),
            target: vec3(cx, cy, 0.0),
            up: vec3(0.0, 1.0, 0.0),
            fovy: 50.0_f32.to_radians(),
            aspect: screen_width().max(1.0) / screen_height().max(1.0),
            z_near: 0.1,
            z_far: 260.0,
            ..Default::default()
        };
        set_camera(&cam3);

        let m = &self.meshes;

        // ===================================================== MATAHARI 3D ====
        // Bola emas 3 nilai + tiga cincin halo (semua mesh, bukan sprite 2D).
        let sun = vec3(
            cx + vw * W3 * 0.26,
            cy + vh * W3 * 0.36,
            -18.0,
        );
        for i in 0..3 {
            let k = i as f32;
            draw_at(
                &m.ball[C_GOLD],
                model(
                    sun,
                    vec3(1.05 + k * 0.42, 1.05 + k * 0.42, 0.3),
                    0.0,
                    0.0,
                ),
            );
        }
        draw_at(&m.ball[C_GOLD], model(sun + vec3(0.04, -0.05, 0.2), vec3(1.0, 1.0, 0.3), 0.0, 0.0));
        draw_at(
            &m.ball[C_CREAM],
            model(sun + vec3(-0.22, 0.26, 0.42), vec3(0.48, 0.48, 0.2), 0.0, 0.0),
        );

        // ============================================================ AWAN ====
        // Kubus/kumpulan bola low-poly di tiga kedalaman.
        for layer in 0..3usize {
            let fi = layer as f32;
            let span = 7.0;
            let z = -11.0 - fi * 2.2;
            let drift = (t * (0.10 + fi * 0.05) + cam.x * W3 * (0.05 + fi * 0.05)).rem_euclid(span);
            for k in -3..4 {
                let kf = k as f32;
                let h = hash01((k + 90) as u32 ^ (layer as u32) * 7919);
                let x = cx + (kf * span + drift - span * 0.5) * (1.0 + fi * 0.15);
                let y = cy + 3.0 + fi * 0.7 + h * 1.6 + (t * 0.3 + kf).sin() * 0.10;
                let s = 0.55 + fi * 0.30 + h * 0.28;
                let col = if layer == 2 { C_WHITE } else { C_CREAM };
                let bob = vec3(x, y, z);
                draw_at(&m.ball[col], model(bob, vec3(s, s * 0.42, s * 0.6), 0.0, 0.0));
                draw_at(
                    &m.ball[col],
                    model(bob + vec3(-s * 0.55, -s * 0.12, 0.0), vec3(s * 0.66, s * 0.32, s * 0.5), 0.0, 0.0),
                );
                draw_at(
                    &m.ball[col],
                    model(bob + vec3(s * 0.58, -s * 0.16, 0.0), vec3(s * 0.60, s * 0.30, s * 0.5), 0.0, 0.0),
                );
            }
        }

        // ========================================================= BUKIT ======
        // Tiga lapisan kerucut, makin dekat makin gelap & besar.
        for (li, &col) in [C_SKY, C_MINT, C_MINT].iter().enumerate() {
            let fi = li as f32;
            let z = -8.0 + fi * 2.4;
            let step = 3.4 - fi * 0.5;
            let ox = cx * (0.20 + fi * 0.18);
            let i0 = ((ox - 7.0) / step).floor() as i64 - 1;
            let i1 = ((ox + 7.0) / step).ceil() as i64 + 1;
            let mut i = i0;
            while i <= i1 {
                let h = hash01(i as u32 ^ (li as u32 + 31).wrapping_mul(2654435761));
                let x = i as f32 * step - ox;
                let hgt = 1.0 + h * 1.5 + fi * 0.3;
                let base = cy - 3.2 - fi * 0.35;
                let c = if li == 2 { shade(PALETTE[col], 0.90) } else { PALETTE[col] };
                draw_at(
                    &m.cone[if li == 0 { C_SKY } else { C_MINT }],
                    model(vec3(x, base + hgt * 0.5, z), vec3(step * 1.15, hgt, step * 1.15), 0.0, 0.0),
                );
                if li == 2 && h > 0.62 {
                    // bayangan jatuh ke lapisan belakang = depth cue
                    draw_at(
                        &m.cone[C_INK],
                        model(vec3(x + 0.22, base + hgt * 0.5 - 0.18, z - 0.4), vec3(step * 1.1, hgt * 0.95, step * 1.1), 0.0, 0.0),
                    );
                }
                let _ = c;
                i += 1;
            }
        }

        // ====================================================== PULAU =========
        // Pulau melayang: batu kerucut terbalik + rumput silinder + pohon.
        for li in 0..3usize {
            let fi = li as f32;
            let step = 6.5 + fi * 2.2;
            let ox = cx * (0.24 + fi * 0.14);
            let i0 = ((ox - 8.0) / step).floor() as i64 - 1;
            let i1 = ((ox + 8.0) / step).ceil() as i64 + 1;
            let mut i = i0;
            while i <= i1 {
                let h = hash01(i as u32 ^ (li as u32 + 71).wrapping_mul(2246822519));
                let x = i as f32 * step - ox + h * 1.5;
                let y = cy + 1.6 + fi * 1.1 + h * 2.2 - (t * 0.20 + h * 6.28).sin() * 0.12;
                let z = -3.6 - fi * 2.0;
                let w = 0.95 + h * 0.85 + fi * 0.25;
                // batu (kerucut terbalik)
                let mut mm = Mat4::from_translation(vec3(x, y - w * 0.55, z));
                mm = mm * Mat4::from_rotation_z(std::f32::consts::PI);
                draw_at(&m.cone[C_SLATE], mm * Mat4::from_scale(vec3(w, w * 1.15, w)));
                // rumput
                draw_at(
                    &m.disc[C_MINT],
                    model(vec3(x, y, z), vec3(w * 2.0, w * 0.5, w * 2.0), 0.0, 0.0),
                );
                // pohon kecil + totem yang menoleh saat chain hidup
                if h > 0.35 {
                    draw_at(
                        &m.cone[C_MINT],
                        model(vec3(x + w * 0.45, y + w * 0.5, z), vec3(w * 0.55, w * 1.0, w * 0.55), 0.0, 0.0),
                    );
                }
                if self.chain > 0 && li == 0 {
                    let follow = (t * 0.6).sin() * 0.5;
                    draw_at(
                        &m.cone[C_PLUM],
                        model(vec3(x - w * 0.45, y + w * 0.42, z), vec3(w * 0.26, w * 0.5, w * 0.26), follow, 0.0),
                    );
                    draw_orb(vec3(x - w * 0.45, y + w * 0.52, z + w * 0.14), w * 0.06, PALETTE[C_WHITE]);
                }
                i += 1;
            }
        }

        // ========================================================= JANGKAR ====
        let anchor_col = if self.chain >= 3 { C_GOLD } else { C_INK };
        for (i, a) in self.anchors.iter().enumerate() {
            let p = p3(a.pos);
            let rr = a.r * W3 * 2.6 * (1.0 + a.fresh * 0.14);
            // rantai ke atas
            for k in 0..5 {
                draw_at(
                    &m.cube[C_SLATE],
                    model(p + vec3(0.0, 0.16 + k as f32 * 0.20, -0.04), vec3(0.055, 0.11, 0.055), (k % 2) as f32 * 0.7, 0.0),
                );
            }
            // cakram utama
            draw_at(&m.disc[anchor_col], model(p, vec3(rr, rr * 0.5, rr), 0.0, 0.0));
            // lubang teal
            draw_at(
                &m.disc[C_BLUE],
                model(p + vec3(0.0, 0.07, 0.0), vec3(rr * 0.42, rr * 0.2, rr * 0.42), 0.0, 0.0),
            );
            // UKIRAN: bekas tangan penambat — 3 goresan tipis.
            // Hanya jelas saat `fresh` (baru dipakai) atau saat chain hidup.
            let eg = (0.30 + a.fresh * 0.55 + self.chain.min(4) as f32 * 0.06).min(1.0);
            if eg > 0.35 {
                let ec = mix(PALETTE[C_CREAM], PALETTE[C_INK], eg);
                for k in 0..3 {
                    let ang = -0.5 + k as f32 * 0.5 + i as f32 * 0.13;
                    let a0 = p + vec3(ang.cos() * rr * 0.18, 0.085, ang.sin() * rr * 0.18);
                    let a1 = p + vec3(ang.cos() * rr * 0.42, 0.085, ang.sin() * rr * 0.42);
                    draw_beam(a0, a1, 0.012, &m.cube[C_INK]);
                    let _ = ec;
                }
            }
        }

        // ============================================================ TALI ====
        if let Some(ai) = self.player.attached {
            if let Some(a) = self.anchors.get(ai) {
                let a3 = p3(a.pos);
                let b3 = p3(self.player.pos);
                let rope_col = if self.pump > 0.25 { C_GOLD } else { C_SLATE };
                let seg = 12usize;
                for k in 0..seg {
                    let k0 = k as f32 / seg as f32;
                    let k1 = (k + 1) as f32 / seg as f32;
                    let p0 = a3.lerp(b3, k0);
                    let p1 = a3.lerp(b3, k1);
                    // sedikit kendur di tengah, terasa seperti tali
                    let sag = (k0 * std::f32::consts::PI).sin() * 0.035;
                    draw_beam(
                        p0 + vec3(0.0, -sag, 0.02),
                        p1 + vec3(0.0, -((k1 * std::f32::consts::PI).sin() * 0.035), 0.02),
                        0.018,
                        &m.cube[rope_col],
                    );
                }
            }
        }

        // ===================================================== KUNANG ========
        // Jejak timing: kunang menyala di busur yang benar, bukan acak.
        for (i, a) in self.anchors.iter().enumerate() {
            if i > 6 {
                break;
            }
            let h = hash01(i as u32 ^ 0x5C1A_9E33);
            let ang = -0.8 + h * 1.6 + (t * 0.9 + h * 6.28).sin() * 0.22;
            let rr = a.r * W3 * 3.4;
            let kp = p3(a.pos) + vec3(ang.sin() * rr, ang.cos() * rr, 0.25);
            let pulse = 0.65 + 0.35 * (t * 4.0 + h * 9.0).sin();
            let size = (0.035 + h * 0.02) * pulse;
            draw_orb(kp, size, PALETTE[C_GOLD]);
            draw_orb(kp + vec3(0.0, 0.0, 0.01), size * 0.45, PALETTE[C_WHITE]);
        }

        // =========================================================== PETIR ====
        // Datang DARI BAWAH: lautan awan gelap menyambar jangkar.
        for b in self.bolts.iter() {
            let bx = b.pos.x * W3;
            let by = -b.pos.y * W3;
            if b.warn > 0.0 && !b.struck {
                let g = 0.15 + 0.10 * (t * 18.0).sin();
                draw_at(
                    &m.ball[C_BLUE],
                    model(vec3(bx, by - 3.4, -0.6), vec3(0.9, 0.35, 0.5), 0.0, 0.0),
                );
                let _ = g;
            }
            if b.struck {
                let top = vec3(bx, by, 0.15);
                let bot = vec3(bx + 0.18, by - 3.2, -0.4);
                draw_beam(bot, top, 0.030, &m.cube[C_WHITE]);
                draw_beam(bot + vec3(-0.10, 0.0, 0.0), top, 0.012, &m.cube[C_GOLD]);
            }
        }

        // ========================================================== JEJAK ====
        for tr in self.trail.iter() {
            let a = (tr.life / 0.42).clamp(0.0, 1.0);
            let s = (0.05 + a * 0.13) * W3 * 70.0;
            draw_at(
                &m.wedge[C_CORAL],
                model(p3(tr.pos) + vec3(0.0, 0.0, 0.06), vec3(s, s * 0.7, s), 0.0, (t * 2.0).sin() * 0.2),
            );
        }

        // ========================================================== PEMAIN ===
        if self.state == State::Play || self.state == State::Dead {
            let p = p3(self.player.pos);
            let r = self.player.r * W3;
            let sq = self.player.squash.value;
            let skin_col = match self.skin {
                1 => C_GOLD,
                2 => C_BLUE,
                3 => C_MINT,
                _ => C_CORAL,
            };
            let sc = if self.last_life > 0.0 { C_GOLD } else { skin_col };
            // bayangan kontak di bidang belakang
            draw_at(
                &m.disc[C_SLATE],
                model(p + vec3(0.18, -r * 0.9, -0.5), vec3(r * 2.0, r * 0.14, r * 1.3), 0.0, 0.0),
            );
            // badan bola 3 nilai + squash
            draw_at(
                &m.ball[sc],
                model(p, vec3(r * 2.0 * sq, r * 2.0 / sq, r * 2.0), 0.0, 0.0),
            );
            // sorot kiri-atas
            draw_orb(p + vec3(-r * 0.42, r * 0.45, r * 0.78), r * 0.24, PALETTE[C_WHITE]);
            // mata: arah gerak
            let dir = self.player.vel.normalize_or_zero();
            let look = if dir.length_squared() > 0.01 { dir } else { vec2(1.0, 0.0) };
            let ex = p.x + look.x * r * 0.42;
            let ey = p.y - look.y * r * 0.42;
            draw_at(&m.disc[C_WHITE], model(vec3(ex, ey, r * 0.92), vec3(r * 0.55, r * 0.28, r * 0.55), 0.0, 0.0));
            draw_at(&m.disc[C_INK], model(vec3(ex + look.x * r * 0.08, ey - look.y * r * 0.08, r * 0.99), vec3(r * 0.24, r * 0.24, r * 0.24), 0.0, 0.0));
            // syal: 3 segitiga mengikuti arah angin
            for k in 0..3 {
                let kf = k as f32;
                let wave = (t * 6.0 + kf * 1.7).sin() * 0.20;
                draw_at(
                    &m.wedge[C_CORAL],
                    model(
                        p + vec3(-look.x * r * (1.0 + kf * 0.55), look.y * r * (1.0 + kf * 0.55), 0.10),
                        vec3(r * (1.0 - kf * 0.22), r * (0.9 - kf * 0.2), r),
                        0.0,
                        wave,
                    ),
                );
            }
            // indikator lompat: dua titik
            if self.air_jumps > 0 && self.player.attached.is_none() {
                for k in 0..2u8 {
                    let on = k < self.air_jumps;
                    let jx = p.x + (k as f32 - 0.5) * 0.22;
                    let jy = p.y - r * 1.35;
                    draw_orb(
                        vec3(jx, jy, r + 0.12),
                        0.045,
                        if on { PALETTE[C_GOLD] } else { PALETTE[C_SLATE] },
                    );
                }
            }
        }

        // ====================================================== PARTIKEL ====
        for pt in self.parts.iter() {
            let a = (pt.life / pt.max).clamp(0.0, 1.0);
            let s = pt.size * W3 * (0.5 + a * 0.7);
            draw_orb(p3(pt.pos) + vec3(0.0, 0.0, 0.12), s.max(0.012), pt.color);
        }

        // ======================================================= RING ========
        for r in self.rings.iter() {
            let k = 1.0 - r.life / r.max;
            let rad = (r.r0 + (r.r1 - r.r0) * ease_out_cubic(k)) * W3;
            for s in 0..12 {
                let a = s as f32 / 12.0 * std::f32::consts::TAU;
                draw_orb(
                    p3(r.pos) + vec3(a.cos() * rad, a.sin() * rad, 0.18),
                    0.022 * (1.0 - k * 0.5),
                    r.color,
                );
            }
        }

        // ================================================== FOREGROUND =======
        // Siluet dedaunan paling dekat: memotong segalanya = bingkai hidup.
        let fg_step = 2.6;
        let fg_ox = cam.x * W3 * 1.34;
        let fi0 = ((fg_ox - 6.0) / fg_step).floor() as i64 - 1;
        let fi1 = ((fg_ox + 6.0) / fg_step).ceil() as i64 + 1;
        let mut fi = fi0;
        while fi <= fi1 {
            let h = hash01(fi as u32 ^ 0x2F1B_9C4D);
            let x = fi as f32 * fg_step - fg_ox;
            let y = cy + 4.4 - h * 1.2;
            let len = 0.55 + h * 0.9;
            let sway = (t * 0.8 + h * 6.28).sin() * 0.10;
            draw_beam(
                vec3(x, y + 0.6, 2.8),
                vec3(x + sway, y + 0.6 - len, 2.8),
                0.030,
                &m.cube[C_INK],
            );
            for k in 0..3 {
                let kf = k as f32;
                let ly = y + 0.6 - len * (0.35 + kf * 0.22);
                let lx = x + sway * (0.35 + kf * 0.22);
                let lr = 0.20 + h * 0.10 - kf * 0.03;
                draw_at(
                    &m.wedge[C_INK],
                    model(vec3(lx + (if k % 2 == 0 { -1.0 } else { 1.0 }) * lr * 0.7, ly, 2.82), vec3(lr, lr, lr), 0.0, (if k % 2 == 0 { 1.0 } else { -1.0 }) * 0.7),
                );
            }
            fi += 1;
        }

        // ============================================ TEKS & EFEK 2D ========
        set_camera(&view.ui_cam());
        for tx in self.texts.iter() {
            let a = (tx.life / tx.max).clamp(0.0, 1.0);
            let sp = tx.pos - cam + vec2(vw * 0.5, vh * 0.5);
            draw_txt_c(&tx.text, sp.x, sp.y + 2.5, tx.size, with_a(INK, a * 0.20), &self.font_d);
            draw_txt_c(&tx.text, sp.x, sp.y, tx.size, with_a(tx.color, a), &self.font_d);
        }

        // near-miss: "hampir" jadi momen, bukan sekadar mati
        if self.near_miss > 0.0 {
            let a = (self.near_miss / 0.55).clamp(0.0, 1.0);
            draw_rectangle(0.0, 0.0, vw, vh, with_a(MUSTARD, a * 0.10));
            draw_txt_c(
                "SO CLOSE!",
                vw * 0.5,
                vh * 0.34 + (1.0 - a) * 10.0,
                28.0,
                with_a(MUSTARD, a),
                &self.font_d,
            );
        }

        // petir dari bawah: tepi bawah memanas sebelum sambaran
        if self.bolts.iter().any(|b| b.warn > 0.0 && !b.struck) {
            for i in 0..6 {
                let k = i as f32 / 5.0;
                draw_rectangle(0.0, vh - 1.0 - k * 34.0, vw, 2.0, with_a(PALETTE[C_BLUE], (1.0 - k) * 0.14));
            }
        }

        set_default_camera();
    }


    fn draw_ui(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;

        match self.state {
            State::Splash => self.draw_splash(view),
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

    fn draw_splash(&mut self, view: &View) {
        let vw = view.vw;
        let vh = view.vh;
        let f = self.font_d.clone();
        let fm = self.font_m.clone();
        let t = self.splash_t;

        // ---- tirai tinta: diorama masih terlihat samar di belakang
        let curtain = ease_out_cubic((t / 0.32).clamp(0.0, 1.0));
        for i in 0..18 {
            let k = i as f32 / 17.0;
            let c = mix(shade(INK, 0.86), INK, k);
            draw_rectangle(0.0, k * vh, vw, vh / 17.0 + 2.0, with_a(c, 0.985 * curtain));
        }

        // ---- garis-garis diagonal: tekstur poster bergerak pelan
        let phase = (t * 26.0).rem_euclid(64.0);
        let mut x = -vh + phase - 64.0;
        while x < vw + 64.0 {
            draw_line(x, vh, x + vh, 0.0, 1.6, with_a(VERMILION, 0.045 * curtain));
            x += 64.0;
        }

        // ---- bingkai tipis + pita vermilion di atas
        let frame = ease_out_cubic(((t - 0.10) / 0.45).clamp(0.0, 1.0));
        if frame > 0.01 {
            let ins = 22.0;
            let len = 26.0 * frame;
            let col = with_a(PAPER, 0.24 * frame);
            for (cx, cy, sx, sy) in [
                (ins, ins, 1.0f32, 1.0f32),
                (vw - ins, ins, -1.0, 1.0),
                (ins, vh - ins, 1.0, -1.0),
                (vw - ins, vh - ins, -1.0, -1.0),
            ] {
                draw_line(cx, cy, cx + len * sx, cy, 1.4, col);
                draw_line(cx, cy, cx, cy + len * sy, 1.4, col);
            }
            let bar = 92.0 * frame;
            draw_rectangle(vw * 0.5 - bar * 0.5, ins + 10.0, bar, 4.0, VERMILION);
        }

        // ---- "by" kecil, sendiri, lalu nama besar
        let by_p = ease_out_cubic(((t - 0.34) / 0.42).clamp(0.0, 1.0));
        if by_p > 0.001 {
            draw_txt_c(
                "by",
                vw * 0.5,
                vh * 0.325 + (1.0 - by_p) * 12.0,
                17.0,
                with_a(PAPER, 0.52 * by_p),
                &fm,
            );
        }

        // ---- BAGUS: tiap huruf menghantam dari skala 2,4 → 1
        let name = "BAGUS";
        let size = 78.0;
        let track = 3.0;
        let chars: Vec<char> = name.chars().collect();
        let widths: Vec<f32> = chars
            .iter()
            .map(|c| text_w(&c.to_string(), &f, size))
            .collect();
        let total: f32 = widths.iter().sum::<f32>() + track * (chars.len() - 1) as f32;
        let mut cx = vw * 0.5 - total * 0.5;
        let cy = vh * 0.415;
        for (i, c) in chars.iter().enumerate() {
            let p = ((t - 0.44 - i as f32 * 0.075) / 0.40).clamp(0.0, 1.0);
            if p > 0.001 {
                let e = ease_out_cubic(p);
                // overshoot kecil di akhir: terasa "membal" seperti stempel
                let pop = 1.0 + (1.0 - e) * 1.55;
                let rot = (1.0 - e) * if i % 2 == 0 { -0.42 } else { 0.42 };
                let ch = c.to_string();
                let gx = cx + widths[i] * 0.5;
                draw_glyph(
                    &ch,
                    gx + 2.2,
                    cy + (1.0 - e) * 46.0 + 3.0,
                    size,
                    pop,
                    rot,
                    with_a(shade(VERMILION, 0.45), e),
                    &f,
                );
                draw_glyph(
                    &ch,
                    gx,
                    cy + (1.0 - e) * 46.0,
                    size,
                    pop,
                    rot,
                    with_a(PAPER, e),
                    &f,
                );
            }
            cx += widths[i] + track;
        }

        // ---- garis: menggambar dari tengah ke luar
        let rp = ease_out_cubic(((t - 0.94) / 0.50).clamp(0.0, 1.0));
        if rp > 0.01 {
            let w = 232.0 * rp;
            let y = cy + 42.0;
            draw_rectangle(vw * 0.5 - w * 0.5, y, w, 2.0, with_a(PAPER, 0.30));
            draw_rectangle(vw * 0.5 - 7.0, y - 3.0, 14.0, 8.0, with_a(VERMILION, rp));
        }

        // ---- GosGilA: huruf meluncur naik satu per satu
        let handle = "GosGilA";
        let hs = 32.0;
        let htrack = 7.0;
        let hchars: Vec<char> = handle.chars().collect();
        let hw: Vec<f32> = hchars
            .iter()
            .map(|c| text_w(&c.to_string(), &f, hs))
            .collect();
        let htotal: f32 = hw.iter().sum::<f32>() + htrack * (hchars.len() - 1) as f32;
        let mut hx = vw * 0.5 - htotal * 0.5;
        let hy = cy + 82.0;
        for (i, c) in hchars.iter().enumerate() {
            let p = ease_out_cubic(((t - 1.06 - i as f32 * 0.045) / 0.42).clamp(0.0, 1.0));
            let ch = c.to_string();
            draw_txt(
                &ch,
                hx,
                hy + (1.0 - p) * 26.0 + 2.0,
                hs,
                with_a(shade(VERMILION, 0.4), p * 0.8),
                &f,
            );
            draw_txt(&ch, hx, hy + (1.0 - p) * 26.0, hs, with_a(VERMILION, p), &f);
            hx += hw[i] + htrack;
        }

        // ---- identitas game kecil di bawah
        let gp = ease_out_cubic(((t - 1.52) / 0.55).clamp(0.0, 1.0));
        if gp > 0.001 {
            draw_txt_c(
                "AYUN",
                vw * 0.5,
                vh * 0.700 + (1.0 - gp) * 10.0,
                15.0,
                with_a(PAPER, 0.34 * gp),
                &fm,
            );
            draw_txt_c(
                "satu jempol. jangan jatuh.",
                vw * 0.5,
                vh * 0.740 + (1.0 - gp) * 10.0,
                15.0,
                with_a(PAPER, 0.62 * gp),
                &fm,
            );
        }

        // ---- ajakan lewati
        let tp = ease_out_cubic(((t - 1.85) / 0.5).clamp(0.0, 1.0));
        if tp > 0.001 {
            let blink = 0.55 + 0.45 * (t * 3.2).sin();
            draw_txt_c(
                "TAP UNTUK LANJUT",
                vw * 0.5,
                vh * 0.905,
                12.0,
                with_a(PAPER, 0.42 * tp * blink),
                &fm,
            );
        }

        // ---- tutup ke kertas sebelum masuk menu (bukan cut kasar)
        if t > 2.90 {
            let k = ((t - 2.90) / 0.25).clamp(0.0, 1.0);
            draw_rectangle(0.0, 0.0, vw, vh, with_a(PAPER, ease_out_cubic(k)));
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
                    draw_txt(
                        &s,
                        x,
                        wm_y + (1.0 - p) * 54.0 + dy,
                        size,
                        with_a(col, p),
                        &f,
                    );
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
            "TAHAN  =  TANGKAP & AYUN",
            "LEPAS  =  TERBANG",
            "TAP    =  LOMPAT  •  2 TAP = DOBEL",
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
            let cy = vh * 0.760 + (1.0 - cp) * 16.0;
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

        // ---- meta-progresi: benang & skin (ketuk area ini buat ganti warna)
        let mp = ease_out_cubic(((tt - 1.62) / 0.6).clamp(0.0, 1.0));
        if mp > 0.01 && self.unlocked > 1 {
            let names = ["MERAH", "EMAS", "BIRU", "HIJAU"];
            let line = format!("{} BENANG   ·   SKIN: {}", self.threads, names[self.skin as usize]);
            draw_txt_c(
                &line,
                vw * 0.5,
                vh * 0.835 + (1.0 - mp) * 8.0,
                12.0,
                with_a(INK_SOFT, 0.85 * mp),
                &fm,
            );
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
            draw_line(
                pivot.x + 1.5,
                pivot.y + 2.0,
                bob2.x + 1.5,
                bob2.y + 2.0,
                2.0,
                with_a(INK, 0.14 * op),
            );
            draw_line(
                pivot.x,
                pivot.y,
                bob2.x,
                bob2.y,
                1.8,
                with_a(INK, 0.34 * op),
            );
            draw_circle(
                bob2.x + 1.2,
                bob2.y + 2.0,
                9.0,
                with_a(shade(INK, 0.5), 0.5 * op),
            );
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
        shadow_at(
            &self.shadow_tex,
            cx + 2.0,
            y + h + 10.0,
            w * 0.62,
            14.0,
            0.22,
        );
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
        draw_txt(
            "SKOR",
            34.0,
            y0 + 25.0,
            11.0,
            with_a(INK_SOFT, 0.9 * ui),
            &fm,
        );
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

        // ---- chain & style: ekspresi mastery, bukan sekadar jarak
        if self.chain > 0 {
            let a = (self.chain_t / 1.6).clamp(0.0, 1.0);
            let label = format!("CHAIN ×{}", self.chain);
            let lw = text_w(&label, &fm, 15.0) + 30.0;
            pill_card(18.0, y0 + 80.0, lw, 34.0, 17.0, ui * a);
            draw_txt(&label, 33.0, y0 + 97.0, 15.0, with_a(MUSTARD, ui * a), &fm);
        }
        if self.style > 0.0 {
            let st = format!("STYLE {}", self.style as i32);
            let stw = text_w(&st, &fm, 12.0);
            draw_txt(
                &st,
                vw - 22.0 - stw,
                y0 + 82.0,
                12.0,
                with_a(INK_SOFT, ui * 0.9),
                &fm,
            );
        }

        // ---- terbaik
        let b = format!("TERBAIK {}", self.best as i32);
        let bw = text_w(&b, &fm, 12.0);
        draw_txt(
            &b,
            vw - 22.0 - bw,
            y0 + 60.0,
            12.0,
            with_a(INK_SOFT, 0.72 * ui),
            &fm,
        );

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
            draw_round_rect_border(ccx - lw * 0.5, ccy - chh * 0.5, lw, chh, 16.0, 1.8, INK, cc);
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
                "TAP = LOMPAT  •  TAHAN = AYUN"
            } else if self.air_jumps > 0 {
                "TAP = LOMPAT  •  TAHAN = TANGKAP"
            } else {
                "TAHAN untuk menangkap jangkar"
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
        shadow_at(
            &self.shadow_tex,
            cx + 4.0,
            y0 + ch + 16.0,
            cw * 0.62,
            26.0,
            0.26 * e,
        );
        draw_round_rect(
            x0 + 3.0,
            y0 + 10.0,
            cw,
            ch,
            24.0,
            with_a(shade(PAPER, 0.80), e),
        );
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
        draw_round_rect(
            x0 + 2.0,
            y0 + 2.0,
            cw - 4.0,
            46.0,
            22.0,
            with_a(VERMILION, e),
        );
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
        let head = if self.death_reason == 0 {
            "JATUH"
        } else {
            "HABIS"
        };
        draw_txt_c(head, cx, top + 26.0, 30.0, with_a(PAPER, e), &f);

        let sub = if self.death_reason == 0 {
            "jatuh ke jurang".to_string()
        } else if self.bolt_hits > 1 {
            format!("petir x{}", self.bolt_hits)
        } else {
            "petir".to_string()
        };
        let sub = format!("{}  ·  +{} BENANG", sub, self.last_threads);
        draw_txt_c(&sub, cx, top + 78.0, 15.0, INK_SOFT, &fm);

        // skor besar: 3 nilai
        draw_txt_c("SKOR", cx, top + 112.0, 13.0, with_a(INK_SOFT, 0.8), &fm);
        let score_txt = format!("{}", self.score as i32);
        draw_txt_c(
            &score_txt,
            cx,
            top + 160.0,
            54.0,
            with_a(shade(INK, 0.55), e),
            &f,
        );
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
            ("STYLE", format!("{}", self.style as i32)),
            ("TERBAIK", format!("{}", self.best as i32)),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = top + 198.0 + i as f32 * 25.0;
            draw_round_rect(
                x0 + 24.0,
                y - 15.0,
                cw - 48.0,
                24.0,
                12.0,
                with_a(PAPER_2, 0.55 * e),
            );
            draw_txt(k, cx - 106.0, y, 14.0, with_a(INK_SOFT, 0.9), &fm);
            let vw_ = text_w(v, &fm, 14.0);
            draw_txt(v, cx + 106.0 - vw_, y, 14.0, INK, &fm);
        }

        if self.new_best {
            draw_txt_c("REKOR BARU", cx, top + 306.0, 15.0, VERMILION, &fm);
        } else if self.best > 1.0 {
            let gap = (self.best - self.score).max(0.0) as i32;
            let line = if gap <= 3 {
                format!("SELISIH {}. ANJIR.", gap)
            } else {
                format!("SELISIH {}", gap)
            };
            let col = if gap <= 3 { VERMILION } else { INK_SOFT };
            draw_txt_c(&line, cx, top + 306.0, 15.0, col, &fm);
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
            DEBUG_GAME = Some(&game as *const Game);
        }
        next_frame().await;
    }
}
