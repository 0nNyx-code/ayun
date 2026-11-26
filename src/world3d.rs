// world3d.rs — fondasi renderer low-poly 3D untuk AYUN.
//
// Semua bentuk dibangun dari kode (tanpa file FBX/GLB). Satu arah cahaya,
// tiga nilai warna per hue (light / mid / dark) — aturan Fancade.
// Shader default macroquad TIDAK memakai `normal`, jadi shading sudah
// dipanggang ke warna tiap sisi saat mesh dibuat.

use macroquad::prelude::*;

/// 70 pixel dunia = 1 unit 3D.
pub const W3: f32 = 1.0 / 70.0;

/// Konversi posisi dunia 2D (x maju, y ke bawah) → posisi 3D (Y ke atas).
pub fn p3(v: Vec2) -> Vec3 {
    vec3(v.x * W3, -v.y * W3, 0.0)
}

// ---------------------------------------------------------------- palet ----
pub const C_CREAM: usize = 0;
pub const C_CORAL: usize = 1;
pub const C_GOLD: usize = 2;
pub const C_BLUE: usize = 3;
pub const C_MINT: usize = 4;
pub const C_INK: usize = 5;
pub const C_SLATE: usize = 6;
pub const C_PLUM: usize = 7;
pub const C_SKY: usize = 8;
pub const C_WHITE: usize = 9;

pub const PALETTE: [Color; 10] = [
    Color::new(1.000, 0.953, 0.878, 1.0), // 0 cream  #FFF3E0
    Color::new(1.000, 0.408, 0.302, 1.0), // 1 coral  #FF684D
    Color::new(1.000, 0.769, 0.000, 1.0), // 2 gold   #FFC400
    Color::new(0.000, 0.537, 1.000, 1.0), // 3 blue   #0089FF
    Color::new(0.239, 0.702, 0.278, 1.0), // 4 mint   #3DB347
    Color::new(0.106, 0.106, 0.149, 1.0), // 5 ink    #1B1B26
    Color::new(0.227, 0.227, 0.302, 1.0), // 6 slate  #3A3A4D
    Color::new(0.357, 0.290, 0.420, 1.0), // 7 plum   #5B4A6B
    Color::new(0.776, 0.906, 0.847, 1.0), // 8 sky    #C6E7D8
    Color::new(1.000, 1.000, 1.000, 1.0), // 9 white
];

/// Arah cahaya tunggal: kiri-atas-depan.
fn light_dir() -> Vec3 {
    vec3(-0.55, 0.78, 0.32).normalize()
}

fn tone(n: Vec3) -> f32 {
    let d = n.dot(light_dir());
    if d > 0.70 {
        1.0
    } else if d > 0.25 {
        0.80
    } else {
        0.58
    }
}

fn dim(base: Color, k: f32) -> Color {
    Color::new(
        (base.r * k).clamp(0.0, 1.0),
        (base.g * k).clamp(0.0, 1.0),
        (base.b * k).clamp(0.0, 1.0),
        base.a,
    )
}

fn tinted(base: Color, n: Vec3) -> Color {
    let k = tone(n);
    Color::new(
        (base.r * k).clamp(0.0, 1.0),
        (base.g * k).clamp(0.0, 1.0),
        (base.b * k).clamp(0.0, 1.0),
        base.a,
    )
}

fn mesh_from_faces(faces: &[(Vec3, Vec3, Vec3, Color)]) -> Mesh {
    let mut vertices = Vec::with_capacity(faces.len() * 3);
    let mut indices = Vec::with_capacity(faces.len() * 3);
    for (a, b, c, col) in faces {
        let s = vertices.len() as u16;
        let cc: [u8; 4] = (*col).into();
        vertices.push(Vertex {
            position: *a,
            uv: vec2(0.0, 0.0),
            color: cc,
            normal: vec4(0.0, 0.0, 0.0, 0.0),
        });
        vertices.push(Vertex {
            position: *b,
            uv: vec2(0.0, 0.0),
            color: cc,
            normal: vec4(0.0, 0.0, 0.0, 0.0),
        });
        vertices.push(Vertex {
            position: *c,
            uv: vec2(0.0, 0.0),
            color: cc,
            normal: vec4(0.0, 0.0, 0.0, 0.0),
        });
        indices.extend_from_slice(&[s, s + 1, s + 2]);
    }
    Mesh {
        vertices,
        indices,
        texture: None,
    }
}

/// Kubus 6 sisi, tiap sisi satu nilai warna.
fn cube(base: Color) -> Mesh {
    let h = 0.5;
    let p = [
        vec3(-h, -h, -h),
        vec3(h, -h, -h),
        vec3(h, h, -h),
        vec3(-h, h, -h),
        vec3(-h, -h, h),
        vec3(h, -h, h),
        vec3(h, h, h),
        vec3(-h, h, h),
    ];
    let faces: [([usize; 4], Vec3); 6] = [
        ([4, 5, 6, 7], vec3(0.0, 0.0, 1.0)),
        ([1, 0, 3, 2], vec3(0.0, 0.0, -1.0)),
        ([5, 1, 2, 6], vec3(1.0, 0.0, 0.0)),
        ([0, 4, 7, 3], vec3(-1.0, 0.0, 0.0)),
        ([7, 6, 2, 3], vec3(0.0, 1.0, 0.0)),
        ([0, 1, 5, 4], vec3(0.0, -1.0, 0.0)),
    ];
    let mut tris = Vec::with_capacity(12);
    for (f, n) in faces.iter() {
        let c = tinted(base, *n);
        tris.push((p[f[0]], p[f[1]], p[f[2]], c));
        tris.push((p[f[0]], p[f[2]], p[f[3]], c));
    }
    mesh_from_faces(&tris)
}

/// Bola lat-long dengan flat shading per segitiga.
fn ball(base: Color, seg: u32, rings: u32) -> Mesh {
    let pt = |a: f32, b: f32| vec3(a.sin() * b.cos(), a.cos(), a.sin() * b.sin());
    let mut tris = Vec::new();
    for j in 0..rings {
        let a0 = std::f32::consts::PI * j as f32 / rings as f32;
        let a1 = std::f32::consts::PI * (j + 1) as f32 / rings as f32;
        for i in 0..seg {
            let b0 = 2.0 * std::f32::consts::PI * i as f32 / seg as f32;
            let b1 = 2.0 * std::f32::consts::PI * (i + 1) as f32 / seg as f32;
            let p00 = pt(a0, b0);
            let p01 = pt(a0, b1);
            let p10 = pt(a1, b0);
            let p11 = pt(a1, b1);
            let n = (p10 - p00).cross(p01 - p00).normalize_or_zero();
            let c = tinted(base, n);
            tris.push((p00, p10, p11, c));
            tris.push((p00, p11, p01, c));
        }
    }
    mesh_from_faces(&tris)
}

/// Kerucut (pohon, puncak pulau, totem).
fn cone(base: Color, seg: u32) -> Mesh {
    let apex = vec3(0.0, 1.0, 0.0);
    let r = 0.5;
    let mut tris = Vec::new();
    for i in 0..seg {
        let b0 = 2.0 * std::f32::consts::PI * i as f32 / seg as f32;
        let b1 = 2.0 * std::f32::consts::PI * (i + 1) as f32 / seg as f32;
        let p0 = vec3(r * b0.cos(), 0.0, r * b0.sin());
        let p1 = vec3(r * b1.cos(), 0.0, r * b1.sin());
        let n = (p0 - apex).cross(p1 - apex).normalize_or_zero();
        let c = tinted(base, n);
        tris.push((apex, p0, p1, c));
        tris.push((vec3(0.0, 0.0, 0.0), p1, p0, dim(base, 0.58)));
    }
    mesh_from_faces(&tris)
}

/// Cakram pendek (jangkar, koin, mata).
fn disc(base: Color, seg: u32) -> Mesh {
    let mut tris = Vec::new();
    let r = 0.5;
    let up = dim(base, 1.0);
    let down = dim(base, 0.56);
    for i in 0..seg {
        let b0 = 2.0 * std::f32::consts::PI * i as f32 / seg as f32;
        let b1 = 2.0 * std::f32::consts::PI * (i + 1) as f32 / seg as f32;
        let a = vec3(r * b0.cos(), 0.06, r * b0.sin());
        let b = vec3(r * b1.cos(), 0.06, r * b1.sin());
        let c = vec3(0.0, 0.06, 0.0);
        tris.push((c, b, a, up));
        let a2 = vec3(r * b0.cos(), -0.06, r * b0.sin());
        let b2 = vec3(r * b1.cos(), -0.06, r * b1.sin());
        tris.push((vec3(0.0, -0.06, 0.0), a2, b2, down));
        let side = dim(base, 0.78);
        tris.push((a, a2, b2, side));
        tris.push((a, b2, b, side));
    }
    mesh_from_faces(&tris)
}

/// Segitiga tipis untuk syal, daun, ekor.
fn wedge(base: Color) -> Mesh {
    mesh_from_faces(&[(
        vec3(-0.5, 0.0, 0.0),
        vec3(0.5, 0.0, 0.0),
        vec3(0.0, 0.55, 0.0),
        tinted(base, vec3(0.0, 0.0, 1.0)),
    )])
}

/// Kumpulan mesh yang dipakai ulang sepanjang game.
pub struct MeshBook {
    pub cube: Vec<Mesh>,
    pub ball: Vec<Mesh>,
    pub cone: Vec<Mesh>,
    pub disc: Vec<Mesh>,
    pub wedge: Vec<Mesh>,
    pub bright_ball: Mesh,
}

impl MeshBook {
    pub fn build() -> Self {
        let mut cube_v = Vec::new();
        let mut ball_v = Vec::new();
        let mut cone_v = Vec::new();
        let mut disc_v = Vec::new();
        let mut wedge_v = Vec::new();
        for &c in PALETTE.iter() {
            cube_v.push(cube(c));
            ball_v.push(ball(c, 8, 5));
            cone_v.push(cone(c, 6));
            disc_v.push(disc(c, 10));
            wedge_v.push(wedge(c));
        }
        MeshBook {
            cube: cube_v,
            ball: ball_v,
            cone: cone_v,
            disc: disc_v,
            wedge: wedge_v,
            bright_ball: ball(Color::new(1.0, 0.98, 0.75, 1.0), 6, 4),
        }
    }
}

/// Gambar mesh dengan transformasi. Macroquad menyalin vertex ke batch,
/// jadi mesh sementara boleh dibuang setelah dipakai.
pub fn draw_at(mesh: &Mesh, model: Mat4) {
    let mut m = Mesh {
        vertices: Vec::with_capacity(mesh.vertices.len()),
        indices: mesh.indices.clone(),
        texture: None,
    };
    for v in mesh.vertices.iter() {
        let mut v2 = *v;
        v2.position = model.transform_point3(v.position);
        m.vertices.push(v2);
    }
    draw_mesh(&m);
}

/// Model matrix dari posisi, skala, rotasi Y dan Z.
pub fn model(pos: Vec3, scale: Vec3, ry: f32, rz: f32) -> Mat4 {
    let mut m = Mat4::from_translation(pos);
    if rz.abs() > 0.0001 {
        m = m * Mat4::from_rotation_z(rz);
    }
    if ry.abs() > 0.0001 {
        m = m * Mat4::from_rotation_y(ry);
    }
    m * Mat4::from_scale(scale)
}

/// Bola dengan warna bebas (partikel, kunang).
pub fn draw_orb(pos: Vec3, r: f32, base: Color) {
    let mesh = ball(base, 6, 4);
    draw_at(&mesh, model(pos, vec3(r, r, r), 0.0, 0.0));
}

/// Balok tipis dari a ke b di ruang 3D (tali, petir, rantai).
pub fn draw_beam(a: Vec3, b: Vec3, width: f32, mesh: &Mesh) {
    let d = b - a;
    let len = d.length();
    if len < 0.0001 {
        return;
    }
    let dir = d / len;
    let up = vec3(0.0, 1.0, 0.0);
    let right = dir.cross(up).normalize_or_zero();
    let real_up = right.cross(dir).normalize_or_zero();
    // Bangun 3 sumbu, lalu pakai rotasi sederhana lewat look_at.
    let m = Mat4::from_cols(
        vec4(right.x, right.y, right.z, 0.0),
        vec4(real_up.x, real_up.y, real_up.z, 0.0),
        vec4(dir.x, dir.y, dir.z, 0.0),
        vec4((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5, 1.0),
    );
    draw_at(mesh, m * Mat4::from_scale(vec3(width, width, len)));
}
