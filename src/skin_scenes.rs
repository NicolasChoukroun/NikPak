//! Painted backgrounds for the illustrated skins.
//!
//! Most skins are colours and CSS gradients (see `skin.rs`). The skins that
//! show a scene instead - the storm, the mountains, the moon, the neon street,
//! the sushi board, the old computers, the chessboard - are drawn here, pixel
//! by pixel, into the same texture. Everything is plain software painting
//! (gradients, soft masks, blurred glow, fBm noise, a 5x7 pixel font), so it
//! works without a graphics card and looks the same in NikPak and unNik.
//!
//! The scenes are written in fractions of the image (0..1), so they keep their
//! composition in the small skin-menu tiles and in a maximised window. Detail
//! that would turn into noise at tile size is reduced with `detail_n`.

#![allow(dead_code)]

use eframe::egui::{Color32, ColorImage, Vec2};

pub(crate) type Rgb = [f32; 3];

/// How strongly lettering drawn in the artwork shows. The interface is drawn
/// on top of these backgrounds, so the artwork's own text has to stay a ghost:
/// at full strength it competes with the title, the logo and the drop ring.
const ART_TEXT: f32 = 0.30;

/// The size the scenes were designed at; small previews get less detail.
const DESIGN_AREA: f32 = 500.0 * 560.0;

/// The ids that have painted artwork.
pub(crate) fn has_scene(id: &str) -> bool {
    matches!(
        id,
        "lightning"
            | "rockies"
            | "moon"
            | "america"
            | "pattaya"
            | "burgers"
            | "sushi"
            | "amiga"
            | "msx"
            | "colecovision"
            | "zx-spectrum"
            | "catlife"
            | "doglife"
            | "mario"
            | "cs"
            | "chess"
    )
}

/// Paint the scene of `id` into an image of `w` x `h` pixels.
pub(crate) fn paint(id: &str, w: usize, h: usize) -> Option<ColorImage> {
    let scene: fn(&mut Canvas) = match id {
        "lightning" => lightning,
        "rockies" => rockies,
        "moon" => moon,
        "america" => america,
        "pattaya" => pattaya,
        "burgers" => burgers,
        "sushi" => sushi,
        "amiga" => amiga,
        "msx" => msx,
        "colecovision" => colecovision,
        "zx-spectrum" => zx_spectrum,
        "catlife" => catlife,
        "doglife" => doglife,
        "mario" => mario,
        "cs" => cs,
        "chess" => chess,
        _ => return None,
    };
    // soft artwork: a 1400 px texture stretched over a 4K window looks the
    // same, and keeps the painting (and its masks) inside a few megabytes
    let f = (1400.0 / w.max(h) as f32).min(1.0);
    let rw = ((w as f32 * f).round() as usize).max(1);
    let rh = ((h as f32 * f).round() as usize).max(1);
    let mut c = Canvas::new(rw, rh);
    scene(&mut c);
    Some(c.image())
}

// ---------------------------------------------------------------------------
// colours and maths
// ---------------------------------------------------------------------------

fn hex(s: &str) -> Rgb {
    let b = s.as_bytes();
    let mut out = [0.0f32; 3];
    let mut n = 0;
    let mut i = if !b.is_empty() && b[0] == b'#' { 1 } else { 0 };
    while i + 1 < b.len() && n < 3 {
        out[n] = (hexval(b[i]) * 16 + hexval(b[i + 1])) as f32 / 255.0;
        n += 1;
        i += 2;
    }
    out
}

fn hexval(c: u8) -> u32 {
    match c {
        b'0'..=b'9' => (c - b'0') as u32,
        b'a'..=b'f' => (c - b'a' + 10) as u32,
        b'A'..=b'F' => (c - b'A' + 10) as u32,
        _ => 0,
    }
}

fn clamp01(v: f32) -> f32 {
    if v < 0.0 {
        0.0
    } else if v > 1.0 {
        1.0
    } else {
        v
    }
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = clamp01(t);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let den = e1 - e0;
    let den = if den.abs() < 1e-9 { if den >= 0.0 { 1e-9 } else { -1e-9 } } else { den };
    let t = clamp01((x - e0) / den);
    t * t * (3.0 - 2.0 * t)
}

fn to8(v: f32) -> u8 {
    (clamp01(v) * 255.0 + 0.5) as u8
}

/// A deterministic small random number generator (xorshift64*).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) | 1)
    }

    fn u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(2685821657736338717) >> 32) as u32
    }

    fn f(&mut self) -> f32 {
        self.u32() as f32 / 4_294_967_295.0
    }

    fn normal(&mut self) -> f32 {
        let u1 = self.f().max(1e-7);
        let u2 = self.f();
        (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
    }
}

// ---------------------------------------------------------------------------
// canvas and masks
// ---------------------------------------------------------------------------

struct Canvas {
    w: usize,
    h: usize,
    px: Vec<Rgb>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        let w = w.max(1);
        let h = h.max(1);
        Canvas { w, h, px: vec![[0.0; 3]; w * h] }
    }

    fn blend(&mut self, m: &Mask, col: Rgb, k: f32) {
        for i in 0..self.px.len() {
            let a = clamp01(m.v[i] * k);
            if a > 0.0 {
                let p = &mut self.px[i];
                p[0] += (col[0] - p[0]) * a;
                p[1] += (col[1] - p[1]) * a;
                p[2] += (col[2] - p[2]) * a;
            }
        }
    }

    fn add(&mut self, m: &Mask, col: Rgb, k: f32) {
        for i in 0..self.px.len() {
            let a = m.v[i] * k;
            if a > 0.0 {
                let p = &mut self.px[i];
                p[0] = (p[0] + col[0] * a).min(1.0);
                p[1] = (p[1] + col[1] * a).min(1.0);
                p[2] = (p[2] + col[2] * a).min(1.0);
            }
        }
    }

    fn add_field(&mut self, f: &[f32], col: Rgb, k: f32) {
        for i in 0..self.px.len() {
            let a = f[i] * k;
            if a > 0.0 {
                let p = &mut self.px[i];
                p[0] = (p[0] + col[0] * a).min(1.0);
                p[1] = (p[1] + col[1] * a).min(1.0);
                p[2] = (p[2] + col[2] * a).min(1.0);
            }
        }
    }

    fn blend_rgb(&mut self, m: &Mask, cols: &[Rgb]) {
        for i in 0..self.px.len() {
            let a = clamp01(m.v[i]);
            if a > 0.0 {
                let c = cols[i];
                let p = &mut self.px[i];
                p[0] += (c[0] - p[0]) * a;
                p[1] += (c[1] - p[1]) * a;
                p[2] += (c[2] - p[2]) * a;
            }
        }
    }

    fn image(&self) -> ColorImage {
        let mut pixels = Vec::with_capacity(self.px.len());
        for p in &self.px {
            pixels.push(Color32::from_rgb(to8(p[0]), to8(p[1]), to8(p[2])));
        }
        ColorImage { size: [self.w, self.h], pixels, source_size: Vec2::new(self.w as f32, self.h as f32) }
    }
}

#[derive(Clone)]
struct Mask {
    w: usize,
    h: usize,
    v: Vec<f32>,
}

impl Mask {
    fn new(w: usize, h: usize) -> Mask {
        let w = w.max(1);
        let h = h.max(1);
        Mask { w, h, v: vec![0.0; w * h] }
    }
}

fn mask_map(a: &Mask, b: &Mask, f: impl Fn(f32, f32) -> f32) -> Mask {
    let mut m = Mask::new(a.w, a.h);
    for i in 0..m.v.len() {
        m.v[i] = clamp01(f(a.v[i], b.v[i]));
    }
    m
}

fn mask_add(a: &Mask, b: &Mask) -> Mask {
    mask_map(a, b, |x, y| x + y)
}

fn mask_sub(a: &Mask, b: &Mask) -> Mask {
    mask_map(a, b, |x, y| x - y)
}

fn mask_mul(a: &Mask, b: &Mask) -> Mask {
    mask_map(a, b, |x, y| x * y)
}

fn mask_scale(a: &Mask, k: f32) -> Mask {
    let b = Mask { w: a.w, h: a.h, v: vec![k; a.w * a.h] };
    mask_map(a, &b, |x, y| x * y)
}

/// Multiply a mask by a vertical smoothstep over the image height.
fn mask_rows(a: &Mask, e0: f32, e1: f32) -> Mask {
    let mut m = a.clone();
    for j in 0..m.h {
        let y = (j as f32 + 0.5) / m.h as f32;
        let k = smoothstep(e0, e1, y);
        for i in 0..m.w {
            m.v[j * m.w + i] *= k;
        }
    }
    m
}

/// Multiply a mask by a per-pixel field.
fn mask_field(a: &Mask, f: &[f32]) -> Mask {
    let mut m = a.clone();
    for i in 0..m.v.len() {
        m.v[i] *= f[i];
    }
    m
}

// ---------------------------------------------------------------------------
// shapes
// ---------------------------------------------------------------------------

fn edge(p: f32, a: f32, b: f32, feather: f32) -> f32 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let f = feather.max(0.5) * 0.5;
    smoothstep(lo - f, lo + f, p) * smoothstep(hi + f, hi - f, p)
}

/// A rectangle in fractions of the image; `feather` is in pixels.
fn mask_rect(c: &Canvas, x0: f32, y0: f32, x1: f32, y1: f32, feather: f32) -> Mask {
    let wf = c.w as f32;
    let hf = c.h as f32;
    let (xa, xb) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    let (ya, yb) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
    let mut m = Mask::new(c.w, c.h);
    let ix0 = ((xa * wf) - feather - 1.0).floor().max(0.0) as usize;
    let ix1 = (((xb * wf) + feather + 1.0).ceil() as usize).min(c.w);
    let iy0 = ((ya * hf) - feather - 1.0).floor().max(0.0) as usize;
    let iy1 = (((yb * hf) + feather + 1.0).ceil() as usize).min(c.h);
    if ix1 <= ix0 || iy1 <= iy0 {
        return m;
    }
    for j in iy0..iy1 {
        let py = j as f32 + 0.5;
        let ky = edge(py, ya * hf, yb * hf, feather);
        if ky <= 0.0 {
            continue;
        }
        for i in ix0..ix1 {
            let px = i as f32 + 0.5;
            m.v[j * c.w + i] = edge(px, xa * wf, xb * wf, feather) * ky;
        }
    }
    m
}

/// A disc or ellipse in fractions of the image; `feather` is in pixels.
fn mask_disc(c: &Canvas, cx: f32, cy: f32, rx: f32, ry: f32, feather: f32) -> Mask {
    let wf = c.w as f32;
    let hf = c.h as f32;
    let rx = rx.max(1e-4);
    let ry = ry.max(1e-4);
    let mut m = Mask::new(c.w, c.h);
    let e = (feather.max(0.3) / (rx.min(ry) * wf.min(hf)).max(1e-4)).min(1.5);
    let ix0 = ((cx - rx) * wf - 1.0).floor().max(0.0) as usize;
    let ix1 = (((cx + rx) * wf + 1.0).ceil() as usize).min(c.w);
    let iy0 = ((cy - ry) * hf - 1.0).floor().max(0.0) as usize;
    let iy1 = (((cy + ry) * hf + 1.0).ceil() as usize).min(c.h);
    if ix1 <= ix0 || iy1 <= iy0 {
        return m;
    }
    for j in iy0..iy1 {
        let py = j as f32 + 0.5;
        let y = py / hf;
        for i in ix0..ix1 {
            let x = (i as f32 + 0.5) / wf;
            let d = (((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2)).sqrt();
            let a = smoothstep(1.0 + e, 1.0, d);
            if a > 0.0 {
                m.v[j * c.w + i] = a;
            }
        }
    }
    m
}

/// A thick line between two points (fractions); `r` is a fraction of the
/// shorter image side, `feather` is in pixels.
fn mask_capsule(c: &Canvas, a: (f32, f32), b: (f32, f32), r: f32, feather: f32) -> Mask {
    let wf = c.w as f32;
    let hf = c.h as f32;
    let s = wf.min(hf);
    let ax = a.0 * wf;
    let ay = a.1 * hf;
    let bx = b.0 * wf;
    let by = b.1 * hf;
    let vx = bx - ax;
    let vy = by - ay;
    let l2 = (vx * vx + vy * vy).max(1e-6);
    let rp = r * s;
    let mut m = Mask::new(c.w, c.h);
    let pad = rp + feather + 2.0;
    let ix0 = ((ax.min(bx) - pad).floor().max(0.0)) as usize;
    let ix1 = (((ax.max(bx) + pad).ceil() as usize)).min(c.w);
    let iy0 = ((ay.min(by) - pad).floor().max(0.0)) as usize;
    let iy1 = (((ay.max(by) + pad).ceil() as usize)).min(c.h);
    if ix1 <= ix0 || iy1 <= iy0 {
        return m;
    }
    for j in iy0..iy1 {
        let py = j as f32 + 0.5;
        for i in ix0..ix1 {
            let px = i as f32 + 0.5;
            let t = (((px - ax) * vx + (py - ay) * vy) / l2).clamp(0.0, 1.0);
            let dx = px - (ax + vx * t);
            let dy = py - (ay + vy * t);
            let d = (dx * dx + dy * dy).sqrt();
            let a = smoothstep(rp + feather, rp - feather, d);
            if a > 0.0 {
                m.v[j * c.w + i] = a;
            }
        }
    }
    m
}

/// A filled polygon (fractions of the image); `feather` is in pixels.
fn mask_poly(c: &Canvas, pts: &[(f32, f32)], feather: f32) -> Mask {
    let wf = c.w as f32;
    let hf = c.h as f32;
    let mut m = Mask::new(c.w, c.h);
    if pts.len() < 3 {
        return m;
    }
    let p: Vec<(f32, f32)> = pts.iter().map(|q| (q.0 * wf, q.1 * hf)).collect();
    let mut ymin = hf;
    let mut ymax = 0.0f32;
    for q in &p {
        ymin = ymin.min(q.1);
        ymax = ymax.max(q.1);
    }
    let y0 = ymin.floor().max(0.0) as usize;
    let y1 = (ymax.ceil() as usize).min(c.h);
    let mut xs: Vec<f32> = Vec::with_capacity(p.len());
    for j in y0..y1 {
        let sy = j as f32 + 0.5;
        xs.clear();
        for k in 0..p.len() {
            let a = p[k];
            let b = p[(k + 1) % p.len()];
            if (a.1 <= sy && b.1 > sy) || (b.1 <= sy && a.1 > sy) {
                let t = (sy - a.1) / (b.1 - a.1);
                xs.push(a.0 + (b.0 - a.0) * t);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut k = 0;
        while k + 1 < xs.len() {
            let xa = xs[k];
            let xb = xs[k + 1];
            if xb > xa {
                let ia = xa.floor().max(0.0) as usize;
                let ib = (xb.ceil() as usize).min(c.w);
                for i in ia..ib {
                    let px = i as f32 + 0.5;
                    if px >= xa && px < xb {
                        m.v[j * c.w + i] = 1.0;
                    }
                }
            }
            k += 2;
        }
    }
    if feather > 0.4 {
        blur(&m, feather)
    } else {
        m
    }
}

// ---------------------------------------------------------------------------
// blur, gradients, noise
// ---------------------------------------------------------------------------

fn box_blur(m: &Mask, r: usize) -> Mask {
    let w = m.w;
    let h = m.h;
    let win = (2 * r + 1) as f32;
    let mut a = m.v.clone();
    let mut b = vec![0.0f32; a.len()];
    let idx = |x: isize, w: usize| -> usize {
        if x < 0 {
            0
        } else if x as usize >= w {
            w - 1
        } else {
            x as usize
        }
    };
    for _ in 0..3 {
        for y in 0..h {
            let row = y * w;
            let mut sum = 0.0;
            for k in -(r as isize)..=(r as isize) {
                sum += a[row + idx(k, w)];
            }
            for x in 0..w {
                b[row + x] = sum / win;
                sum -= a[row + idx(x as isize - r as isize, w)];
                sum += a[row + idx(x as isize + r as isize + 1, w)];
            }
        }
        for x in 0..w {
            let mut sum = 0.0;
            for k in -(r as isize)..=(r as isize) {
                sum += b[idx(k, h) * w + x];
            }
            for y in 0..h {
                a[y * w + x] = sum / win;
                sum -= b[idx(y as isize - r as isize, h) * w + x];
                sum += b[idx(y as isize + r as isize + 1, h) * w + x];
            }
        }
    }
    Mask { w, h, v: a }
}

fn blur(m: &Mask, radius: f32) -> Mask {
    if radius <= 0.0 {
        m.clone()
    } else {
        box_blur(m, radius.round().max(1.0) as usize)
    }
}

fn vgrad(c: &mut Canvas, stops: &[(f32, &str)]) {
    let mut st: Vec<(f32, Rgb)> = stops.iter().map(|s| (s.0, hex(s.1))).collect();
    st.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    for j in 0..c.h {
        let t = (j as f32 + 0.5) / c.h as f32;
        let mut col = st[0].1;
        if t <= st[0].0 {
            col = st[0].1;
        } else if t >= st[st.len() - 1].0 {
            col = st[st.len() - 1].1;
        } else {
            for k in 0..st.len() - 1 {
                if t >= st[k].0 && t <= st[k + 1].0 {
                    let f = (t - st[k].0) / (st[k + 1].0 - st[k].0).max(1e-6);
                    col = mix(st[k].1, st[k + 1].1, f);
                    break;
                }
            }
        }
        for i in 0..c.w {
            c.px[j * c.w + i] = col;
        }
    }
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add(seed.wrapping_mul(2_246_822_519));
    h ^= h >> 13;
    h = h.wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 16_777_215.0
}

/// Value noise at one octave, in 0..1.
fn value_noise(c: &Canvas, cells: u32, seed: u32) -> Vec<f32> {
    let gw = cells.max(2) as f32;
    let gh = (cells as f32 * c.h as f32 / c.w as f32).max(2.0);
    let mut out = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32 * gh;
        let yi = y.floor();
        let fy = y - yi;
        let sy = fy * fy * (3.0 - 2.0 * fy);
        let y0 = yi as i32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32 * gw;
            let xi = x.floor();
            let fx = x - xi;
            let sx = fx * fx * (3.0 - 2.0 * fx);
            let x0 = xi as i32;
            let a = hash2(x0, y0, seed);
            let b = hash2(x0 + 1, y0, seed);
            let cc = hash2(x0, y0 + 1, seed);
            let d = hash2(x0 + 1, y0 + 1, seed);
            let top = a + (b - a) * sx;
            let bot = cc + (d - cc) * sx;
            out[j * c.w + i] = top + (bot - top) * sy;
        }
    }
    out
}

/// Value noise with a separate number of cells across and down, so the
/// result can be stretched into flat sheets (storm clouds).
fn value_noise_xy(c: &Canvas, cells_x: u32, cells_y: u32, seed: u32) -> Vec<f32> {
    let gw = cells_x.max(2) as f32;
    let gh = cells_y.max(2) as f32;
    let mut out = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32 * gh;
        let yi = y.floor();
        let fy = y - yi;
        let sy = fy * fy * (3.0 - 2.0 * fy);
        let y0 = yi as i32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32 * gw;
            let xi = x.floor();
            let fx = x - xi;
            let sx = fx * fx * (3.0 - 2.0 * fx);
            let x0 = xi as i32;
            let a = hash2(x0, y0, seed);
            let b = hash2(x0 + 1, y0, seed);
            let cc = hash2(x0, y0 + 1, seed);
            let d = hash2(x0 + 1, y0 + 1, seed);
            let top = a + (b - a) * sx;
            let bot = cc + (d - cc) * sx;
            out[j * c.w + i] = top + (bot - top) * sy;
        }
    }
    out
}

/// fBm with separate cell counts across and down (see `value_noise_xy`).
fn fbm_xy(c: &Canvas, cells_x: u32, cells_y: u32, octaves: u32, seed: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; c.w * c.h];
    let mut amp = 1.0f32;
    let mut tot = 0.0f32;
    for o in 0..octaves {
        let n = value_noise_xy(c, (cells_x * (1u32 << o)).max(2), (cells_y * (1u32 << o)).max(2), seed.wrapping_add(101 * o));
        for i in 0..out.len() {
            out[i] += n[i] * amp;
        }
        tot += amp;
        amp *= 0.5;
    }
    if tot > 0.0 {
        for v in out.iter_mut() {
            *v /= tot;
        }
    }
    out
}

/// Fractional Brownian motion, normalised to 0..1.
fn fbm(c: &Canvas, cells: u32, octaves: u32, seed: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; c.w * c.h];
    let mut amp = 1.0f32;
    let mut tot = 0.0f32;
    for o in 0..octaves {
        let n = value_noise(c, (cells * (1u32 << o)).max(2), seed.wrapping_add(101 * o));
        for i in 0..out.len() {
            out[i] += n[i] * amp;
        }
        tot += amp;
        amp *= 0.5;
    }
    if tot > 0.0 {
        for v in out.iter_mut() {
            *v /= tot;
        }
    }
    out
}

/// One-dimensional fBm for ridgelines and streaks.
fn fbm1(w: usize, cells: u32, octaves: u32, seed: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; w];
    let mut amp = 1.0f32;
    let mut tot = 0.0f32;
    for o in 0..octaves {
        let cells = (cells * (1u32 << o)).max(2) as f32;
        for i in 0..w {
            let x = (i as f32 + 0.5) / w as f32 * cells;
            let xi = x.floor();
            let fx = x - xi;
            let sx = fx * fx * (3.0 - 2.0 * fx);
            let a = hash2(xi as i32, 0, seed.wrapping_add(101 * o));
            let b = hash2(xi as i32 + 1, 0, seed.wrapping_add(101 * o));
            out[i] += (a + (b - a) * sx) * amp;
        }
        tot += amp;
        amp *= 0.5;
    }
    if tot > 0.0 {
        for v in out.iter_mut() {
            *v /= tot;
        }
    }
    out
}

/// A jagged mountain profile: fBm folded into sharp peaks.
fn ridge1(w: usize, cells: u32, octaves: u32, seed: u32) -> Vec<f32> {
    let mut n = fbm1(w, cells, octaves, seed);
    let mut sorted = n.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let lo = sorted[(w * 2 / 100).min(w - 1)];
    let hi = sorted[(w * 98 / 100).min(w - 1)];
    let d = (hi - lo).max(1e-6);
    for v in n.iter_mut() {
        let t = clamp01((*v - lo) / d);
        *v = (1.0 - (2.0 * t - 1.0).abs()).powf(0.78);
    }
    n
}

/// Vertical vignette.
fn vignette(c: &mut Canvas, strength: f32) {
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let r = (((x - 0.5) * 1.35).powi(2) + ((y - 0.5) * 1.1).powi(2)).sqrt() / 0.75;
            let k = 1.0 - strength * r.min(1.0).powi(2);
            let p = &mut c.px[j * c.w + i];
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
        }
    }
}

/// A little film grain, so the flat gradients do not band.
fn grain(c: &mut Canvas, amount: f32) {
    for j in 0..c.h {
        for i in 0..c.w {
            let h = hash2(i as i32, j as i32, 4242) * 2.0 - 1.0;
            let p = &mut c.px[j * c.w + i];
            p[0] = clamp01(p[0] + h * amount);
            p[1] = clamp01(p[1] + h * amount);
            p[2] = clamp01(p[2] + h * amount);
        }
    }
}

// ---------------------------------------------------------------------------
// the 5x7 pixel font used by the computer and neon skins
// ---------------------------------------------------------------------------

fn glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        ' ' => [0; 7],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
        ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0, 0],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '>' => [0b01000, 0b00100, 0b00010, 0b00001, 0b00010, 0b00100, 0b01000],
        '<' => [0b00010, 0b00100, 0b01000, 0b10000, 0b01000, 0b00100, 0b00010],
        '?' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '(' => [0b00010, 0b00100, 0b01000, 0b01000, 0b01000, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00010, 0b00100, 0b01000],
        '*' => [0, 0b10101, 0b01110, 0b11111, 0b01110, 0b10101, 0],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0, 0],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0],
        ',' => [0, 0, 0, 0, 0b01100, 0b01100, 0b01000],
        '\'' => [0b00100, 0b00100, 0, 0, 0, 0, 0],
        '"' => [0b01010, 0b01010, 0, 0, 0, 0, 0],
        _ => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0, 0b00100],
    }
}

/// Pixel text; `px` is the size of one font pixel as a fraction of height.
fn text_mask(c: &Canvas, s: &str, x: f32, y: f32, px: f32) -> Mask {
    let u = (px * c.h as f32).max(0.05);
    let mut m = Mask::new(c.w, c.h);
    let mut cx = x * c.w as f32;
    let cy = y * c.h as f32;
    for ch in s.chars() {
        let g = glyph(ch);
        for (gy, row) in g.iter().enumerate() {
            for gx in 0..5 {
                if *row & (1u8 << (4 - gx)) == 0 {
                    continue;
                }
                let x0 = cx + gx as f32 * u;
                let y0 = cy + gy as f32 * u;
                let x1 = x0 + u;
                let y1 = y0 + u;
                let ia = x0.floor().max(0.0) as usize;
                let ib = (x1.ceil() as usize).min(c.w);
                let ja = y0.floor().max(0.0) as usize;
                let jb = (y1.ceil() as usize).min(c.h);
                for j in ja..jb {
                    for i in ia..ib {
                        let p = (i as f32 + 0.5, j as f32 + 0.5);
                        let a = edge(p.0, x0, x1, 0.6) * edge(p.1, y0, y1, 0.6);
                        if a > m.v[j * c.w + i] {
                            m.v[j * c.w + i] = a;
                        }
                    }
                }
            }
        }
        cx += 6.0 * u;
    }
    m
}

fn neon_text(c: &mut Canvas, s: &str, x: f32, y: f32, px: f32, col: Rgb, glow: f32, strength: f32) {
    let m = text_mask(c, s, x, y, px);
    c.add(&blur(&m, glow), col, strength);
    c.add(&blur(&m, glow * 0.35), col, strength * 0.8);
    let core = mix(col, hex("#FFFFFF"), 0.55);
    c.blend(&blur(&m, 0.8), core, 1.0);
}

// ---------------------------------------------------------------------------
// shared scene pieces
// ---------------------------------------------------------------------------

fn merge_max(dst: &mut Mask, src: &Mask) {
    for i in 0..dst.v.len() {
        if src.v[i] > dst.v[i] {
            dst.v[i] = src.v[i];
        }
    }
}

fn detail_n(c: &Canvas, n: usize, floor: usize) -> usize {
    let a = (c.w * c.h) as f32 / DESIGN_AREA;
    ((n as f32 * a).round() as usize).max(floor).min(n)
}

fn halo(c: &mut Canvas, cx: f32, cy: f32, r: f32, col: Rgb, strength: f32, spread: f32) {
    let m = mask_disc(c, cx, cy, r, r, 1.0);
    let px = (r * c.w.min(c.h) as f32 * 0.45 * spread).max(2.0);
    c.add(&blur(&m, px), col, strength);
}

fn stars(c: &mut Canvas, n: usize, seed: u64, bright: f32, max_r: f32) {
    let mut rng = Rng::new(seed);
    let count = detail_n(c, n, 8);
    for _ in 0..count {
        let x = rng.f();
        let y = rng.f().powf(1.4) * 0.85;
        let r = (0.3 + rng.f().powf(3.0)) * max_r;
        let b = (0.25 + rng.f().powi(2)) * bright;
        let m = mask_disc(c, x, y, r, r, 0.9);
        c.add(&m, hex("#FFFFFF"), b);
        if rng.f() < 0.25 {
            let m2 = mask_disc(c, x, y, r * 3.0, r * 3.0, 2.0);
            c.add(&blur(&m2, 2.0), hex("#BFD8FF"), b * 0.5);
        }
    }
}

struct Cloud {
    seed: u64,
    y0: f32,
    y1: f32,
    cells: u32,
    cover: f32,
    col: &'static str,
    hi: &'static str,
    /// colour, x, y, strength
    light: Option<(&'static str, f32, f32, f32)>,
}

fn cloud_field(c: &mut Canvas, k: &Cloud) {
    let n = fbm(c, k.cells, 7, k.seed as u32);
    let shade = fbm(c, (k.cells * 2).max(2), 5, k.seed as u32 + 9);
    let band_top = k.y0;
    let band_bottom = k.y1;
    let col = hex(k.col);
    let hi = hex(k.hi);
    let mut m = Mask::new(c.w, c.h);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let band = smoothstep(band_top - 0.10, band_top + 0.14, y) * smoothstep(band_bottom + 0.20, band_bottom - 0.16, y);
        if band <= 0.0 {
            continue;
        }
        for i in 0..c.w {
            let idx = j * c.w + i;
            let a = smoothstep(k.cover - 0.17, k.cover + 0.23, n[idx]) * band;
            if a <= 0.0 {
                continue;
            }
            let s = (shade[idx] * 0.9 + (1.0 - (y / band_bottom.max(0.2)).clamp(0.0, 1.0)) * 0.35).clamp(0.0, 1.0);
            let cc = mix(col, hi, s.powf(1.6));
            let p = &mut c.px[idx];
            let w = a * 0.94;
            p[0] += (cc[0] - p[0]) * w;
            p[1] += (cc[1] - p[1]) * w;
            p[2] += (cc[2] - p[2]) * w;
            m.v[idx] = w;
        }
    }
    if let Some((lc, lx, ly, ls)) = k.light {
        let lcol = hex(lc);
        let mut f = vec![0.0f32; c.w * c.h];
        for j in 0..c.h {
            let y = (j as f32 + 0.5) / c.h as f32;
            for i in 0..c.w {
                let x = (i as f32 + 0.5) / c.w as f32;
                let d = ((x - lx).powi(2) + (y - ly).powi(2)).sqrt();
                f[j * c.w + i] = (-(d / 0.36).powi(2)).exp() * ls;
            }
        }
        let lit = mask_field(&m, &f);
        c.add(&lit, lcol, 1.0);
    }
}

/// A storm cloud deck: `cloud_field` with a stretched, flatter structure, a
/// workable edge and a chosen amount of light on the tops.
#[allow(clippy::too_many_arguments)]
fn storm_cloud(c: &mut Canvas, seed: u64, y0: f32, y1: f32, cx: u32, cy: u32, cover: f32, col: &str, hi: &str, soft: f32, lift: f32, edge: f32, light: Option<(&str, f32, f32, f32, f32)>) {
    let n = fbm_xy(c, cx, cy, 6, seed as u32);
    let shade = fbm_xy(c, (cx * 2).max(2), (cy * 3).max(2), 4, seed as u32 + 9);
    let col = hex(col);
    let hi = hex(hi);
    let mut m = Mask::new(c.w, c.h);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let band = smoothstep(y0 - 0.12, y0 + 0.16, y) * smoothstep(y1 + 0.24, y1 - 0.18, y);
        if band <= 0.0 {
            continue;
        }
        for i in 0..c.w {
            let idx = j * c.w + i;
            let a = smoothstep(cover - edge * 0.5, cover + edge, n[idx]) * band;
            if a <= 0.0 {
                continue;
            }
            let s = (shade[idx] * 0.85 + (1.0 - (y / y1.max(0.2)).clamp(0.0, 1.0)) * lift).clamp(0.0, 1.0);
            let cc = mix(col, hi, s.powf(1.55));
            let p = &mut c.px[idx];
            let w = a * soft;
            p[0] += (cc[0] - p[0]) * w;
            p[1] += (cc[1] - p[1]) * w;
            p[2] += (cc[2] - p[2]) * w;
            m.v[idx] = w;
        }
    }
    if let Some((lc, lx, ly, ls, lr)) = light {
        let lcol = hex(lc);
        let mut f = vec![0.0f32; c.w * c.h];
        for j in 0..c.h {
            let y = (j as f32 + 0.5) / c.h as f32;
            for i in 0..c.w {
                let x = (i as f32 + 0.5) / c.w as f32;
                let d = ((x - lx).powi(2) + ((y - ly) * 1.35).powi(2)).sqrt();
                f[j * c.w + i] = (-(d / lr).powi(2)).exp() * ls;
            }
        }
        let lit = mask_field(&m, &f);
        c.add(&lit, lcol, 1.0);
    }
}

/// A vertical haze band, soft at both ends: `col`, from `amt0` at `y0` to
/// `amt1` at `y1`.
fn grad_band(c: &mut Canvas, y0: f32, y1: f32, col: &str, amt0: f32, amt1: f32, feather: f32) {
    let col = hex(col);
    let mut f = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let t = ((y - y0) / (y1 - y0).max(1e-6)).clamp(0.0, 1.0);
        let a = (amt0 + (amt1 - amt0) * t) * smoothstep(y0 - feather, y0 + feather, y) * smoothstep(y1 + feather * 1.6, y1 - feather, y);
        for i in 0..c.w {
            f[j * c.w + i] = a;
        }
    }
    c.blend(&Mask { w: c.w, h: c.h, v: f }, col, 1.0);
}

/// Sunlight catching the flanks of a ridge that face the sun: a soft band
/// under the skyline, brightest near the crest.
fn ridge_rim(c: &mut Canvas, ridge: &[f32], light_x: f32, col: &str, k: f32, fall: f32, line: f32, glow: f32) {
    if ridge.len() < 2 {
        return;
    }
    let w = c.w;
    let mut ry = vec![0.0f32; w];
    for i in 0..w {
        let pos = if w > 1 { i as f32 / (w - 1) as f32 } else { 0.0 } * (ridge.len() - 1) as f32;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(ridge.len() - 1);
        let f = pos - i0 as f32;
        ry[i] = ridge[i0] + (ridge[i1] - ridge[i0]) * f;
    }
    let mut facing = vec![0.0f32; w];
    for i in 0..w {
        let a = ry[if i == 0 { 0 } else { i - 1 }];
        let b = ry[(i + 1).min(w - 1)];
        let slope = ((b - a) * 0.5 * w as f32 * 0.02).clamp(-1.5, 1.5);
        let side = if light_x > 0.5 { 1.0 } else { -1.0 };
        facing[i] = (slope * side * 1.8).clamp(0.0, 1.0).powf(0.8);
    }
    let col = hex(col);
    let mut f = vec![0.0f32; c.w * c.h];
    let mut sharp = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..w {
            let dy = y - ry[i];
            if dy <= -0.003 {
                continue;
            }
            let prof = (-dy.max(0.0) / fall).exp() * facing[i];
            f[j * w + i] = prof;
            sharp[j * w + i] = (-dy.max(0.0) / line).exp() * facing[i];
        }
    }
    c.add_field(&f, col, k);
    if glow > 0.0 {
        let b = blur(&Mask { w, h: c.h, v: f }, 6.0);
        let mut lit = vec![0.0f32; c.w * c.h];
        for i in 0..lit.len() {
            lit[i] = b.v[i] * 0.8 + sharp[i] * 0.2 * facing[i % w];
        }
        c.add_field(&lit, col, glow);
    }
}

fn pines(c: &mut Canvas, yline: &[f32], n: usize, seed: u64, col: Rgb, size: f32) {
    let mut rng = Rng::new(seed);
    let count = detail_n(c, n, 6);
    if yline.len() < 2 {
        return;
    }
    for _ in 0..count {
        let x = rng.f();
        let i = ((x * (yline.len() - 1) as f32) as usize).min(yline.len() - 1);
        let y = yline[i];
        let s = size * (0.6 + rng.f() * 0.7);
        let h = s * 2.2;
        c.blend(&mask_rect(c, x - s * 0.05, y - h * 0.15, x + s * 0.05, y + h * 0.25, 0.5), col, 1.0);
        for k in 0..3 {
            let yy = y - h * (0.1 + 0.28 * k as f32);
            let w0 = s * (1.0 - 0.22 * k as f32);
            let tri = [(x - w0, yy + s * 0.55), (x + w0, yy + s * 0.55), (x, yy - s * 0.45)];
            c.blend(&mask_poly(c, &tri, 0.35), col, 1.0);
        }
    }
}

struct Terrain<'a> {
    ridge: &'a [f32],
    dark: &'a str,
    light: &'a str,
    haze: Option<&'a str>,
    haze_amt: f32,
    snowline: Option<f32>,
    snow: &'a str,
    snow_shade: &'a str,
    snow_cov: f32,
    seed: u64,
    rough: f32,
    snow_depth: f32,
}

impl<'a> Terrain<'a> {
    fn new(ridge: &'a [f32], dark: &'a str, light: &'a str) -> Terrain<'a> {
        Terrain {
            ridge,
            dark,
            light,
            haze: None,
            haze_amt: 0.0,
            snowline: None,
            snow: "#F2F7FC",
            snow_shade: "#C2D4E6",
            snow_cov: 0.4,
            seed: 0,
            rough: 0.35,
            snow_depth: 0.12,
        }
    }
}

fn terrain(c: &mut Canvas, t: &Terrain) {
    let w = c.w;
    let h = c.h;
    if t.ridge.len() < 2 {
        return;
    }
    let mut ry = vec![0.0f32; w];
    for i in 0..w {
        let pos = if w > 1 { i as f32 / (w - 1) as f32 } else { 0.0 } * (t.ridge.len() - 1) as f32;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(t.ridge.len() - 1);
        let f = pos - i0 as f32;
        ry[i] = t.ridge[i0] + (t.ridge[i1] - t.ridge[i0]) * f;
    }
    let mut slope = vec![0.0f32; w];
    for i in 0..w {
        let a = ry[if i == 0 { 0 } else { i - 1 }];
        let b = ry[(i + 1).min(w - 1)];
        slope[i] = ((b - a) * 0.5 * w as f32 * 0.02).clamp(-1.5, 1.5);
    }
    let n = fbm(c, 6, 5, t.seed as u32);
    let n2 = fbm(c, 16, 4, t.seed as u32 + 5);
    let dark = hex(t.dark);
    let light = hex(t.light);
    let haze = t.haze.map(hex);
    let mut mask = Mask::new(w, h);
    let mut cols = vec![[0.0f32; 3]; w * h];
    let mut shadev = vec![0.0f32; w * h];
    let mut depthv = vec![0.0f32; w * h];
    for j in 0..h {
        let y = (j as f32 + 0.5) / h as f32;
        for i in 0..w {
            let idx = j * w + i;
            let inside = smoothstep(-0.0016, 0.0016, y - ry[i]);
            if inside <= 0.0 {
                continue;
            }
            let depth = ((y - ry[i]) / 0.35).clamp(0.0, 1.0);
            let mut shade = 0.52 - slope[i] * 0.30 + (n[idx] - 0.5) * t.rough + (n2[idx] - 0.5) * t.rough * 0.6 - depth * 0.30;
            shade = clamp01(shade);
            let mut col = mix(dark, light, shade);
            let k = 0.86 + 0.22 * (1.0 - depth);
            col = [col[0] * k, col[1] * k, col[2] * k];
            if let Some(hz) = haze {
                let a = clamp01(t.haze_amt * (0.55 + 0.45 * (1.0 - depth)));
                col = mix(col, hz, a);
            }
            cols[idx] = col;
            mask.v[idx] = inside;
            shadev[idx] = shade;
            depthv[idx] = depth;
        }
    }
    c.blend_rgb(&mask, &cols);
    if let Some(snowline) = t.snowline {
        let jitter = fbm(c, 9, 4, t.seed as u32 + 11);
        let patch = fbm(c, 13, 4, t.seed as u32 + 23);
        let snow_col = hex(t.snow);
        let snow_shade = hex(t.snow_shade);
        let mut smask = Mask::new(w, h);
        let mut scols = vec![[0.0f32; 3]; w * h];
        let mut lit = vec![0.0f32; w * h];
        for j in 0..h {
            let y = (j as f32 + 0.5) / h as f32;
            for i in 0..w {
                let idx = j * w + i;
                if mask.v[idx] <= 0.0 {
                    continue;
                }
                let sj = (jitter[idx] - 0.5) * 0.045;
                let mut sm = smoothstep(0.0, 0.45, (snowline + sj - y) / 0.05) * mask.v[idx];
                let steep = smoothstep(0.40, 0.95, slope[i].abs() + n2[idx] * 0.30);
                sm *= 1.0 - steep * (1.0 - t.snow_cov);
                sm *= 0.55 + 0.60 * smoothstep(0.35, 0.62, patch[idx]);
                // the lower edge of a snow cap is ragged, not a straight line
                let depth = t.snow_depth * (0.45 + 1.1 * patch[idx]);
                sm *= smoothstep(depth, 0.0, y - ry[i]);
                sm = clamp01(sm);
                if sm <= 0.0 {
                    continue;
                }
                let sc = mix(snow_shade, snow_col, clamp01(shadev[idx].powf(1.4) * 0.95 + 0.08));
                let f = 0.82 + 0.28 * clamp01(1.0 - depthv[idx] * 1.4);
                scols[idx] = [sc[0] * f, sc[1] * f, sc[2] * f];
                smask.v[idx] = sm;
                lit[idx] = slope[i].max(0.0) * 0.9 * sm * (1.0 - depthv[idx]);
            }
        }
        c.blend_rgb(&smask, &scols);
        c.add_field(&lit, hex("#FFD9A8"), 0.18);
    }
}

fn bolt_path(seed: u64, start: (f32, f32), end: (f32, f32), steps: usize, amp: f32, decay: f32) -> Vec<(f32, f32)> {
    let mut rng = Rng::new(seed);
    let ax = start.0;
    let ay = start.1;
    let dx = end.0 - ax;
    let dy = end.1 - ay;
    let plen = (-dy * -dy + dx * dx).sqrt().max(1e-6);
    let px = -dy / plen;
    let py = dx / plen;
    let mut off = 0.0f32;
    let mut pts = vec![start];
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        off = off * decay + rng.normal() * amp * (0.45 + 0.55 * (t * std::f32::consts::PI).sin());
        pts.push((ax + dx * t + px * off, ay + dy * t + py * off));
    }
    let mut fine = vec![pts[0]];
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mx = (a.0 + b.0) * 0.5;
        let my = (a.1 + b.1) * 0.5;
        let ex = -(b.1 - a.1);
        let ey = b.0 - a.0;
        let l = (ex * ex + ey * ey).sqrt().max(1e-6);
        let off2 = rng.normal() * 0.008;
        fine.push((mx + ex / l * off2, my + ey / l * off2));
        fine.push(b);
    }
    fine
}

fn stroke_bolt(c: &Canvas, path: &[(f32, f32)], w0: f32, taper: f32, core: &mut Mask, gl: &mut Mask) {
    let n = path.len().max(2);
    for j in 0..path.len().saturating_sub(1) {
        let t = j as f32 / (n - 1) as f32;
        let w = (w0 * (1.0 - taper * t)).max(0.0005);
        let a = path[j];
        let b = path[j + 1];
        merge_max(core, &mask_capsule(c, a, b, w, 0.72));
        merge_max(gl, &mask_capsule(c, a, b, w * 2.2, 0.9));
    }
}

fn draw_bolt(c: &mut Canvas, pts: &[(f32, f32)], width: f32, seed: u64, taper: f32, branches: usize, dim: f32) {
    let mut rng = Rng::new(seed);
    let n = pts.len();
    let mut trunk = Mask::new(c.w, c.h);
    let mut glowm = Mask::new(c.w, c.h);
    let mut br_core = Mask::new(c.w, c.h);
    let mut br_glow = Mask::new(c.w, c.h);
    stroke_bolt(c, pts, width, taper, &mut trunk, &mut glowm);
    for k in 0..branches {
        let i = ((n as f32 * (0.18 + 0.15 * k as f32 + rng.f() * 0.05)) as usize).min(n.saturating_sub(6));
        let a = pts[i];
        let sign = if k % 2 == 0 { -1.0 } else { 1.0 };
        let ang = std::f32::consts::FRAC_PI_2 + sign * (0.50 + rng.f() * 0.30);
        let ln = 0.05 + rng.f() * 0.07;
        let b = (a.0 + ang.cos() * ln, a.1 + ang.sin() * ln);
        let bp = bolt_path(seed.wrapping_add(k as u64 * 17 + 1), a, b, 4, 0.035, 0.55);
        stroke_bolt(c, &bp, width * 0.30, taper, &mut br_core, &mut br_glow);
    }
    c.blend(&trunk, hex("#F2F8FF"), 1.0);
    c.blend(&br_core, hex("#F2F8FF"), 0.8 * dim);
    c.add(&blur(&glowm, 22.0), hex("#7FB4FF"), 0.30 * dim);
    c.add(&blur(&br_glow, 9.0), hex("#7FB4FF"), 0.20 * dim);
    c.add(&blur(&glowm, 8.0), hex("#7FB4FF"), 0.55 * dim);
    c.add(&blur(&glowm, 2.4), hex("#DCEBFF"), 0.55 * dim);
    c.add(&blur(&trunk, 1.2), hex("#FFFFFF"), 0.35 * dim);
}

/// Lightning as long straight runs with sharp kinks: no subdivision between
/// the segments, so it reads as a real channel and not as a wiggly line.
fn bolt_crisp(seed: u64, start: (f32, f32), end: (f32, f32), steps: usize, amp: f32, decay: f32) -> Vec<(f32, f32)> {
    let mut rng = Rng::new(seed);
    let ax = start.0;
    let ay = start.1;
    let dx = end.0 - ax;
    let dy = end.1 - ay;
    let plen = (dx * dx + dy * dy).sqrt().max(1e-6);
    let px = -dy / plen;
    let py = dx / plen;
    let mut off = 0.0f32;
    let mut pts = vec![start];
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        // kinks come in bursts, so some runs stay long and straight
        let k = if rng.f() < 0.55 { 1.0 } else { 0.15 };
        off = off * decay + rng.normal() * amp * k * (0.45 + 0.55 * (t * std::f32::consts::PI).sin());
        pts.push((ax + dx * t + px * off, ay + dy * t + py * off));
    }
    pts
}

/// A bolt at full quality: white core, glow falling off in rings, and forks
/// at fixed stations that always head down and away from the trunk.
fn bolt_layers(c: &mut Canvas, pts: &[(f32, f32)], width: f32, seed: u64, taper: f32, branches: usize, dim: f32) {
    let mut trunk = Mask::new(c.w, c.h);
    let mut glow1 = Mask::new(c.w, c.h);
    let mut glow2 = Mask::new(c.w, c.h);
    let mut br = Mask::new(c.w, c.h);
    stroke_bolt(c, pts, width, taper, &mut trunk, &mut glow1);
    let n = pts.len();
    // (fraction along the trunk, side, length)
    let stations = [(0.24f32, -1.0f32, 0.085f32), (0.42, 1.0, 0.070), (0.58, -1.0, 0.055), (0.74, 1.0, 0.045)];
    for (k, &(t, side, ln)) in stations.iter().enumerate().take(branches.min(stations.len())) {
        if n < 3 {
            break;
        }
        let i = ((t * (n - 1) as f32) as usize).clamp(1, n - 2);
        let a0 = pts[i];
        let ang = std::f32::consts::FRAC_PI_2 + side * (0.52 + 0.10 * (k % 2) as f32);
        // the fork starts off the trunk, not on its centre
        let a = (a0.0 + ang.cos() * width * 0.6, a0.1 + ang.sin() * width * 0.6);
        let b = (a.0 + ang.cos() * ln, a.1 + ang.sin() * ln);
        let bp = bolt_crisp(seed.wrapping_add(k as u64 * 17 + 1), a, b, 4, 0.010, 0.45);
        stroke_bolt(c, &bp, width * 0.62, 0.88, &mut br, &mut glow2);
    }
    c.add(&blur(&trunk, 54.0), hex("#2E4C8E"), 0.13 * dim);
    c.add(&blur(&glow1, 26.0), hex("#5A86D6"), 0.20 * dim);
    c.add(&blur(&glow1, 10.0), hex("#9CBEF8"), 0.34 * dim);
    c.add(&blur(&glow1, 3.4), hex("#DCE9FF"), 0.60 * dim);
    // the forks: tapered glow with a faint thread inside, not a hard pen line
    c.add(&blur(&br, 8.0), hex("#7FA6E8"), 0.24 * dim);
    c.add(&blur(&br, 2.2), hex("#C6DAFF"), 0.50 * dim);
    c.add(&blur(&br, 0.8), hex("#EFF5FF"), 0.70 * dim);
    c.blend(&trunk, hex("#FFFFFF"), 0.94);
    c.add(&blur(&trunk, 2.2), hex("#DCE9FF"), 0.30 * dim);
}

fn rain(c: &mut Canvas, n: usize, seed: u64, alpha: f32) {
    let mut rng = Rng::new(seed);
    let mut m = Mask::new(c.w, c.h);
    let count = detail_n(c, n, 20);
    for _ in 0..count {
        let x = rng.f();
        let y = rng.f();
        let ln = 0.02 + rng.f() * 0.04;
        merge_max(&mut m, &mask_capsule(c, (x, y), (x - 0.12 * ln, y + ln), 0.0007, 0.7));
    }
    c.add(&m, hex("#9FB6D8"), alpha);
}

fn ridge_poly(c: &mut Canvas, yline: &[f32], top: &str, bottom: &str) {
    if yline.len() < 2 {
        return;
    }
    let mut pts = Vec::with_capacity(yline.len() + 2);
    pts.push((0.0, 1.0));
    for (i, y) in yline.iter().enumerate() {
        pts.push((i as f32 / (yline.len() - 1) as f32, *y));
    }
    pts.push((1.0, 1.0));
    let m = mask_poly(c, &pts, 0.4);
    let a = hex(top);
    let b = hex(bottom);
    let mut cols = vec![[0.0f32; 3]; c.w * c.h];
    for j in 0..c.h {
        let t = (j as f32 + 0.5) / c.h as f32;
        let col = mix(a, b, t);
        for i in 0..c.w {
            cols[j * c.w + i] = col;
        }
    }
    c.blend_rgb(&m, &cols);
}

// ---------------------------------------------------------------------------
// the night sky
// ---------------------------------------------------------------------------

fn moon(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#080D24"), (0.35, "#0E1738"), (0.70, "#141D42"), (1.0, "#070A18")]);
    stars(c, 240, 11, 1.0, 0.0018);
    let (cx, cy, r) = (0.68f32, 0.27f32, 0.150f32);
    halo(c, cx, cy, r * 2.4, hex("#93A8E8"), 0.10, 1.0);
    halo(c, cx, cy, r * 1.55, hex("#B6C6FF"), 0.10, 1.0);
    let disc = mask_disc(c, cx, cy, r, r, 0.7);
    let tex = fbm(c, 9, 4, 21);
    let mut cols = vec![[0.0f32; 3]; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let dx = (x - cx) / r;
            let dy = (y - cy) / r;
            let light = clamp01(0.55 - 0.45 * (dx * 0.45 + dy * 0.55));
            let mut base = mix(hex("#F6F2E2"), hex("#D9D4C2"), light);
            let k = 0.94 + 0.10 * tex[j * c.w + i];
            base = [base[0] * k, base[1] * k, base[2] * k];
            cols[j * c.w + i] = base;
        }
    }
    c.blend_rgb(&disc, &cols);
    // the dark seas
    let mut f = fbm(c, 4, 4, 33);
    for v in f.iter_mut() {
        *v = smoothstep(0.52, 0.78, *v);
    }
    let mar = mask_field(&disc, &f);
    c.blend(&mar, hex("#A8A695"), 0.22);
    let mut f2 = fbm(c, 5, 4, 77);
    for v in f2.iter_mut() {
        *v = smoothstep(0.58, 0.82, *v);
    }
    let mar2 = mask_field(&disc, &f2);
    c.blend(&mar2, hex("#B4B1A0"), 0.14);
    // craters
    let mut rng = Rng::new(5);
    for _ in 0..16 {
        let a = rng.f() * std::f32::consts::TAU;
        let rr = rng.f().powf(0.5) * r * 0.82;
        let px = cx + a.cos() * rr;
        let py = cy + a.sin() * rr * 0.95;
        let cr = r * (0.025 + rng.f().powf(1.8) * 0.07);
        let inside = mask_mul(&mask_disc(c, px, py, cr, cr, 1.8), &disc);
        c.blend(&inside, hex("#9E9B8C"), 0.16);
        let rim = mask_sub(&mask_disc(c, px, py, cr * 1.30, cr * 1.30, 1.6), &mask_disc(c, px, py, cr * 0.85, cr * 0.85, 1.6));
        let rim = mask_scale(&mask_mul(&rim, &disc), 0.7);
        c.add(&rim, hex("#FCF8E9"), 0.10);
    }
    cloud_field(
        c,
        &Cloud {
            seed: 31,
            y0: 0.34,
            y1: 0.46,
            cells: 7,
            cover: 0.58,
            col: "#2A3458",
            hi: "#4C5B8C",
            light: Some(("#D3DEFF", cx, cy, 0.30)),
        },
    );
    cloud_field(
        c,
        &Cloud {
            seed: 32,
            y0: 0.66,
            y1: 0.90,
            cells: 3,
            cover: 0.50,
            col: "#131A34",
            hi: "#2D3860",
            light: Some(("#A8BCF0", cx, cy, 0.10)),
        },
    );
    vignette(c, 0.32);
    grain(c, 0.012);
}

fn lightning(c: &mut Canvas) {
    // a night storm: the sky is nearly black, the cloud decks are the
    // picture, and the light only comes from the flash itself
    vgrad(c, &[(0.0, "#04070E"), (0.16, "#070C17"), (0.42, "#0A111F"), (0.72, "#05080F"), (1.0, "#010204")]);
    // high, big cloud masses at the top of the frame
    storm_cloud(c, 31, -0.30, 0.22, 2, 5, 0.42, "#060A13", "#313D5E", 0.96, 0.50, 0.13, None);
    // the storm cell the bolt comes out of
    storm_cloud(c, 32, 0.02, 0.44, 3, 8, 0.47, "#05080F", "#232D48", 0.96, 0.45, 0.13, Some(("#A8C4FF", 0.655, 0.09, 0.85, 0.30)));
    // low scud, lit from above by the flash
    storm_cloud(c, 33, 0.36, 0.70, 2, 7, 0.58, "#04060B", "#131B2E", 0.93, 0.30, 0.15, Some(("#6C90D8", 0.60, 0.30, 0.34, 0.32)));
    // a dark far hill, so it reads as a place and not just a sky
    let mut yfar = fbm1(c.w, 2, 3, 5);
    for y in yfar.iter_mut() {
        *y = 0.955 - *y * 0.020;
    }
    ridge_poly(c, &yfar, "#04070C", "#010204");

    // the main channel: long straight runs, sharp kinks
    let pts = bolt_crisp(23, (0.715, -0.06), (0.470, 0.945), 9, 0.085, 0.22);
    bolt_layers(c, &pts, 0.00220, 23, 0.28, 4, 1.0);
    // a second, faint bolt far behind it, for depth
    let far_pts = bolt_crisp(79, (0.245, 0.02), (0.285, 0.29), 4, 0.030, 0.25);
    bolt_layers(c, &far_pts, 0.0006, 79, 0.5, 0, 0.11);
    // the flash on the cloud deck, tight around the bolt: three gradients
    // falling off at different rates, not one flat blob
    let top = pts[(pts.len() as f32 * 0.12) as usize];
    let (tx, ty) = (top.0, top.1.max(0.04));
    c.add(&blur(&mask_disc(c, tx, ty, 0.30, 0.165, 1.0), 42.0), hex("#3E5C9E"), 0.055);
    c.add(&blur(&mask_disc(c, tx, ty, 0.16, 0.088, 1.0), 42.0), hex("#6E92D8"), 0.07);
    c.add(&blur(&mask_disc(c, tx, ty, 0.075, 0.041, 1.0), 42.0), hex("#B9CFF8"), 0.10);
    rain(c, 420, 9, 0.045);
    // the rain catches the light near the bolt
    c.add(&blur(&mask_disc(c, 0.60, 0.42, 0.22, 0.30, 1.0), 26.0), hex("#5C7CC0"), 0.05);
    vignette(c, 0.50);
    grain(c, 0.008);
}

fn rockies(c: &mut Canvas) {
    // --- sky: deep blue zenith down to a warm band at the horizon ---------
    vgrad(
        c,
        &[
            (0.0, "#03060D"),
            (0.14, "#061324"),
            (0.30, "#0A2539"),
            (0.44, "#194256"),
            (0.545, "#2E4C5C"),
            (0.600, "#584634"),
            (0.632, "#74513A"),
            (0.660, "#402D20"),
            (0.70, "#21160F"),
            (0.80, "#100A06"),
            (1.0, "#060302"),
        ],
    );
    stars(c, 40, 41, 0.14, 0.0014);
    // --- the sun, setting in a col of the far range -----------------------
    let (sx, sy) = (0.720f32, 0.628f32);
    c.add(&blur(&mask_disc(c, sx, sy, 0.34, 0.17, 1.0), 48.0), hex("#5E7286"), 0.07);
    c.add(&blur(&mask_disc(c, sx, sy, 0.17, 0.11, 1.0), 26.0), hex("#C08050"), 0.15);
    c.add(&blur(&mask_disc(c, sx, sy, 0.085, 0.062, 1.0), 12.0), hex("#F0A862"), 0.28);
    c.add(&blur(&mask_disc(c, sx, sy, 0.042, 0.038, 1.2), 5.0), hex("#FFDCA6"), 0.45);
    // a flat flare across the horizon, and the disc itself
    c.add(&blur(&mask_disc(c, sx, sy, 0.26, 0.007, 1.0), 6.0), hex("#FFC98A"), 0.08);
    c.blend(&mask_disc(c, sx, sy, 0.017, 0.019, 1.4), hex("#FFEFCD"), 0.95);
    // --- thin clouds over the horizon, lit from below ---------------------
    for &(seed, y0, y1, cover, amt) in [(71u64, 0.44f32, 0.50f32, 0.55f32, 0.26f32), (72, 0.52, 0.58, 0.58, 0.30)].iter() {
        let n = fbm_xy(c, 2, 10, 5, seed as u32);
        let mut f = vec![0.0f32; c.w * c.h];
        let mut cols = vec![[0.0f32; 3]; c.w * c.h];
        for j in 0..c.h {
            let y = (j as f32 + 0.5) / c.h as f32;
            let band = smoothstep(y0 - 0.045, y0 + 0.015, y) * smoothstep(y1 + 0.045, y1 - 0.015, y);
            if band <= 0.0 {
                continue;
            }
            let warm = mix(hex("#7C8FA0"), hex("#F2BE8E"), clamp01(1.0 - (y - sy).abs() / 0.13));
            for i in 0..c.w {
                let idx = j * c.w + i;
                let a = smoothstep(cover - 0.10, cover + 0.14, n[idx]) * band * amt;
                f[idx] = a;
                cols[idx] = warm;
            }
        }
        c.blend_rgb(&Mask { w: c.w, h: c.h, v: f }, &cols);
    }

    // --- four ranges, far to near, each darker than the one behind --------
    let w = c.w;
    // a slow envelope over the peaks, so some massifs tower and others are
    // low: without it, four cells of noise make a picket fence
    let n1 = ridge1(w, 4, 6, 185);
    let env1 = fbm1(w, 1, 3, 385);
    let mut far = vec![0.0f32; w];
    for i in 0..w {
        far[i] = 0.470 + 0.200 * (1.0 - n1[i]).powf(1.25) * (0.45 + 0.55 * env1[i]);
    }
    let mut t = Terrain::new(&far, "#131E28", "#314656");
    t.haze = Some("#6E5B49");
    t.haze_amt = 0.12;
    t.snowline = Some(0.497);
    t.snow = "#B7C9DA";
    t.snow_shade = "#6E8698";
    t.snow_cov = 0.30;
    t.snow_depth = 0.085;
    t.seed = 161;
    t.rough = 0.72;
    terrain(c, &t);
    ridge_rim(c, &far, sx, "#E8A468", 0.22, 0.022, 0.004, 0.04);
    grad_band(c, 0.61, 0.66, "#B08C6E", 0.08, 0.02, 0.035);

    let n2 = ridge1(w, 4, 6, 202);
    let env2 = fbm1(w, 1, 3, 402);
    let mut mid = vec![0.0f32; w];
    for i in 0..w {
        mid[i] = 0.640 + 0.170 * (1.0 - n2[i]).powf(1.20) * (0.40 + 0.60 * env2[i]);
    }
    let mut t = Terrain::new(&mid, "#0A141C", "#22343F");
    t.haze = Some("#5A4A3A");
    t.haze_amt = 0.07;
    t.snowline = Some(0.657);
    t.snow = "#9FB4C6";
    t.snow_shade = "#54697C";
    t.snow_cov = 0.20;
    t.snow_depth = 0.040;
    t.seed = 262;
    t.rough = 0.66;
    terrain(c, &t);
    ridge_rim(c, &mid, sx, "#D08E58", 0.16, 0.018, 0.004, 0.02);
    grad_band(c, 0.73, 0.79, "#6B5745", 0.16, 0.02, 0.03);

    let n3 = ridge1(w, 4, 6, 303);
    let env3 = fbm1(w, 1, 3, 503);
    let mut near = vec![0.0f32; w];
    for i in 0..w {
        near[i] = 0.790 + 0.090 * (1.0 - n3[i]).powf(1.15) * (0.50 + 0.50 * env3[i]);
    }
    let mut t = Terrain::new(&near, "#060F14", "#18272D");
    t.seed = 303;
    t.rough = 0.44;
    terrain(c, &t);

    let n4 = ridge1(w, 2, 5, 404);
    let mut fore = vec![0.0f32; w];
    for i in 0..w {
        fore[i] = 0.905 + 0.030 * (1.0 - n4[i]);
    }
    let mut t = Terrain::new(&fore, "#030607", "#08100F");
    t.seed = 404;
    t.rough = 0.20;
    terrain(c, &t);
    // two rows of spruce, the back row slightly higher and smaller
    let mut tree_line = fore.clone();
    for y in tree_line.iter_mut() {
        *y -= 0.006;
    }
    pines(c, &tree_line, 26, 17, hex("#020505"), 0.024);
    let mut back = fore.clone();
    for y in back.iter_mut() {
        *y -= 0.030;
    }
    pines(c, &back, 14, 91, hex("#040808"), 0.015);
    ridge_rim(c, &tree_line, sx, "#6B4E3A", 0.09, 0.010, 0.003, 0.0);
    vignette(c, 0.36);
    grain(c, 0.010);
}

fn america(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#071229"), (0.55, "#0A1A3C"), (1.0, "#050B1A")]);
    stars(c, 90, 23, 0.7, 0.0014);
    for i in 0..7 {
        let top = 0.56 + i as f32 * 0.062;
        let m = mask_rect(c, 0.0, top, 1.0, top + 0.031, 1.2);
        let m = mask_rows(&m, 0.54, 0.60);
        let col = if i % 2 == 0 { hex("#A61E2E") } else { hex("#8C1626") };
        c.blend(&m, col, 1.0);
    }
    for i in 0..5 {
        for j in 0..4 {
            let shift = if j % 2 == 1 { 0.09 } else { 0.0 };
            let x = 0.12 + i as f32 * 0.185 + shift;
            let y = 0.12 + j as f32 * 0.085;
            if x > 0.86 {
                continue;
            }
            let mut pts = Vec::with_capacity(10);
            for k in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                let rr = if k % 2 == 0 { 0.028 } else { 0.011 };
                pts.push((x + a.cos() * rr, y + a.sin() * rr));
            }
            let mm = mask_poly(c, &pts, 0.4);
            c.blend(&mm, hex("#EDF1FA"), 1.0);
            c.add(&blur(&mm, 3.0), hex("#9FC0FF"), 0.25);
        }
    }
    // one large, very faint star
    let mut pts = Vec::with_capacity(10);
    for k in 0..10 {
        let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
        let rr = if k % 2 == 0 { 0.16 } else { 0.065 };
        pts.push((0.76 + a.cos() * rr, 0.33 + a.sin() * rr));
    }
    let mm = mask_poly(c, &pts, 0.6);
    c.add(&blur(&mm, 14.0), hex("#3E6BFF"), 0.10);
    c.blend(&mask_scale(&mm, 0.05), hex("#DCE6FF"), 1.0);
    vignette(c, 0.35);
    grain(c, 0.012);
}

fn pattaya(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#07040F"), (0.45, "#120820"), (0.75, "#1D0B22"), (1.0, "#08040C")]);
    c.add(&mask_disc(c, 0.55, 0.62, 0.45, 0.45, 1.2), hex("#FF2E9A"), 0.10);
    c.add(&mask_disc(c, 0.20, 0.55, 0.32, 0.32, 1.2), hex("#20D8FF"), 0.08);
    // the skyline
    let mut rng = Rng::new(8);
    let mut x = -0.03f32;
    while x < 1.03 {
        let bw = 0.045 + rng.f() * 0.075;
        let bh = 0.10 + rng.f().powf(1.5) * 0.28;
        let top = 0.80 - bh;
        c.blend(&mask_rect(c, x, top, x + bw, 0.86, 0.5), hex("#0A0614"), 1.0);
        let mut wy = top + 0.02;
        while wy < 0.83 {
            let mut wx = x + 0.008;
            while wx < x + bw - 0.008 {
                if rng.f() < 0.28 {
                    let col = if rng.f() < 0.6 { hex("#FFC56B") } else { hex("#7FE9FF") };
                    c.add(&mask_rect(c, wx, wy, wx + 0.006, wy + 0.010, 0.4), col, 0.35);
                }
                wx += 0.018;
            }
            wy += 0.028;
        }
        x += bw + 0.008;
    }
    // neon signs
    let signs = [
        (0.06f32, 0.30f32, 0.030f32, 0.20f32, "#FF2E9A", "HOTEL"),
        (0.15, 0.46, 0.022, 0.10, "#20D8FF", "BAR"),
        (0.78, 0.36, 0.028, 0.16, "#FFD23F", "CLUB"),
    ];
    for &(sx, sy, sw, sh, col, label) in signs.iter() {
        let frame = mask_rect(c, sx, sy, sx + sw, sy + sh, 0.5);
        let inner = mask_rect(c, sx + 0.005, sy + 0.008, sx + sw - 0.005, sy + sh - 0.008, 0.5);
        let m = mask_sub(&frame, &inner);
        let cc = hex(col);
        c.add(&blur(&m, 7.0), cc, 0.7);
        c.blend(&m, mix(cc, hex("#FFFFFF"), 0.35), 1.0);
        let short: String = label.chars().take(3).collect();
        neon_text(c, &short, sx + 0.006, sy + 0.02, 0.010, cc, 5.0, 0.55);
    }
    // palms
    for &(px, ph) in [(0.90f32, 0.30f32), (0.96, 0.24)].iter() {
        let top = 0.88 - ph;
        for k in 0..14 {
            let ang = -std::f32::consts::FRAC_PI_2 + (k as f32 / 13.0 - 0.5) * 2.4;
            let ln = 0.10 + 0.05 * (k as f32 * 2.3).cos();
            c.blend(
                &mask_capsule(c, (px, top), (px + ang.cos() * ln, top + ang.sin() * ln * 0.7), 0.006, 0.6),
                hex("#07030C"),
                1.0,
            );
        }
        c.blend(&mask_rect(c, px - 0.006, top, px + 0.006, 0.90, 0.5), hex("#07030C"), 1.0);
    }
    c.add(&mask_rect(c, 0.0, 0.86, 1.0, 0.94, 6.0), hex("#FF2E9A"), 0.10);
    c.add(&mask_rect(c, 0.0, 0.88, 1.0, 0.93, 8.0), hex("#20D8FF"), 0.06);
    vignette(c, 0.40);
    grain(c, 0.012);
}

fn burgers(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#160A07"), (0.50, "#24110C"), (0.86, "#160A08"), (1.0, "#0B0504")]);
    // tiled wall
    let mut tiles = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let tx = ((x * 7.0) % 1.0 - 0.5).abs() > 0.47;
            let ty = ((y * 8.0) % 1.0 - 0.5).abs() > 0.47;
            tiles[j * c.w + i] = if tx || ty { 1.0 } else { 0.0 };
        }
    }
    c.add_field(&tiles, hex("#3A1B14"), 0.45);
    for i in 0..7 {
        let y = i as f32 / 7.0;
        c.add(&mask_rect(c, 0.0, y + 0.001, 1.0, y + 0.004, 0.3), hex("#0F0605"), 0.5);
    }
    // neon ring
    let (cx, cy, rr) = (0.645f32, 0.54f32, 0.195f32);
    let ring = mask_sub(&mask_disc(c, cx, cy, rr, rr, 1.2), &mask_disc(c, cx, cy, rr - 0.010, rr - 0.010, 1.2));
    c.add(&blur(&ring, 16.0), hex("#FF2E86"), 0.50);
    c.add(&blur(&ring, 5.0), hex("#FF5CA0"), 0.55);
    c.blend(&blur(&ring, 0.8), hex("#FFD9E6"), 1.0);
    // burger: top bun
    let dome = mask_disc(c, cx, cy - 0.055, 0.155, 0.105, 0.9);
    let mut topf = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let v = smoothstep(cy - 0.005, cy - 0.045, y);
        for i in 0..c.w {
            topf[j * c.w + i] = v;
        }
    }
    let dome = mask_field(&dome, &topf);
    c.blend(&dome, hex("#C87F35"), 1.0);
    // warm highlight on the top of the bun
    let mut hf = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            hf[j * c.w + i] = clamp01(1.0 - ((x - cx + 0.02).powi(2) + (y - (cy - 0.115)).powi(2)) / 0.020);
        }
    }
    let hl = mask_field(&dome, &hf);
    c.add(&hl, hex("#FFD79A"), 0.55);
    let mut sf = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let v = smoothstep(cy - 0.045, cy - 0.005, y);
        for i in 0..c.w {
            sf[j * c.w + i] = v;
        }
    }
    let shade = mask_field(&dome, &sf);
    c.add(&shade, hex("#7A4318"), 0.35);
    // sesame seeds
    let mut rng = Rng::new(4);
    for _ in 0..12 {
        let a = rng.f() * std::f32::consts::PI;
        let r2 = rng.f().powf(0.55);
        let sx = cx + a.cos() * 0.130 * r2;
        let sy = cy - 0.062 - a.sin() * 0.072 * r2;
        let seed = mask_mul(&mask_disc(c, sx, sy, 0.0065, 0.009, 0.6), &dome);
        c.blend(&seed, hex("#FFF3D4"), 0.95);
    }
    // cheese, patty, lettuce, bottom bun
    c.blend(&mask_rect(c, cx - 0.150, cy - 0.020, cx + 0.150, cy + 0.004, 0.6), hex("#F2B03C"), 1.0);
    let drop = [(cx + 0.10, cy - 0.018), (cx + 0.19, cy + 0.020), (cx + 0.10, cy + 0.020)];
    c.blend(&mask_poly(c, &drop, 0.6), hex("#F2B03C"), 1.0);
    c.add(&mask_rect(c, cx - 0.150, cy - 0.018, cx + 0.150, cy - 0.010, 0.4), hex("#FFDD8A"), 0.5);
    c.blend(&mask_rect(c, cx - 0.145, cy + 0.006, cx + 0.145, cy + 0.044, 0.9), hex("#38180E"), 1.0);
    c.add(&mask_rect(c, cx - 0.140, cy + 0.010, cx + 0.140, cy + 0.020, 0.5), hex("#8A4A2A"), 0.45);
    c.add(&mask_rect(c, cx - 0.140, cy + 0.036, cx + 0.140, cy + 0.043, 0.4), hex("#1C0A05"), 0.8);
    for i in 0..11 {
        let lx = cx - 0.140 + i as f32 * 0.028;
        let dy = if i % 2 == 0 { 0.0 } else { 0.008 };
        c.blend(&mask_disc(c, lx, cy + 0.054 + dy, 0.021, 0.014, 0.6), hex("#3E9E44"), 1.0);
    }
    c.blend(&mask_rect(c, cx - 0.140, cy + 0.046, cx + 0.140, cy + 0.056, 0.6), hex("#5CC05C"), 1.0);
    c.blend(&mask_rect(c, cx - 0.130, cy + 0.064, cx + 0.130, cy + 0.108, 2.2), hex("#C0762F"), 1.0);
    c.add(&mask_rect(c, cx - 0.110, cy + 0.072, cx + 0.110, cy + 0.082, 1.1), hex("#FFC98A"), 0.45);
    c.add(&mask_rect(c, cx - 0.130, cy + 0.100, cx + 0.130, cy + 0.108, 1.1), hex("#5E3012"), 0.6);
    c.add(&blur(&mask_disc(c, cx, cy + 0.02, 0.16, 0.13, 1.4), 26.0), hex("#FF7A2F"), 0.18);
    // lettering
    neon_text(c, "BURGERS", 0.060, 0.085, 0.0122, hex("#FF2E86"), 8.0, 0.65 * ART_TEXT);
    neon_text(c, "TOKYO", 0.060, 0.155, 0.0122, hex("#FFC24A"), 8.0, 0.60 * ART_TEXT);
    // vertical neon bars on the right
    for (i, x) in [0.895f32, 0.935].iter().enumerate() {
        let x = *x;
        for j in 0..3 {
            let y = 0.44 + j as f32 * 0.105;
            let col = if i == 0 { hex("#FF2E86") } else { hex("#FFC24A") };
            c.add(&blur(&mask_rect(c, x - 0.010, y, x + 0.010, y + 0.075, 0.8), 8.0), col, 0.40);
            c.blend(&mask_rect(c, x - 0.004, y + 0.006, x + 0.004, y + 0.069, 0.7), hex("#FFE3EE"), 1.0);
        }
    }
    c.blend(&mask_rect(c, 0.0, 0.87, 1.0, 1.0, 1.2), hex("#180A08"), 1.0);
    c.add(&mask_rect(c, 0.0, 0.866, 1.0, 0.874, 0.6), hex("#FF7A2F"), 0.65);
    c.add(&mask_rect(c, 0.0, 0.874, 1.0, 0.90, 3.0), hex("#FF7A2F"), 0.10);
    vignette(c, 0.42);
    grain(c, 0.012);
}

const SUSHI_FILLS: [&[(&str, f32, f32)]; 3] = [
    &[("#E4603A", 0.38, 0.30), ("#F6C08A", 0.18, 0.14)],
    &[("#E4603A", 0.34, 0.26), ("#7FAE4C", 0.24, 0.18), ("#F6C08A", 0.14, 0.10)],
    &[("#F0A03C", 0.34, 0.26), ("#D8E8B0", 0.20, 0.16)],
];

fn sushi(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#0A0F0D"), (0.40, "#101813"), (0.70, "#141E18"), (1.0, "#070B09")]);
    let mut spot = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            spot[j * c.w + i] = (-(((x - 0.42) / 0.55).powi(2) + ((y - 0.02) / 0.55).powi(2))).exp();
        }
    }
    c.add_field(&spot, hex("#FFE7BC"), 0.10);
    // the board
    let board = mask_poly(c, &[(-0.05, 0.60), (1.05, 0.55), (1.05, 0.92), (-0.05, 0.97)], 1.0);
    c.blend(&board, hex("#3E2C1D"), 1.0);
    let mut bf = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        let v = smoothstep(0.62, 0.56, y);
        for i in 0..c.w {
            bf[j * c.w + i] = v;
        }
    }
    c.add(&mask_field(&board, &bf), hex("#8A6A44"), 0.30);
    for i in 0..6 {
        let yb = 0.64 + i as f32 * 0.045;
        let line = mask_mul(&board, &mask_rect(c, 0.0, yb, 1.0, yb + 0.0022, 0.4));
        c.blend(&line, hex("#241708"), 0.55);
    }
    c.blend(&mask_rect(c, -0.05, 0.60, 1.05, 0.615, 0.8), hex("#1A1A17"), 0.8);
    // maki rolls
    let rolls = [(0.25f32, 0.615f32, 0.085f32, 0usize), (0.50, 0.560, 0.097, 1), (0.74, 0.630, 0.080, 2)];
    let fills = SUSHI_FILLS;
    for &(cx, cy, r, fi) in rolls.iter() {
        c.add(&mask_disc(c, cx + 0.014, cy + 0.024, r * 1.02, r * 0.90, 1.6), hex("#000000"), 0.50);
        c.blend(&mask_disc(c, cx, cy, r, r * 0.93, 1.4), hex("#101C15"), 1.0);
        c.add(&mask_disc(c, cx - r * 0.25, cy - r * 0.35, r * 0.75, r * 0.30, 1.1), hex("#44604C"), 0.28);
        c.blend(&mask_disc(c, cx, cy, r * 0.72, r * 0.67, 1.0), hex("#EDE2CB"), 1.0);
        let mut rng = Rng::new((cx * 1000.0) as u64);
        let mut grains = Mask::new(c.w, c.h);
        for _ in 0..40 {
            let a = rng.f() * std::f32::consts::TAU;
            let rr = rng.f().powf(0.55) * r * 0.64;
            let gx = cx + a.cos() * rr;
            let gy = cy + a.sin() * rr * 0.92;
            merge_max(&mut grains, &mask_disc(c, gx, gy, 0.0048, 0.0068, 0.5));
        }
        c.add(&grains, hex("#FFFFFF"), 0.42);
        c.add(&mask_disc(c, cx, cy, r * 0.80, r * 0.74, 1.2), hex("#B9AE96"), 0.16);
        c.blend(&mask_disc(c, cx, cy + r * 0.10, r * 0.42, r * 0.34, 1.2), hex("#B0A28A"), 0.30);
        for (k, &(col, fr, _ar)) in fills[fi].iter().enumerate() {
            let sgn = if k == 0 { 0.012 } else { -0.012 };
            let fx = cx + sgn * r / 0.09;
            let fy = cy + if k == 0 { 0.0 } else { 0.012 };
            let mut pts = Vec::with_capacity(7);
            for q in 0..7 {
                let a = q as f32 / 7.0 * std::f32::consts::TAU;
                let rr = r * fr * (0.85 + 0.35 * (q as f32 * 2.7 + fi as f32).sin());
                pts.push((fx + a.cos() * rr, fy + a.sin() * rr * 0.9));
            }
            c.blend(&mask_poly(c, &pts, 0.6), hex(col), 1.0);
        }
    }
    // nigiri
    c.add(&mask_disc(c, 0.905, 0.725, 0.105, 0.050, 1.6), hex("#000000"), 0.40);
    c.blend(&mask_disc(c, 0.895, 0.695, 0.100, 0.048, 0.9), hex("#EDE2CB"), 1.0);
    c.add(&mask_disc(c, 0.870, 0.686, 0.060, 0.020, 1.0), hex("#FFFFFF"), 0.30);
    c.blend(&mask_capsule(c, (0.805, 0.672), (0.985, 0.648), 0.032, 0.9), hex("#E87444"), 1.0);
    c.add(&mask_capsule(c, (0.805, 0.666), (0.985, 0.642), 0.014, 1.0), hex("#F4A070"), 0.55);
    for i in 0..5 {
        let t = i as f32 / 4.0;
        let x = 0.820 + t * 0.150;
        let y = 0.672 - t * 0.022;
        c.blend(&mask_capsule(c, (x, y - 0.016), (x + 0.010, y + 0.016), 0.0032, 0.6), hex("#FFD6B8"), 0.5);
    }
    // chopsticks
    c.blend(&mask_capsule(c, (0.00, 0.24), (0.46, 0.16), 0.0075, 0.7), hex("#C7A06A"), 1.0);
    c.blend(&mask_capsule(c, (0.00, 0.285), (0.47, 0.195), 0.0075, 0.7), hex("#AF8952"), 1.0);
    // soy dish and wasabi
    c.blend(&mask_disc(c, 0.18, 0.86, 0.105, 0.055, 0.8), hex("#141210"), 1.0);
    c.blend(&mask_disc(c, 0.18, 0.852, 0.092, 0.046, 0.8), hex("#2C130C"), 1.0);
    c.add(&mask_disc(c, 0.16, 0.842, 0.035, 0.014, 0.8), hex("#FFE9C9"), 0.22);
    c.blend(&mask_disc(c, 0.36, 0.86, 0.030, 0.023, 0.7), hex("#7FAE4C"), 1.0);
    c.add(&mask_disc(c, 0.352, 0.853, 0.012, 0.008, 0.7), hex("#D7F0A8"), 0.45);
    // sesame seeds
    let mut rng = Rng::new(9);
    for _ in 0..14 {
        let x = 0.05 + rng.f() * 0.90;
        let y = 0.72 + rng.f() * 0.23;
        c.blend(&mask_disc(c, x, y, 0.004, 0.003, 0.5), hex("#E8D9B8"), 0.7);
    }
    vignette(c, 0.36);
    grain(c, 0.012);
}

// --- Amiga Workbench -------------------------------------------------------
//
// A Kickstart 1.3 screen: the blue desktop with its dither, and the Workbench
// disk window lying on it - white frame, black outline, light body, the disk
// icons with their orange labels, the checkered ball and the bottom bar.
// The window is what the interface is drawn on, so it stays a light, quiet
// field; the blue reads as the screen around it.

const AMIGA_FRAME: &str = "#FFFFFF";
const AMIGA_INK: &str = "#0A1420";
const AMIGA_BODY: &str = "#C9DCF5";
const AMIGA_ORANGE: &str = "#FF8800";

/// The chrome of a Workbench window, the way Kickstart 1.3 drew it: a black
/// outline, a white border, an optional title bar and a body inset by a
/// hairline bevel.
fn amiga_frame(c: &mut Canvas, x0: f32, y0: f32, x1: f32, y1: f32, body_col: &str, bar_bottom: f32) {
    let shadow = mask_rect(c, x0 + 0.007, y0 + 0.008, x1 + 0.007, y1 + 0.008, 2.0);
    c.blend(&shadow, hex("#012A5A"), 0.55);
    c.blend(&mask_rect(c, x0, y0, x1, y1, 0.6), hex(AMIGA_FRAME), 1.0);
    let ink = hex(AMIGA_INK);
    // the black outline of the window
    c.blend(&mask_rect(c, x0, y0, x0 + 0.005, y1, 0.4), ink, 1.0);
    c.blend(&mask_rect(c, x1 - 0.005, y0, x1, y1, 0.4), ink, 1.0);
    c.blend(&mask_rect(c, x0, y0, x1, y0 + 0.0030, 0.4), ink, 1.0);
    c.blend(&mask_rect(c, x0, y1 - 0.0030, x1, y1, 0.4), ink, 1.0);
    // title bar, and the rule that closes it off
    if bar_bottom > y0 {
        c.blend(&mask_rect(c, x0 + 0.005, y0 + 0.0030, x1 - 0.005, bar_bottom, 0.5), hex(AMIGA_FRAME), 1.0);
        c.blend(&mask_rect(c, x0 + 0.005, bar_bottom, x1 - 0.005, bar_bottom + 0.0030, 0.4), ink, 1.0);
    }
    let top = (bar_bottom + 0.0030).max(y0 + 0.0030);
    c.blend(&mask_rect(c, x0 + 0.005, top, x1 - 0.005, y1 - 0.0030, 0.5), hex(body_col), 1.0);
    // bevel: a light edge at the top left, a grey one at the bottom right
    let light = hex("#FFFFFF");
    let grey = hex("#B4BEC8");
    c.blend(&mask_rect(c, x0 + 0.005, top, x0 + 0.0065, y1 - 0.0030, 0.4), light, 0.85);
    c.blend(&mask_rect(c, x0 + 0.005, top, x1 - 0.005, top + 0.0022, 0.4), light, 0.85);
    c.blend(&mask_rect(c, x1 - 0.0065, top, x1 - 0.005, y1 - 0.0030, 0.4), grey, 0.85);
    c.blend(&mask_rect(c, x0 + 0.005, y1 - 0.0050, x1 - 0.005, y1 - 0.0030, 0.4), grey, 0.85);
}

/// One of the floppy-disk icons of the Workbench disk window: pale shell,
/// dark shutter, an orange label stripe and the name written underneath.
fn amiga_floppy(c: &mut Canvas, x: f32, y: f32, label: &str) {
    let (w, h) = (0.068, 0.060);
    c.blend(&mask_rect(c, x + 0.003, y + 0.003, x + w + 0.003, y + h + 0.003, 0.6), hex("#AEBECF"), 1.0);
    c.blend(&mask_rect(c, x, y, x + w, y + h, 0.5), hex("#22303F"), 1.0);
    c.blend(&mask_rect(c, x + 0.005, y + 0.004, x + w - 0.005, y + 0.017, 0.4), hex("#8A9AAB"), 1.0);
    c.blend(&mask_rect(c, x + 0.014, y + 0.006, x + w - 0.014, y + 0.012, 0.4), hex("#22303F"), 1.0);
    c.blend(&mask_rect(c, x + 0.006, y + 0.025, x + w - 0.006, y + h - 0.004, 0.4), hex("#FFFFFF"), 1.0);
    c.blend(&mask_rect(c, x + 0.006, y + 0.026, x + w - 0.006, y + 0.033, 0.4), hex(AMIGA_ORANGE), 1.0);
    c.blend(&text_mask(c, label, x + 0.002, y + h + 0.0045, 0.0022), hex(AMIGA_INK), 0.85);
}

/// The checkered Boing ball, Amiga's mascot.
fn amiga_ball(c: &mut Canvas, cx: f32, cy: f32, r: f32) {
    let sphere = mask_disc(c, cx, cy, r, r, 0.9);
    c.blend(&sphere, hex("#C8102E"), 1.0);
    let mut chk = Mask::new(c.w, c.h);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            if d > r {
                continue;
            }
            let v = (d / r).clamp(0.0, 1.0);
            let u = (y - cy).atan2(x - cx);
            let lat = (1.0 - 2.0 * v * v).clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
            let cell = ((u / std::f32::consts::TAU + 0.5) * 8.0).floor() + (lat * 6.0).floor();
            if cell as i32 % 2 != 0 {
                chk.v[j * c.w + i] = 1.0;
            }
        }
    }
    c.blend(&mask_mul(&sphere, &chk), hex("#FFFFFF"), 1.0);
    // shading round the rim, so it reads as a ball and not as a disc
    let mut shade = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            let v = (d / r).clamp(0.0, 1.0);
            shade[j * c.w + i] = sphere.v[j * c.w + i] * smoothstep(0.42, 1.02, v) * 0.45;
        }
    }
    for i in 0..c.px.len() {
        let k = 1.0 - shade[i];
        let p = &mut c.px[i];
        p[0] *= k;
        p[1] *= k;
        p[2] *= k;
    }
    let gloss = mask_disc(c, cx - r * 0.42, cy - r * 0.46, r * 0.26, r * 0.19, 0.8);
    c.add(&gloss, hex("#FFFFFF"), 0.30);
}

fn amiga(c: &mut Canvas) {
    // ---- the desktop: blue, in the dither the chipset drew it with --------
    vgrad(c, &[(0.0, "#1266C4"), (0.5, "#0A56AE"), (1.0, "#064A9C")]);
    for j in 0..c.h {
        for i in 0..c.w {
            if (i / 2 + j / 2) % 2 == 0 {
                let p = &mut c.px[j * c.w + i];
                p[0] *= 0.94;
                p[1] *= 0.94;
                p[2] *= 0.94;
            }
        }
    }

    // ---- the Workbench disk window ---------------------------------------
    // Its title bar is left plain and runs down to the height of the
    // program's own window buttons, which are drawn inside it.
    amiga_frame(c, 0.030, 0.014, 0.970, 0.986, AMIGA_BODY, 0.070);

    // ---- the disk icons, top left ----------------------------------------
    amiga_floppy(c, 0.052, 0.066, "NIKPAK");
    amiga_floppy(c, 0.148, 0.066, "DISK");

    // ---- the orange highlight row ----------------------------------------
    c.blend(&mask_rect(c, 0.050, 0.1460, 0.232, 0.1605, 0.5), hex(AMIGA_ORANGE), 1.0);
    c.blend(&text_mask(c, "WORKBENCH", 0.055, 0.1475, 0.0018), hex(AMIGA_INK), 0.85);

    // ---- a drawer, under the icons ---------------------------------------
    c.blend(&mask_rect(c, 0.057, 0.1695, 0.129, 0.2135, 0.6), hex("#AEBECF"), 1.0);
    c.blend(&mask_rect(c, 0.054, 0.1660, 0.126, 0.2100, 0.5), hex("#E8E0C4"), 1.0);
    c.blend(&mask_rect(c, 0.054, 0.1660, 0.126, 0.1725, 0.4), hex("#22303F"), 1.0);
    c.blend(&mask_rect(c, 0.058, 0.1750, 0.122, 0.1775, 0.4), hex(AMIGA_ORANGE), 1.0);
    c.blend(&text_mask(c, "WORK", 0.054, 0.2125, 0.0022), hex(AMIGA_INK), 0.85);

    // ---- the ball, top right ---------------------------------------------
    amiga_ball(c, 0.855, 0.150, 0.072);

    // ---- a second window, blue-bodied, at the right ----------------------
    amiga_frame(c, 0.735, 0.258, 0.952, 0.400, "#0A3A6E", 0.2850);
    c.blend(&text_mask(c, "WORKBENCH", 0.7455, 0.2655, 0.0022), hex(AMIGA_INK), 0.85);
    for (i, &(k, col)) in [("NIKPAK", "#FFFFFF"), ("DISK", "#FFFFFF"), ("1.6.3", "#8FC0FF")].iter().enumerate() {
        c.blend(&text_mask(c, k, 0.7465, 0.2900 + i as f32 * 0.0235, 0.0022), hex(col), 0.85);
    }

    // ---- the trashcan, bottom left ---------------------------------------
    c.blend(&mask_rect(c, 0.053, 0.8560, 0.105, 0.8625, 0.4), hex("#22303F"), 1.0);
    c.blend(&mask_rect(c, 0.058, 0.8600, 0.100, 0.9100, 0.5), hex("#EAEAE6"), 1.0);
    c.blend(&mask_rect(c, 0.093, 0.8600, 0.100, 0.9100, 0.4), hex("#B0B6BE"), 0.9);
    c.blend(&text_mask(c, "TRASH", 0.052, 0.9130, 0.0022), hex(AMIGA_INK), 0.85);

    // ---- the bottom bar of the screen ------------------------------------
    c.blend(&mask_rect(c, 0.030, 0.9540, 0.970, 0.9740, 0.6), hex("#D6DEE8"), 1.0);
    c.blend(&mask_rect(c, 0.030, 0.9540, 0.970, 0.9557, 0.4), hex("#8A96A4"), 1.0);
    c.blend(&mask_rect(c, 0.040, 0.9595, 0.046, 0.9685, 0.4), hex(AMIGA_ORANGE), 1.0);
    c.blend(&text_mask(c, "NIKPAK 1.6", 0.7520, 0.9570, 0.0022), hex(AMIGA_INK), 0.85);

    vignette(c, 0.10);
    grain(c, 0.006);
}


fn msx(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#020A20"), (0.55, "#03102E"), (1.0, "#01061A")]);
    c.add(&mask_disc(c, 0.5, 0.5, 0.55, 0.45, 1.4), hex("#1B4FFF"), 0.09);
    for j in 0..c.h {
        let scan = 0.5 + 0.5 * ((j as f32 + 0.5) * std::f32::consts::PI / 2.2).cos();
        let k = 0.90 + 0.10 * scan;
        for i in 0..c.w {
            let p = &mut c.px[j * c.w + i];
            p[0] *= k;
            p[1] *= k;
            p[2] *= k;
        }
    }
    let m = text_mask(c, "MSX", 0.09, 0.13, 0.035);
    c.add(&blur(&m, 8.0), hex("#FF2D2D"), 0.5 * ART_TEXT);
    c.add(&blur(&m, 2.0), hex("#FF6A5A"), 0.4 * ART_TEXT);
    c.blend(&m, hex("#F2F2F2"), ART_TEXT);
    let basic = text_mask(c, "BASIC", 0.10, 0.40, 0.010);
    c.blend(&basic, hex("#7FA8FF"), ART_TEXT);
    for i in 0..7 {
        let y = 0.47 + i as f32 * 0.036;
        let wdt = 0.22 + ((i * 37) % 23) as f32 * 0.014;
        c.blend(&mask_rect(c, 0.10, y, 0.10 + wdt, y + 0.012, 0.4), hex("#5C86D8"), 0.55);
    }
    let ready = text_mask(c, "READY", 0.10, 0.80, 0.011);
    c.blend(&ready, hex("#BFD4FF"), ART_TEXT);
    c.add(&mask_rect(c, 0.235, 0.802, 0.255, 0.820, 0.3), hex("#BFD4FF"), 0.75);
    c.blend(&mask_rect(c, 0.72, 0.08, 0.95, 0.30, 1.0), hex("#0A1A44"), 1.0);
    c.blend(&mask_rect(c, 0.728, 0.088, 0.942, 0.292, 0.8), hex("#12306E"), 1.0);
    for i in 0..3 {
        let y = 0.115 + i as f32 * 0.055;
        c.blend(&mask_rect(c, 0.742, y, 0.928, y + 0.028, 0.4), hex("#2C5AC4"), 0.7);
        c.blend(&mask_rect(c, 0.748, y + 0.008, 0.922, y + 0.020, 0.3), hex("#0A1A44"), 1.0);
    }
    vignette(c, 0.34);
    grain(c, 0.01);
}

fn colecovision(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#0A0514"), (0.5, "#120A22"), (1.0, "#05030A")]);
    let (cx, cy) = (0.5f32, 0.62f32);
    let cols = ["#E23A3A", "#F08A24", "#F2D33A", "#3FBF6A", "#2E7BE8", "#8A4FD8"];
    for (i, col) in cols.iter().enumerate() {
        let r0 = 0.30 + i as f32 * 0.038;
        let ring = mask_sub(&mask_disc(c, cx, cy, r0 + 0.030, r0 + 0.030, 0.8), &mask_disc(c, cx, cy, r0, r0, 0.8));
        let ring = mask_rows(&ring, 0.58, 0.40);
        let cc = hex(*col);
        c.add(&blur(&ring, 9.0), cc, 0.55);
        c.blend(&mask_scale(&ring, 0.9), cc, 1.0);
    }
    c.add(&mask_disc(c, cx, cy, 0.42, 0.42, 1.0), hex("#FFFFFF"), 0.04);
    // keypad
    for j in 0..4 {
        for i in 0..3 {
            let x = 0.09 + i as f32 * 0.055;
            let y = 0.72 + j as f32 * 0.055;
            c.blend(&mask_rect(c, x, y, x + 0.040, y + 0.040, 0.8), hex("#241736"), 1.0);
            c.blend(&mask_rect(c, x + 0.006, y + 0.006, x + 0.034, y + 0.030, 0.6), hex("#352350"), 1.0);
            c.add(&mask_rect(c, x + 0.006, y + 0.032, x + 0.034, y + 0.034, 0.3), hex("#8A6BC0"), 0.4);
        }
    }
    let m = text_mask(c, "COLECOVISION", 0.12, 0.19, 0.0095);
    c.add(&blur(&m, 6.0), hex("#7F5CFF"), 0.35 * ART_TEXT);
    c.blend(&m, hex("#EDE6FF"), ART_TEXT);
    // controller cable
    for k in 0..24 {
        let a = k as f32 / 24.0 * std::f32::consts::TAU * 1.25;
        let f = 1.0 - k as f32 / 40.0;
        let x = 0.80 + 0.10 * a.cos() * f;
        let y = 0.70 + 0.10 * a.sin() * f;
        let x2 = 0.80 + 0.10 * (a + 0.35).cos() * f;
        let y2 = 0.70 + 0.10 * (a + 0.35).sin() * f;
        c.blend(&mask_capsule(c, (x, y), (x2, y2), 0.005, 0.5), hex("#1A1228"), 1.0);
    }
    vignette(c, 0.40);
    grain(c, 0.012);
}

fn zx_spectrum(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#060606"), (0.55, "#0A0A0B"), (1.0, "#030303")]);
    let mut gridm = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            let gx = ((x * 64.0) % 1.0 - 0.5).abs() > 0.47;
            let gy = ((y * 72.0) % 1.0 - 0.5).abs() > 0.47;
            if gx || gy {
                gridm[j * c.w + i] = 1.0;
            }
        }
    }
    c.add_field(&gridm, hex("#1E1E26"), 0.35);
    let cols = ["#D7332E", "#E8B426", "#3BD14A", "#2FC7D8"];
    for (i, col) in cols.iter().enumerate() {
        let off = 0.055 * i as f32;
        let pts = [(-0.1, 0.62 + off), (1.1, 0.38 + off), (1.1, 0.44 + off), (-0.1, 0.68 + off)];
        let m = mask_poly(c, &pts, 0.7);
        let cc = hex(*col);
        c.add(&blur(&m, 7.0), cc, 0.35);
        c.blend(&m, cc, 1.0);
    }
    let l1 = text_mask(c, "10 PRINT \"NIKPAK\"", 0.08, 0.12, 0.0085);
    c.blend(&l1, hex("#E8E8F0"), ART_TEXT);
    let l2 = text_mask(c, "20 GOTO 10", 0.08, 0.175, 0.0085);
    c.blend(&l2, hex("#A8A8BC"), ART_TEXT);
    let name = text_mask(c, "ZX SPECTRUM", 0.08, 0.80, 0.0105);
    c.add(&blur(&name, 5.0), hex("#2FC7D8"), 0.30 * ART_TEXT);
    c.blend(&name, hex("#F0F0F5"), ART_TEXT);
    c.add(&mask_rect(c, 0.235, 0.185, 0.247, 0.196, 0.2), hex("#E8E8F0"), 0.8);
    vignette(c, 0.36);
    grain(c, 0.008);
}

fn catlife(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#F7E9D2"), (0.5, "#F1DCC0"), (1.0, "#E4C7A0")]);
    let mut f = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            f[j * c.w + i] = (-(((x - 0.22) / 0.24).powi(2) + ((y - 0.24) / 0.24).powi(2))).exp();
        }
    }
    c.add_field(&f, hex("#FFD79A"), 0.35);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            f[j * c.w + i] = (-(((x - 0.22) / 0.10).powi(2) + ((y - 0.24) / 0.10).powi(2))).exp();
        }
    }
    c.add_field(&f, hex("#FFC978"), 0.30);
    // paw prints
    let mut rng = Rng::new(6);
    for _ in 0..9 {
        let px = 0.05 + rng.f() * 0.90;
        let py = 0.55 + rng.f() * 0.40;
        let ang = rng.f() * 1.2 - 0.6;
        let col = hex("#C98A55");
        c.blend(&mask_disc(c, px, py + 0.012, 0.011, 0.009, 0.7), col, 1.0);
        for k in 0..4 {
            let a = -1.1 + k as f32 * 0.62 + ang;
            c.blend(&mask_disc(c, px + a.cos() * 0.018, py + 0.002 + a.sin() * 0.013, 0.0048, 0.0048, 0.6), col, 1.0);
        }
    }
    // the cat
    let (cx, base) = (0.68f32, 0.88f32);
    let body = mask_disc(c, cx, base - 0.02, 0.15, 0.20, 1.0);
    let head = mask_disc(c, cx, base - 0.27, 0.098, 0.085, 1.0);
    let ear1 = mask_poly(c, &[(cx - 0.09, base - 0.31), (cx - 0.075, base - 0.42), (cx - 0.015, base - 0.35)], 0.7);
    let ear2 = mask_poly(c, &[(cx + 0.09, base - 0.31), (cx + 0.075, base - 0.42), (cx + 0.015, base - 0.35)], 0.7);
    let tail = mask_capsule(c, (cx + 0.13, base + 0.02), (cx + 0.22, base - 0.06), 0.026, 0.8);
    let cat = mask_add(&mask_add(&body, &head), &mask_add(&mask_add(&ear1, &ear2), &tail));
    c.add(&blur(&cat, 10.0), hex("#B36A34"), 0.18);
    c.blend(&cat, hex("#C97C3F"), 1.0);
    for k in 0..4 {
        let y = base - 0.12 + k as f32 * 0.045;
        let stripe = mask_mul(&cat, &mask_rect(c, cx - 0.11, y, cx + 0.11, y + 0.012, 0.6));
        c.blend(&stripe, hex("#B0652F"), 0.55);
    }
    c.blend(&mask_disc(c, cx, base - 0.25, 0.045, 0.030, 0.7), hex("#E7B98A"), 1.0);
    c.blend(&mask_disc(c, cx - 0.055, base + 0.035, 0.038, 0.030, 0.7), hex("#C97C3F"), 1.0);
    c.blend(&mask_disc(c, cx + 0.055, base + 0.035, 0.038, 0.030, 0.7), hex("#C97C3F"), 1.0);
    c.blend(&mask_disc(c, cx, base - 0.13, 0.062, 0.075, 0.8), hex("#E2B183"), 1.0);
    for dx in [-0.014f32, 0.014].iter() {
        let dx = *dx;
        c.blend(&mask_capsule(c, (cx + dx, base + 0.045), (cx + dx, base + 0.055), 0.0016, 0.5), hex("#8A4A22"), 0.7);
    }
    for dx in [-0.038f32, 0.038].iter() {
        let dx = *dx;
        c.blend(&mask_disc(c, cx + dx, base - 0.285, 0.013, 0.016, 0.5), hex("#3A2A1C"), 1.0);
        c.add(&mask_disc(c, cx + dx - 0.004, base - 0.290, 0.005, 0.007, 0.5), hex("#FFF6E0"), 0.6);
    }
    c.blend(&mask_poly(c, &[(cx - 0.008, base - 0.262), (cx + 0.008, base - 0.262), (cx, base - 0.252)], 0.4), hex("#8C4A2A"), 1.0);
    for d in [-1.0f32, 1.0].iter() {
        let d = *d;
        for k in 0..2 {
            c.blend(
                &mask_capsule(c, (cx + d * 0.04, base - 0.255 + k as f32 * 0.006), (cx + d * 0.13, base - 0.262 + k as f32 * 0.012), 0.0018, 0.5),
                hex("#7A4A28"),
                1.0,
            );
        }
    }
    c.add(&mask_disc(c, cx, base + 0.045, 0.19, 0.045, 1.2), hex("#8A5A30"), 0.25);
    vignette(c, 0.18);
    grain(c, 0.008);
}

fn dog_bone(c: &mut Canvas, x0: f32, y0: f32, a: f32) {
    let dx = a.cos();
    let dy = a.sin();
    let p0 = (x0 - dx * 0.035, y0 - dy * 0.035);
    let p1 = (x0 + dx * 0.035, y0 + dy * 0.035);
    c.blend(&mask_capsule(c, p0, p1, 0.008, 0.6), hex("#F6F0DE"), 1.0);
    for s in [-1.0f32, 1.0].iter() {
        for q in [-1.0f32, 1.0].iter() {
            let ox = -dy * 0.012 * *q;
            let oy = dx * 0.012 * *q;
            c.blend(&mask_disc(c, p0.0 + *s * dx * 0.008 + ox, p0.1 + *s * dy * 0.008 + oy, 0.011, 0.011, 0.6), hex("#F6F0DE"), 1.0);
            c.blend(&mask_disc(c, p1.0 + *s * dx * 0.008 + ox, p1.1 + *s * dy * 0.008 + oy, 0.011, 0.011, 0.6), hex("#F6F0DE"), 1.0);
        }
    }
}

fn doglife(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#DDEBF7"), (0.5, "#EAF2F7"), (0.78, "#D7E3C9"), (1.0, "#A9C08B")]);
    c.add(&mask_disc(c, 0.80, 0.18, 0.16, 0.16, 1.4), hex("#FFF3C8"), 0.6);
    let mut gy = fbm1(c.w, 5, 4, 77);
    for v in gy.iter_mut() {
        *v = 0.80 + *v * 0.02;
    }
    ridge_poly(c, &gy, "#8FAE6B", "#6E8F4E");
    let (cx, base) = (0.66f32, 0.80f32);
    let body = mask_disc(c, cx, base - 0.09, 0.16, 0.19, 1.0);
    let head = mask_disc(c, cx, base - 0.32, 0.105, 0.095, 1.0);
    let ear1 = mask_disc(c, cx - 0.09, base - 0.33, 0.045, 0.075, 0.9);
    let ear2 = mask_disc(c, cx + 0.09, base - 0.33, 0.040, 0.070, 0.9);
    let snout = mask_disc(c, cx, base - 0.285, 0.055, 0.040, 0.8);
    let tail = mask_capsule(c, (cx + 0.14, base + 0.02), (cx + 0.20, base - 0.10), 0.022, 0.8);
    let legs = mask_add(
        &mask_rect(c, cx - 0.10, base - 0.02, cx - 0.05, base + 0.06, 0.8),
        &mask_rect(c, cx + 0.04, base - 0.02, cx + 0.09, base + 0.06, 0.8),
    );
    let dog = mask_add(&mask_add(&mask_add(&body, &head), &snout), &mask_add(&mask_add(&ear1, &ear2), &mask_add(&tail, &legs)));
    c.add(&blur(&dog, 10.0), hex("#8A6236"), 0.18);
    c.blend(&dog, hex("#C79A62"), 1.0);
    let chest = mask_mul(&dog, &mask_disc(c, cx, base - 0.22, 0.065, 0.10, 0.8));
    c.blend(&chest, hex("#E2C79A"), 0.75);
    c.blend(&mask_disc(c, cx, base - 0.272, 0.016, 0.013, 0.5), hex("#2A211A"), 1.0);
    for dx in [-0.040f32, 0.040].iter() {
        let dx = *dx;
        c.blend(&mask_disc(c, cx + dx, base - 0.345, 0.012, 0.014, 0.5), hex("#2A211A"), 1.0);
        c.add(&mask_disc(c, cx + dx - 0.003, base - 0.350, 0.004, 0.005, 0.5), hex("#FFFFFF"), 0.7);
    }
    c.blend(&mask_rect(c, cx - 0.075, base - 0.245, cx + 0.075, base - 0.225, 0.6), hex("#D9584C"), 1.0);
    c.blend(&mask_disc(c, cx, base - 0.20, 0.012, 0.012, 0.5), hex("#F2C14A"), 1.0);
    for &(bx, by, ang) in [(0.14f32, 0.88f32, 0.5f32), (0.30, 0.94, -0.2), (0.90, 0.90, 0.9)].iter() {
        dog_bone(c, bx, by, ang);
    }
    let mut rng = Rng::new(12);
    for _ in 0..6 {
        let px = 0.05 + rng.f() * 0.45;
        let py = 0.84 + rng.f() * 0.13;
        c.blend(&mask_disc(c, px, py + 0.012, 0.010, 0.008, 0.6), hex("#5E7C42"), 1.0);
        for k in 0..4 {
            let a = -1.2 + k as f32 * 0.6;
            c.blend(&mask_disc(c, px + a.cos() * 0.016, py + a.sin() * 0.013, 0.0042, 0.0042, 0.55), hex("#5E7C42"), 1.0);
        }
    }
    vignette(c, 0.14);
    grain(c, 0.008);
}

fn mario_cloud(c: &mut Canvas, cx: f32, cy: f32, s: f32) {
    for &(dx, dy, r) in [(-0.9f32, 0.15f32, 0.5f32), (-0.35, -0.15, 0.7), (0.35, -0.10, 0.65), (0.9, 0.15, 0.5)].iter() {
        c.blend(&mask_disc(c, cx + dx * s, cy + dy * s, r * s, r * s * 0.75, 0.9), hex("#FFFFFF"), 1.0);
    }
    c.add(&mask_disc(c, cx, cy + s * 0.35, s * 1.3, s * 0.30, 1.0), hex("#FFFFFF"), 0.6);
    c.add(&mask_disc(c, cx, cy + s * 0.55, s * 1.4, s * 0.18, 1.0), hex("#BFD8FF"), 0.25);
}

fn mario(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#5C94FC"), (0.55, "#6FA6FF"), (0.80, "#8FC0FF"), (1.0, "#B7D8FF")]);
    mario_cloud(c, 0.20, 0.20, 0.075);
    mario_cloud(c, 0.72, 0.30, 0.065);
    mario_cloud(c, 0.48, 0.10, 0.050);
    for &(bx, s) in [(0.08f32, 0.9f32), (0.30, 0.7), (0.86, 0.8)].iter() {
        c.blend(&mask_disc(c, bx, 0.86, 0.14 * s, 0.08 * s, 1.0), hex("#2E9B3C"), 1.0);
        c.blend(&mask_disc(c, bx - 0.08 * s, 0.87, 0.09 * s, 0.06 * s, 1.0), hex("#3DB24D"), 1.0);
        c.blend(&mask_disc(c, bx + 0.09 * s, 0.87, 0.08 * s, 0.055 * s, 1.0), hex("#3DB24D"), 1.0);
    }
    let by = 0.68f32;
    let bx0 = 0.08f32;
    for i in 0..4 {
        let x = bx0 + i as f32 * 0.155;
        c.blend(&mask_rect(c, x, by, x + 0.14, by + 0.075, 0.7), hex("#C84C0C"), 1.0);
        for r in 0..2 {
            for cc in 0..2 {
                let off = if r % 2 == 1 { 0.033 } else { 0.0 };
                let xx = x + 0.008 + cc as f32 * 0.066 + off;
                let yy = by + 0.006 + r as f32 * 0.035;
                c.blend(&mask_rect(c, xx, yy, xx + 0.058, yy + 0.026, 0.3), hex("#A93A08"), 1.0);
            }
        }
        c.add(&mask_rect(c, x, by, x + 0.14, by + 0.006, 0.4), hex("#F08A4A"), 0.6);
    }
    // a mystery block
    let (x, y) = (0.70f32, 0.60f32);
    c.blend(&mask_rect(c, x, y, x + 0.11, y + 0.11, 0.7), hex("#E8A020"), 1.0);
    c.blend(&mask_rect(c, x + 0.010, y + 0.010, x + 0.100, y + 0.100, 0.5), hex("#F7C948"), 1.0);
    for cc in 0..4 {
        for r in 0..4 {
            c.add(&mask_disc(c, x + 0.020 + cc as f32 * 0.023, y + 0.020 + r as f32 * 0.023, 0.004, 0.004, 0.5), hex("#FFF0A8"), 0.6);
        }
    }
    // pipe
    let px = 0.84f32;
    c.blend(&mask_rect(c, px, 0.78, px + 0.16, 1.0, 0.7), hex("#1F9B2E"), 1.0);
    c.blend(&mask_rect(c, px - 0.015, 0.75, px + 0.175, 0.80, 0.7), hex("#2DBE3E"), 1.0);
    c.add(&mask_rect(c, px + 0.015, 0.78, px + 0.045, 1.0, 0.5), hex("#7FE08A"), 0.5);
    c.add(&mask_rect(c, px - 0.005, 0.752, px + 0.020, 0.798, 0.4), hex("#9FF0A8"), 0.6);
    // coins
    for cx in [0.32f32, 0.58].iter() {
        let cx = *cx;
        c.add(&mask_disc(c, cx, 0.34, 0.030, 0.038, 0.7), hex("#FFD54A"), 0.35);
        c.blend(&mask_disc(c, cx, 0.34, 0.026, 0.034, 0.6), hex("#F2B31C"), 1.0);
        c.blend(&mask_rect(c, cx - 0.004, 0.318, cx + 0.004, 0.362, 0.4), hex("#FFE9A0"), 1.0);
    }
    // ground
    c.blend(&mask_rect(c, 0.0, 0.92, 1.0, 1.0, 0.8), hex("#C86428"), 1.0);
    for i in 0..12 {
        let x = i as f32 / 12.0;
        c.blend(&mask_rect(c, x, 0.92, x + 0.08, 0.945, 0.4), hex("#A94E1C"), 1.0);
    }
    c.add(&mask_rect(c, 0.0, 0.92, 1.0, 0.928, 0.4), hex("#F2A05A"), 0.5);
    vignette(c, 0.12);
    grain(c, 0.008);
}

fn cs_crate(c: &mut Canvas, x0: f32, y0: f32, s: f32, dark: bool) {
    let (c1, c2) = if dark { ("#5A4326", "#402D1A") } else { ("#7A5A34", "#5C4226") };
    c.blend(&mask_rect(c, x0, y0, x0 + s, y0 + s * 0.72, 0.6), hex(c1), 1.0);
    c.add(&mask_rect(c, x0, y0, x0 + s, y0 + s * 0.05, 0.3), hex("#C9A06A"), 0.35);
    for k in 1..3 {
        let yy = y0 + s * 0.72 * k as f32 / 3.0;
        c.blend(&mask_rect(c, x0, yy, x0 + s, yy + 0.008, 0.3), hex(c2), 1.0);
    }
    c.blend(&mask_rect(c, x0 + s * 0.46, y0, x0 + s * 0.54, y0 + s * 0.72, 0.3), hex(c2), 1.0);
}

fn cs(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#2A2A34"), (0.35, "#4A4038"), (0.62, "#8A6E4E"), (0.85, "#B08C60"), (1.0, "#6A4F34")]);
    let mut sun = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            sun[j * c.w + i] = (-(((x - 0.28) / 0.20).powi(2) + ((y - 0.50) / 0.16).powi(2))).exp();
        }
    }
    c.add_field(&sun, hex("#FFC26B"), 0.30);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            sun[j * c.w + i] = (-(((x - 0.28) / 0.055).powi(2) + ((y - 0.50) / 0.05).powi(2))).exp();
        }
    }
    c.add_field(&sun, hex("#FFE9C0"), 0.45);
    // distant town
    let mut rng = Rng::new(9);
    let mut x = -0.02f32;
    while x < 1.02 {
        let bw = 0.05 + rng.f() * 0.10;
        let bh = 0.06 + rng.f().powf(1.4) * 0.22;
        c.blend(&mask_rect(c, x, 0.72 - bh, x + bw, 0.72, 0.6), hex("#5E4A34"), 1.0);
        c.add(&mask_rect(c, x, 0.72 - bh, x + 0.004, 0.72, 0.4), hex("#A88A5E"), 0.35);
        let mut wy = 0.72 - bh + 0.02;
        while wy < 0.70 {
            let mut wx = x + 0.012;
            while wx < x + bw - 0.010 {
                if rng.f() < 0.18 {
                    c.add(&mask_rect(c, wx, wy, wx + 0.007, wy + 0.012, 0.4), hex("#FFD08A"), 0.30);
                }
                wx += 0.022;
            }
            wy += 0.035;
        }
        x += bw + 0.012;
    }
    cs_crate(c, 0.04, 0.70, 0.26, false);
    cs_crate(c, 0.22, 0.78, 0.30, true);
    cs_crate(c, 0.78, 0.74, 0.22, true);
    // watchtower
    c.blend(&mask_rect(c, 0.66, 0.36, 0.69, 0.72, 0.4), hex("#4A3A28"), 1.0);
    c.blend(&mask_rect(c, 0.63, 0.36, 0.72, 0.38, 0.4), hex("#4A3A28"), 1.0);
    c.add(&mask_rect(c, 0.0, 0.57, 1.0, 0.73, 16.0), hex("#D8B27A"), 0.22);
    // HUD crosshair
    let (hx, hy) = (0.74f32, 0.28f32);
    let col = hex("#B6E33A");
    c.add(&blur(&mask_disc(c, hx, hy, 0.09, 0.09, 1.2), 8.0), col, 0.14);
    for &(dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].iter() {
        c.blend(&mask_capsule(c, (hx + dx * 0.030, hy + dy * 0.030), (hx + dx * 0.050, hy + dy * 0.050), 0.0022, 0.5), col, 1.0);
    }
    c.blend(&mask_disc(c, hx, hy, 0.006, 0.006, 0.5), col, 1.0);
    for &(sx, sy) in [(1.0f32, 1.0f32), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)].iter() {
        let x0 = hx + sx * 0.070;
        let y0 = hy + sy * 0.070;
        c.blend(&mask_capsule(c, (x0, y0), (x0 + sx * 0.018, y0), 0.0018, 0.4), col, 1.0);
        c.blend(&mask_capsule(c, (x0, y0), (x0, y0 + sy * 0.018), 0.0018, 0.4), col, 1.0);
    }
    // camo blotches at the edges
    let mut rng = Rng::new(21);
    for _ in 0..26 {
        let x0 = -0.05 + rng.f() * 1.10;
        let y0 = -0.05 + rng.f() * 1.10;
        if x0 > 0.25 && x0 < 0.90 && y0 > 0.10 && y0 < 0.80 {
            continue;
        }
        let s = 0.05 + rng.f() * 0.08;
        let mut pts = Vec::with_capacity(7);
        for k in 0..7 {
            let a = k as f32 / 7.0 * std::f32::consts::TAU;
            let rr = s * (0.7 + rng.f() * 0.5);
            pts.push((x0 + a.cos() * rr, y0 + a.sin() * rr * 0.7));
        }
        c.blend(&mask_poly(c, &pts, 2.0), hex("#3E3A22"), 0.20);
    }
    vignette(c, 0.34);
    grain(c, 0.010);
}

fn chess_piece(c: &mut Canvas, cx: f32, base_y: f32, s: f32, crown: bool) {
    let col = hex("#120D09");
    c.add(&mask_disc(c, cx, base_y + s * 0.015, s * 0.75, s * 0.16, 1.4), hex("#000000"), 0.45);
    c.blend(
        &mask_poly(c, &[(cx - s * 0.55, base_y), (cx + s * 0.55, base_y), (cx + s * 0.34, base_y - s * 0.18), (cx - s * 0.34, base_y - s * 0.18)], 0.7),
        col,
        1.0,
    );
    c.blend(
        &mask_poly(
            c,
            &[(cx - s * 0.34, base_y - s * 0.18), (cx + s * 0.34, base_y - s * 0.18), (cx + s * 0.20, base_y - s * 0.55), (cx - s * 0.20, base_y - s * 0.55)],
            0.7,
        ),
        col,
        1.0,
    );
    c.blend(&mask_rect(c, cx - s * 0.30, base_y - s * 0.62, cx + s * 0.30, base_y - s * 0.55, 0.7), col, 1.0);
    c.blend(&mask_disc(c, cx, base_y - s * 0.78, s * 0.26, s * 0.24, 0.7), col, 1.0);
    if crown {
        c.blend(&mask_rect(c, cx - s * 0.22, base_y - s * 1.02, cx + s * 0.22, base_y - s * 0.94, 0.5), col, 1.0);
        for k in 0..3 {
            let x = cx - s * 0.20 + k as f32 * s * 0.20;
            c.blend(&mask_poly(c, &[(x - s * 0.08, base_y - s * 1.02), (x, base_y - s * 1.22), (x + s * 0.08, base_y - s * 1.02)], 0.4), col, 1.0);
        }
        c.blend(&mask_rect(c, cx - s * 0.035, base_y - s * 1.34, cx + s * 0.035, base_y - s * 1.10, 0.4), col, 1.0);
        c.blend(&mask_rect(c, cx - s * 0.10, base_y - s * 1.25, cx + s * 0.10, base_y - s * 1.19, 0.4), col, 1.0);
    }
    let gold = hex("#E3B778");
    c.add(&mask_capsule(c, (cx - s * 0.20, base_y - s * 0.55), (cx - s * 0.33, base_y - s * 0.05), s * 0.022, 0.7), gold, 0.55);
    c.add(&mask_capsule(c, (cx - s * 0.26, base_y - s * 0.78), (cx - s * 0.10, base_y - s * 0.92), s * 0.020, 0.7), gold, 0.55);
    if crown {
        c.add(&mask_capsule(c, (cx - s * 0.21, base_y - s * 0.96), (cx - s * 0.21, base_y - s * 1.16), s * 0.016, 0.6), gold, 0.55);
    }
}

fn chess(c: &mut Canvas) {
    vgrad(c, &[(0.0, "#160F0A"), (0.40, "#241811"), (0.75, "#180F0A"), (1.0, "#0A0604")]);
    let mut lamp = vec![0.0f32; c.w * c.h];
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            lamp[j * c.w + i] = (-(((x - 0.52) / 0.55).powi(2) + ((y + 0.10) / 0.62).powi(2))).exp();
        }
    }
    c.add_field(&lamp, hex("#FFD9A0"), 0.16);
    for j in 0..c.h {
        let y = (j as f32 + 0.5) / c.h as f32;
        for i in 0..c.w {
            let x = (i as f32 + 0.5) / c.w as f32;
            lamp[j * c.w + i] = (-(((x - 0.52) / 0.20).powi(2) + ((y - 0.02) / 0.16).powi(2))).exp();
        }
    }
    c.add_field(&lamp, hex("#FFE9C8"), 0.10);
    // board in perspective
    let horizon = 0.335f32;
    let vpx = 0.50f32;
    let edge = |r: usize| -> (f32, f32) {
        let t = (r as f32 / 8.0).powf(1.55);
        (horizon + (1.02 - horizon) * t, 0.115 + 0.62 * t)
    };
    for r in 0..8 {
        let (y0, hw0) = edge(r);
        let (y1, hw1) = edge(r + 1);
        for cc in 0..8 {
            let x0a = vpx - hw0 + (2.0 * hw0) * cc as f32 / 8.0;
            let x1a = vpx - hw0 + (2.0 * hw0) * (cc + 1) as f32 / 8.0;
            let x0b = vpx - hw1 + (2.0 * hw1) * cc as f32 / 8.0;
            let x1b = vpx - hw1 + (2.0 * hw1) * (cc + 1) as f32 / 8.0;
            let light_sq = (r + cc) % 2 == 0;
            let base = if light_sq { hex("#DCC49E") } else { hex("#4E3120") };
            let m = mask_poly(c, &[(x0a, y0), (x1a, y0), (x1b, y1), (x0b, y1)], 0.45);
            c.blend(&m, base, 1.0);
            c.add(&m, hex("#FFF0D2"), 0.05 * (1.0 - (r as f32 / 8.0).powf(0.8)));
        }
    }
    c.add(&mask_rect(c, 0.0, 0.30, 1.0, 0.37, 14.0), hex("#000000"), 0.25);
    chess_piece(c, 0.34, 0.815, 0.115, true);
    chess_piece(c, 0.70, 0.875, 0.075, false);
    chess_piece(c, 0.545, 0.735, 0.048, false);
    // a fallen white pawn
    let (px, py) = (0.865f32, 0.93f32);
    c.add(&mask_disc(c, px, py + 0.02, 0.045, 0.012, 1.2), hex("#000000"), 0.4);
    c.blend(&mask_disc(c, px, py - 0.010, 0.030, 0.032, 0.7), hex("#E8D6B8"), 1.0);
    c.blend(&mask_rect(c, px - 0.032, py - 0.004, px + 0.032, py + 0.014, 0.7), hex("#E8D6B8"), 1.0);
    c.add(&mask_disc(c, px - 0.010, py - 0.020, 0.012, 0.010, 0.7), hex("#FFFFFF"), 0.35);
    vignette(c, 0.34);
    grain(c, 0.012);
}

