//! Skins: the colours of NikPak and unNik.
//!
//! The same skins as Nikkomander (src/skins_data.rs is generated from its
//! src/skins.ts by tools/gen_skins.py). A skin is a set of CSS variables; this
//! module turns them into the window's colours and paints the background
//! (`app-bg`: CSS gradients, stripes and dot patterns) into a texture, so a
//! skin looks the same in the three programs. Custom skins are Nikkomander's
//! `.skin.json` files, imported into the `skins` folder next to the settings.
//!
//! The chosen skin is saved in the shared settings file (`skin=<id>`): NikPak
//! and unNik use the same one, and a running window follows a change made in
//! the other program.

use crate::skins_data::{BUILTIN, EXTRA, TWEAKS};
use eframe::egui::{self, Color32, ColorImage, CornerRadius, Mesh, Pos2, Rect, Shape, TextureHandle, TextureOptions, Vec2};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

pub const DEFAULT: &str = "nikkoid";
pub const BRIGHT: &str = "nikkoid-light";

// ---------------------------------------------------------------------------
// The colours the windows use
// ---------------------------------------------------------------------------

/// Everything the windows draw with, taken from the skin's CSS variables.
#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub dark: bool,
    /// the original NikPak look (keeps unNik's own file colours)
    pub nikkoid: bool,
    pub bg: Color32,
    pub bg_top: Color32,
    pub bg_bottom: Color32,
    pub panel_top: Color32,
    pub panel_bottom: Color32,
    pub dialog_top: Color32,
    pub dialog_bottom: Color32,
    pub panel: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_hover: Color32,
    pub accent_fg: Color32,
    pub text: Color32,
    pub text_dim: Color32,
    pub track: Color32,
    pub ok: Color32,
    pub err: Color32,
    pub warn: Color32,
    pub cursor: Color32,
    pub border: Color32,
    pub dir: Color32,
    /// inset fields (destination, password, breadcrumbs)
    pub field: Color32,
    /// behind dialogs
    pub overlay: Color32,
    pub icon_dir: Color32,
    pub icon_arc: Color32,
    pub icon_img: Color32,
    pub icon_code: Color32,
    pub icon_media: Color32,
    pub icon_file: Color32,
    /// 0 = no glow (flat skins) .. 1 = NikPak's glow
    pub glow: f32,
    /// corner radius relative to Nikkoid's (0 = square corners)
    pub radius: f32,
}

pub struct Skin {
    pub id: String,
    pub name: String,
    pub dark: bool,
    pub custom: bool,
    pub c: Colors,
    /// `app-bg` (painted over `bg`)
    layers: Vec<Layer>,
    /// a background with large shapes (pizza): the window content gets a veil
    veil: bool,
    /// the background is a painted scene (`skin_scenes.rs`), not a gradient
    scene: bool,
}

// ---------------------------------------------------------------------------
// CSS values
// ---------------------------------------------------------------------------

/// Premultiplied RGBA, 0..1.
type Px = [f32; 4];

/// A CSS colour: #rgb, #rgba, #rrggbb, #rrggbbaa, rgb(), rgba(), a few names.
/// Returns straight (not premultiplied) RGBA, 0..1.
pub fn parse_color(s: &str) -> Option<[f32; 4]> {
    let s = s.trim().to_ascii_lowercase();
    let named = |n: &str| -> Option<&str> {
        Some(match n {
            "white" => "#ffffff",
            "black" => "#000000",
            "red" => "#ff0000",
            "green" => "#008000",
            "lime" => "#00ff00",
            "blue" => "#0000ff",
            "yellow" => "#ffff00",
            "orange" => "#ffa500",
            "gold" => "#ffd700",
            "pink" => "#ffc0cb",
            "purple" => "#800080",
            "navy" => "#000080",
            "silver" => "#c0c0c0",
            "gray" | "grey" => "#808080",
            "teal" => "#008080",
            _ => return None,
        })
    };
    if s == "transparent" {
        return Some([0.0; 4]);
    }
    if let Some(h) = named(&s) {
        return parse_color(h);
    }
    if let Some(hex) = s.strip_prefix('#') {
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let d = |i: usize, n: usize| u8::from_str_radix(&hex[i..i + n], 16).ok().map(|v| if n == 1 { v * 17 } else { v } as f32 / 255.0);
        return match hex.len() {
            3 => Some([d(0, 1)?, d(1, 1)?, d(2, 1)?, 1.0]),
            4 => Some([d(0, 1)?, d(1, 1)?, d(2, 1)?, d(3, 1)?]),
            6 => Some([d(0, 2)?, d(2, 2)?, d(4, 2)?, 1.0]),
            8 => Some([d(0, 2)?, d(2, 2)?, d(4, 2)?, d(6, 2)?]),
            _ => None,
        };
    }
    let inner = s.strip_prefix("rgba(").or_else(|| s.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(|c: char| c == ',' || c == '/' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 {
        return None;
    }
    let num = |p: &str, scale: f32| -> Option<f32> {
        match p.strip_suffix('%') {
            Some(v) => v.parse::<f32>().ok().map(|v| v / 100.0),
            None => p.parse::<f32>().ok().map(|v| v / scale),
        }
    };
    let a = if parts.len() > 3 { num(parts[3], 1.0)? } else { 1.0 };
    Some([num(parts[0], 255.0)?, num(parts[1], 255.0)?, num(parts[2], 255.0)?, a].map(|v| v.clamp(0.0, 1.0)))
}

fn premul(c: [f32; 4]) -> Px {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

fn to_c32(c: [f32; 4]) -> Color32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(b(c[0]), b(c[1]), b(c[2]), b(c[3]))
}

/// Split at `sep` outside parentheses.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c == sep && depth == 0 => {
                out.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out.retain(|p| !p.is_empty());
    out
}

fn split_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, None::<usize>);
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if c.is_whitespace() && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
        } else if c == '/' && depth == 0 {
            if let Some(st) = start.take() {
                out.push(&s[st..i]);
            }
            out.push("/");
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(&s[st..]);
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Len {
    Px(f32),
    Pct(f32),
}

impl Len {
    fn parse(s: &str) -> Option<Len> {
        let s = s.trim();
        Some(match s {
            "left" | "top" => Len::Pct(0.0),
            "center" => Len::Pct(50.0),
            "right" | "bottom" => Len::Pct(100.0),
            _ => {
                if let Some(v) = s.strip_suffix('%') {
                    Len::Pct(v.parse().ok()?)
                } else if let Some(v) = s.strip_suffix("px") {
                    Len::Px(v.parse().ok()?)
                } else if let Some(v) = s.strip_suffix("deg") {
                    // angles of conic gradients: a share of the turn
                    Len::Pct(v.parse::<f32>().ok()? / 3.6)
                } else if let Some(v) = s.strip_suffix("turn") {
                    Len::Pct(v.parse::<f32>().ok()? * 100.0)
                } else {
                    let v: f32 = s.parse().ok()?;
                    Len::Px(v)
                }
            }
        })
    }
    fn of(self, full: f32) -> f32 {
        match self {
            Len::Px(v) => v,
            Len::Pct(p) => p / 100.0 * full,
        }
    }
}

fn parse_angle(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("deg") {
        return v.parse().ok();
    }
    if let Some(v) = s.strip_suffix("turn") {
        return v.parse::<f32>().ok().map(|v| v * 360.0);
    }
    if let Some(v) = s.strip_suffix("rad") {
        return v.parse::<f32>().ok().map(f32::to_degrees);
    }
    if let Some(dir) = s.strip_prefix("to ") {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        for w in dir.split_whitespace() {
            match w {
                "top" => y = -1.0,
                "bottom" => y = 1.0,
                "left" => x = -1.0,
                "right" => x = 1.0,
                _ => return None,
            }
        }
        return Some(x.atan2(-y).to_degrees());
    }
    None
}

#[derive(Clone, Debug)]
struct Stop {
    col: Px,
    pos: Option<Len>,
}

#[derive(Clone, Copy, Debug)]
enum RSize {
    ClosestSide,
    FarthestSide,
    ClosestCorner,
    FarthestCorner,
    Circle(f32),
    Ellipse(Len, Len),
}

#[derive(Clone, Debug)]
enum Kind {
    Linear { angle: f32 },
    Radial { circle: bool, size: RSize, at: (Len, Len) },
    Conic { from: f32, at: (Len, Len) },
}

#[derive(Clone, Debug)]
enum Image {
    Color(Px),
    Gradient { kind: Kind, stops: Vec<Stop>, repeating: bool },
}

#[derive(Clone, Debug)]
struct Layer {
    image: Image,
    pos: (Len, Len),
    size: Option<(Option<Len>, Option<Len>)>,
    repeat: (bool, bool),
}

fn parse_position(toks: &[&str]) -> Option<(Len, Len)> {
    match toks {
        [] => Some((Len::Pct(50.0), Len::Pct(50.0))),
        [a] => {
            let l = Len::parse(a)?;
            Some(if matches!(*a, "top" | "bottom") { (Len::Pct(50.0), l) } else { (l, Len::Pct(50.0)) })
        }
        [a, b, ..] => {
            // "top left" is allowed too
            if matches!(*a, "top" | "bottom") || matches!(*b, "left" | "right") {
                Some((Len::parse(b)?, Len::parse(a)?))
            } else {
                Some((Len::parse(a)?, Len::parse(b)?))
            }
        }
    }
}

fn parse_stop(s: &str) -> Option<Vec<Stop>> {
    let toks = split_ws(s);
    let col = premul(parse_color(toks.first()?)?);
    let poss: Vec<Len> = toks[1..].iter().filter_map(|t| Len::parse(t)).collect();
    Some(match poss.as_slice() {
        [] => vec![Stop { col, pos: None }],
        [a] => vec![Stop { col, pos: Some(*a) }],
        [a, b, ..] => vec![Stop { col, pos: Some(*a) }, Stop { col, pos: Some(*b) }],
    })
}

fn parse_gradient(name: &str, inner: &str) -> Option<Image> {
    let repeating = name.starts_with("repeating-");
    let base = name.trim_start_matches("repeating-");
    let args = split_top(inner, ',');
    let mut stops = Vec::new();
    let mut first_is_config = false;
    if let Some(a0) = args.first() {
        first_is_config = parse_stop(a0).is_none();
    }
    for a in args.iter().skip(first_is_config as usize) {
        stops.extend(parse_stop(a)?);
    }
    if stops.is_empty() {
        return None;
    }
    let cfg: Vec<&str> = if first_is_config { split_ws(args[0]) } else { Vec::new() };
    let kind = match base {
        "linear-gradient" => Kind::Linear { angle: if first_is_config { parse_angle(args[0])? } else { 180.0 } },
        "radial-gradient" => {
            let at_i = cfg.iter().position(|t| *t == "at");
            let (shape, at) = match at_i {
                Some(i) => (&cfg[..i], parse_position(&cfg[i + 1..])?),
                None => (&cfg[..], (Len::Pct(50.0), Len::Pct(50.0))),
            };
            let mut circle = shape.contains(&"circle");
            let lens: Vec<Len> = shape.iter().filter_map(|t| if matches!(*t, "circle" | "ellipse") { None } else { Len::parse(t) }).collect();
            let kw = shape.iter().find(|t| t.contains("-side") || t.contains("-corner"));
            let size = match (lens.as_slice(), kw) {
                ([a, b, ..], _) => RSize::Ellipse(*a, *b),
                ([Len::Px(r)], _) => {
                    circle = true;
                    RSize::Circle(*r)
                }
                (_, Some(&"closest-side")) => RSize::ClosestSide,
                (_, Some(&"farthest-side")) => RSize::FarthestSide,
                (_, Some(&"closest-corner")) => RSize::ClosestCorner,
                _ => RSize::FarthestCorner,
            };
            Kind::Radial { circle, size, at }
        }
        "conic-gradient" => {
            let mut from = 0.0;
            let mut at = (Len::Pct(50.0), Len::Pct(50.0));
            let mut i = 0;
            while i < cfg.len() {
                match cfg[i] {
                    "from" if i + 1 < cfg.len() => {
                        from = parse_angle(cfg[i + 1])?;
                        i += 2;
                    }
                    "at" => {
                        let end = cfg[i + 1..].iter().position(|t| *t == "from").map_or(cfg.len(), |k| i + 1 + k);
                        at = parse_position(&cfg[i + 1..end])?;
                        i = end;
                    }
                    _ => i += 1,
                }
            }
            Kind::Conic { from, at }
        }
        _ => return None,
    };
    Some(Image::Gradient { kind, stops, repeating })
}

fn parse_layer(s: &str) -> Option<Layer> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    let image;
    let rest;
    if let Some(open) = lower.find('(').filter(|&i| lower[..i].ends_with("-gradient")) {
        let name = lower[..open].trim();
        let mut depth = 0;
        let mut close = None;
        for (i, c) in lower[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(open + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let close = close?;
        image = parse_gradient(name, &lower[open + 1..close])?;
        rest = lower[close + 1..].to_string();
    } else {
        // a plain colour (the last layer of a background)
        return Some(Layer {
            image: Image::Color(premul(parse_color(&lower)?)),
            pos: (Len::Px(0.0), Len::Px(0.0)),
            size: None,
            repeat: (true, true),
        });
    }
    let toks = split_ws(&rest);
    let mut repeat = (true, true);
    let mut pos_t = Vec::new();
    let mut size_t = Vec::new();
    let mut after_slash = false;
    for t in toks {
        match t {
            "/" => after_slash = true,
            "repeat-x" => repeat = (true, false),
            "repeat-y" => repeat = (false, true),
            "no-repeat" => repeat = (false, false),
            "repeat" => repeat = (true, true),
            _ if after_slash => size_t.push(t),
            _ => pos_t.push(t),
        }
    }
    let pos = if pos_t.is_empty() { (Len::Px(0.0), Len::Px(0.0)) } else { parse_position(&pos_t)? };
    let size = if size_t.is_empty() {
        None
    } else {
        let one = |t: &str| if t == "auto" { None } else { Len::parse(t) };
        Some((one(size_t[0]), size_t.get(1).and_then(|t| one(t))))
    };
    Some(Layer { image, pos, size, repeat })
}

fn parse_background(css: &str) -> Vec<Layer> {
    split_top(css, ',').into_iter().filter_map(parse_layer).collect()
}

/// The first and the last colour of a background's bottom layer (a vertical
/// gradient for panels and dialogs).
fn ends(css: &str) -> Option<([f32; 4], [f32; 4])> {
    let layers = parse_background(css);
    let last = layers.last()?;
    let un = |p: Px| if p[3] > 0.0 { [p[0] / p[3], p[1] / p[3], p[2] / p[3], p[3]] } else { [0.0; 4] };
    match &last.image {
        Image::Color(c) => Some((un(*c), un(*c))),
        Image::Gradient { stops, .. } => Some((un(stops.first()?.col), un(stops.last()?.col))),
    }
}

// ---------------------------------------------------------------------------
// Painting a background (CPU, into a texture)
// ---------------------------------------------------------------------------

struct Ramp {
    pos: Vec<f32>,
    col: Vec<Px>,
    repeating: bool,
}

impl Ramp {
    /// CSS colour-stop fix-up; `len` turns pixel positions into 0..1.
    fn new(stops: &[Stop], len: f32, repeating: bool) -> Ramp {
        let n = stops.len();
        let mut pos: Vec<Option<f32>> = stops
            .iter()
            .map(|s| s.pos.map(|p| match p {
                Len::Px(v) => if len > 0.0 { v / len } else { 0.0 },
                Len::Pct(v) => v / 100.0,
            }))
            .collect();
        if pos[0].is_none() {
            pos[0] = Some(0.0);
        }
        if pos[n - 1].is_none() {
            pos[n - 1] = Some(1.0);
        }
        let mut max = f32::MIN;
        for p in pos.iter_mut().flatten() {
            if *p < max {
                *p = max;
            }
            max = *p;
        }
        let mut i = 0;
        while i < n {
            if pos[i].is_none() {
                let a = i - 1;
                let mut b = i;
                while pos[b].is_none() {
                    b += 1;
                }
                let (pa, pb) = (pos[a].unwrap(), pos[b].unwrap());
                for k in a + 1..b {
                    pos[k] = Some(pa + (pb - pa) * (k - a) as f32 / (b - a) as f32);
                }
                i = b;
            }
            i += 1;
        }
        Ramp { pos: pos.into_iter().map(|p| p.unwrap()).collect(), col: stops.iter().map(|s| s.col).collect(), repeating }
    }

    fn at(&self, mut t: f32) -> Px {
        let n = self.pos.len();
        if self.repeating {
            let (a, b) = (self.pos[0], self.pos[n - 1]);
            if b - a > 1e-6 {
                t = a + (t - a).rem_euclid(b - a);
            }
        }
        if t <= self.pos[0] {
            return self.col[0];
        }
        if t >= self.pos[n - 1] {
            return self.col[n - 1];
        }
        for i in 0..n - 1 {
            let (a, b) = (self.pos[i], self.pos[i + 1]);
            if t >= a && t < b {
                let f = (t - a) / (b - a);
                let (x, y) = (self.col[i], self.col[i + 1]);
                return [x[0] + (y[0] - x[0]) * f, x[1] + (y[1] - x[1]) * f, x[2] + (y[2] - x[2]) * f, x[3] + (y[3] - x[3]) * f];
            }
        }
        self.col[n - 1]
    }
}

/// A layer ready to be painted on a box of a given size.
struct Prepared<'a> {
    layer: &'a Layer,
    tile: (f32, f32),
    origin: (f32, f32),
    ramp: Option<Ramp>,
    geo: [f32; 6],
}

fn prepare(layer: &Layer, w: f32, h: f32) -> Prepared<'_> {
    let (tw, th) = match layer.size {
        None => (w, h),
        Some((a, b)) => (a.map_or(w, |l| l.of(w)), b.map_or(h, |l| l.of(h))),
    };
    let (tw, th) = (tw.max(0.5), th.max(0.5));
    let origin = (
        match layer.pos.0 {
            Len::Px(v) => v,
            Len::Pct(p) => (w - tw) * p / 100.0,
        },
        match layer.pos.1 {
            Len::Px(v) => v,
            Len::Pct(p) => (h - th) * p / 100.0,
        },
    );
    let mut geo = [0.0; 6];
    let ramp = match &layer.image {
        Image::Color(_) => None,
        Image::Gradient { kind, stops, repeating } => {
            let len = match kind {
                Kind::Linear { angle } => {
                    let a = angle.to_radians();
                    let (s, c) = (a.sin(), -a.cos());
                    let l = (tw * s).abs() + (th * c).abs();
                    geo = [s, c, l.max(1e-3), 0.0, 0.0, 0.0];
                    l
                }
                Kind::Radial { circle, size, at } => {
                    let (cx, cy) = (at.0.of(tw), at.1.of(th));
                    let (l, r, t, b) = (cx.abs(), (tw - cx).abs(), cy.abs(), (th - cy).abs());
                    let corner = |far: bool| {
                        let dx = if (l > r) == far { l } else { r };
                        let dy = if (t > b) == far { t } else { b };
                        (dx, dy)
                    };
                    let (rx, ry) = match size {
                        RSize::Circle(r) => (*r, *r),
                        RSize::Ellipse(a, b) => (a.of(tw), b.of(th)),
                        RSize::ClosestSide if *circle => (l.min(r).min(t).min(b), l.min(r).min(t).min(b)),
                        RSize::FarthestSide if *circle => (l.max(r).max(t).max(b), l.max(r).max(t).max(b)),
                        RSize::ClosestSide => (l.min(r), t.min(b)),
                        RSize::FarthestSide => (l.max(r), t.max(b)),
                        RSize::ClosestCorner | RSize::FarthestCorner => {
                            let (dx, dy) = corner(matches!(size, RSize::FarthestCorner));
                            if *circle {
                                let d = dx.hypot(dy);
                                (d, d)
                            } else {
                                // the aspect ratio of closest-side, through the corner
                                let ratio = if t.min(b) > 0.0 { l.min(r) / t.min(b) } else { 1.0 };
                                let rx = dx.hypot(dy * ratio);
                                (rx, if ratio > 0.0 { rx / ratio } else { rx })
                            }
                        }
                    };
                    geo = [cx, cy, rx.max(1e-3), ry.max(1e-3), 0.0, 0.0];
                    rx.max(1e-3)
                }
                Kind::Conic { from, at } => {
                    geo = [at.0.of(tw), at.1.of(th), *from, 0.0, 0.0, 0.0];
                    1.0
                }
            };
            Some(Ramp::new(stops, len, *repeating))
        }
    };
    Prepared { layer, tile: (tw, th), origin, ramp, geo }
}

impl Prepared<'_> {
    fn sample(&self, x: f32, y: f32) -> Option<Px> {
        let (tw, th) = self.tile;
        let mut lx = x - self.origin.0;
        let mut ly = y - self.origin.1;
        if self.layer.repeat.0 {
            lx = lx.rem_euclid(tw);
        } else if lx < 0.0 || lx >= tw {
            return None;
        }
        if self.layer.repeat.1 {
            ly = ly.rem_euclid(th);
        } else if ly < 0.0 || ly >= th {
            return None;
        }
        let g = &self.geo;
        match &self.layer.image {
            Image::Color(c) => Some(*c),
            Image::Gradient { kind, .. } => {
                let t = match kind {
                    Kind::Linear { .. } => ((lx - tw / 2.0) * g[0] + (ly - th / 2.0) * g[1]) / g[2] + 0.5,
                    Kind::Radial { .. } => ((lx - g[0]) / g[2]).hypot((ly - g[1]) / g[3]),
                    Kind::Conic { .. } => {
                        let a = (lx - g[0]).atan2(-(ly - g[1])).to_degrees() - g[2];
                        a.rem_euclid(360.0) / 360.0
                    }
                };
                Some(self.ramp.as_ref()?.at(t))
            }
        }
    }
}

/// Paint a skin's background on a box of `w` x `h` points, `scale` pixels per point.
/// Fairly generous: the artwork of a painted skin is a background for the
/// interface, never the other way round.
const VEIL_BASE: f32 = 0.58;
const VEIL_EXTRA: f32 = 0.30;
const VEIL_TOP: f32 = 0.12;
const ART_LOCAL: f32 = 0.32;
const ART_MEAN_R: usize = 16;

/// Make a painted background discreet, so the text, the ring and the icons
/// stay perfectly readable on top of it: the artwork is flattened toward its
/// own local mean (big lettering and neon signs become soft washes) and then
/// faded toward the skin's background colour, most strongly behind the drop
/// ring and the text column and a little more over the logo.
fn discreet(img: ColorImage, bg: Color32, scale: f32) -> ColorImage {
    let w = img.size[0];
    let h = img.size[1];
    let mut px: Vec<[f32; 3]> = img.pixels.iter().map(|c| [c.r() as f32, c.g() as f32, c.b() as f32]).collect();
    // 1. local mean (separable box blur, radius ART_MEAN_R)
    {
        // the mean radius is in points, so it keeps its effect at any DPI; it
        // must stay small next to the picture itself, or the little tiles of
        // the skins menu would be flattened into one colour
        let r = ((ART_MEAN_R as f32 * scale).round() as usize).max(2).min((w.min(h) / 8).max(2));
        let mut tmp = vec![[0.0f32; 3]; px.len()];
        for j in 0..h {
            for i in 0..w {
                let mut acc = [0.0f32; 3];
                let mut n = 0.0f32;
                for k in -(r as isize)..=(r as isize) {
                    let x = (i as isize + k).clamp(0, w as isize - 1) as usize;
                    let c = px[j * w + x];
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                    n += 1.0;
                }
                tmp[j * w + i] = [acc[0] / n, acc[1] / n, acc[2] / n];
            }
        }
        for i in 0..w {
            for j in 0..h {
                let mut acc = [0.0f32; 3];
                let mut n = 0.0f32;
                for k in -(r as isize)..=(r as isize) {
                    let y = (j as isize + k).clamp(0, h as isize - 1) as usize;
                    let c = tmp[y * w + i];
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                    n += 1.0;
                }
                let mean = [acc[0] / n, acc[1] / n, acc[2] / n];
                let c = px[j * w + i];
                px[j * w + i] = [
                    mean[0] + (c[0] - mean[0]) * ART_LOCAL,
                    mean[1] + (c[1] - mean[1]) * ART_LOCAL,
                    mean[2] + (c[2] - mean[2]) * ART_LOCAL,
                ];
            }
        }
    }
    // 2. a little blur, so nothing reads as text
    let blur_pass = |src: &Vec<[f32; 3]>| -> Vec<[f32; 3]> {
        let mut out = vec![[0.0f32; 3]; src.len()];
        for j in 0..h {
            for i in 0..w {
                let mut acc = [0.0f32; 3];
                let mut n = 0.0f32;
                for (dy, dx) in [(0i32, 0i32), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let y = (j as i32 + dy).clamp(0, h as i32 - 1) as usize;
                    let x = (i as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    let c = src[y * w + x];
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                    n += 1.0;
                }
                out[j * w + i] = [acc[0] / n, acc[1] / n, acc[2] / n];
            }
        }
        out
    };
    px = blur_pass(&px);
    px = blur_pass(&px);
    // 3. the veil
    let b = [bg.r() as f32, bg.g() as f32, bg.b() as f32];
    for j in 0..h {
        let y = (j as f32 + 0.5) / h as f32;
        let top = VEIL_TOP * ((0.22 - y) / 0.22).clamp(0.0, 1.0);
        for i in 0..w {
            let x = (i as f32 + 0.5) / w as f32;
            let dx = (x - 0.5) / 0.52;
            let dy = (y - 0.42) / 0.55;
            let r = (dx * dx + dy * dy).sqrt();
            let k = VEIL_BASE + VEIL_EXTRA * (1.0 - r).clamp(0.0, 1.0).powf(1.2) + top;
            let c = &mut px[j * w + i];
            for ch in 0..3 {
                c[ch] = c[ch] * (1.0 - k) + b[ch] * k;
            }
        }
    }
    let v = |f: f32| (f.clamp(0.0, 255.0) + 0.5) as u8;
    ColorImage {
        size: [w, h],
        pixels: px.iter().map(|c| Color32::from_rgb(v(c[0]), v(c[1]), v(c[2]))).collect(),
        source_size: img.source_size,
    }
}

fn paint(s: &Skin, w: f32, h: f32, scale: f32) -> ColorImage {
    let (pw, ph) = ((w * scale).round().max(1.0) as usize, (h * scale).round().max(1.0) as usize);
    if s.scene {
        if let Some(img) = crate::skin_scenes::paint(&s.id, pw, ph) {
            return discreet(img, s.c.bg, scale);
        }
    }
    let prepared: Vec<Prepared> = s.layers.iter().rev().map(|l| prepare(l, w, h)).collect();
    let base = s.c.bg;
    let b = [base.r() as f32 / 255.0, base.g() as f32 / 255.0, base.b() as f32 / 255.0, 1.0];
    let mut pixels = Vec::with_capacity(pw * ph);
    for py in 0..ph {
        let y = (py as f32 + 0.5) / scale;
        for px in 0..pw {
            let x = (px as f32 + 0.5) / scale;
            let mut c = b;
            for l in &prepared {
                if let Some(s) = l.sample(x, y) {
                    let k = 1.0 - s[3];
                    c = [s[0] + c[0] * k, s[1] + c[1] * k, s[2] + c[2] * k, 1.0];
                }
            }
            let v = |f: f32| (f.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            pixels.push(Color32::from_rgb(v(c[0]), v(c[1]), v(c[2])));
        }
    }
    ColorImage { size: [pw, ph], pixels, source_size: Vec2::new(pw as f32, ph as f32) }
}

thread_local! {
    static TEXTURES: RefCell<HashMap<(String, [u32; 3]), TextureHandle>> = RefCell::new(HashMap::new());
}

fn texture(ctx: &egui::Context, s: &Skin, size: Vec2) -> TextureHandle {
    let ppp = ctx.pixels_per_point();
    let key = (s.id.clone(), [size.x.round() as u32, size.y.round() as u32, (ppp * 100.0) as u32]);
    TEXTURES.with(|t| {
        let mut t = t.borrow_mut();
        if let Some(h) = t.get(&key) {
            return h.clone();
        }
        if t.len() > 80 {
            t.clear();
        }
        let img = paint(s, size.x, size.y, ppp);
        let h = ctx.load_texture(format!("skin-{}-{}x{}", key.0, key.1[0], key.1[1]), img, TextureOptions::LINEAR);
        t.insert(key, h.clone());
        h
    })
}

/// Points of a rounded rectangle (corners of `radius`).
pub(crate) fn rounded_points(r: Rect, radius: f32) -> Vec<Pos2> {
    let radius = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    let mut pts = Vec::new();
    let corners = [
        (Pos2::new(r.right() - radius, r.top() + radius), -90.0f32),
        (Pos2::new(r.right() - radius, r.bottom() - radius), 0.0),
        (Pos2::new(r.left() + radius, r.bottom() - radius), 90.0),
        (Pos2::new(r.left() + radius, r.top() + radius), 180.0),
    ];
    for (c, a0) in corners {
        let steps = if radius < 0.5 { 0 } else { 10 };
        for k in 0..=steps {
            let a = (a0 + 90.0 / 10.0 * k as f32).to_radians();
            pts.push(c + Vec2::new(a.cos(), a.sin()) * radius);
        }
    }
    pts
}

/// The skin's background (`app-bg`) in a rounded rectangle.
pub fn paint_background(ctx: &egui::Context, p: &egui::Painter, r: Rect, radius: f32) {
    let s = current();
    paint_skin_background(ctx, p, &s, r, radius);
    if s.veil {
        let a = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 200);
        crate::gui::gradient_panel_px(p, r.shrink(9.0), (radius - 6.0).max(0.0), a(s.c.panel_top), a(s.c.panel_bottom));
    }
}

/// Any skin's background (the previews in the skin menu).
pub fn paint_skin_background(ctx: &egui::Context, p: &egui::Painter, s: &Skin, r: Rect, radius: f32) {
    let tex = texture(ctx, s, r.size());
    let mut mesh = Mesh::with_texture(tex.id());
    let uv = |q: Pos2| Pos2::new((q.x - r.left()) / r.width(), (q.y - r.top()) / r.height());
    mesh.vertices.push(egui::epaint::Vertex { pos: r.center(), uv: uv(r.center()), color: Color32::WHITE });
    let pts = rounded_points(r, radius);
    for q in &pts {
        mesh.vertices.push(egui::epaint::Vertex { pos: *q, uv: uv(*q), color: Color32::WHITE });
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

// ---------------------------------------------------------------------------
// Building a skin from its CSS variables
// ---------------------------------------------------------------------------

/// Nikkomander's defaults (styles/base.css :root) for variables a skin leaves out.
const CSS_DEFAULTS: &[(&str, &str)] = &[
    ("radius", "10px"),
    ("bg", "#0b151c"),
    ("bg-alt", "#0f1d26"),
    ("panel", "#112430"),
    ("panel-alt", "#142a37"),
    ("fg", "#dcd5c4"),
    ("fg-dim", "#7f949f"),
    ("accent", "#3fb8a6"),
    ("accent-fg", "#06201c"),
    ("cursor", "#1d5e6e"),
    ("marked", "#f2b84b"),
    ("dir", "#8fd3e6"),
    ("border", "#1f3a49"),
    ("dialog", "#122733"),
    ("danger", "#ef6f6c"),
    ("ok", "#5ed3a0"),
    ("icon-dir", "#6cc4dd"),
    ("icon-arc", "#e0a24a"),
    ("icon-img", "#b58be0"),
    ("icon-code", "#6fd1a0"),
    ("icon-media", "#e27a8b"),
    ("icon-file", "#8fa2ad"),
    ("glow", "transparent"),
];

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

/// Darken an opaque colour toward black by the fraction `t` (0..1), keeping the
/// gamma-encoded sRGB values — the same interpolation as the website's
/// `color-mix(in srgb, <colour> (1-t)%, #000)`. Used for filled (primary) buttons.
fn toward_black(c: Color32, t: f32) -> Color32 {
    let f = 1.0 - t;
    Color32::from_rgb(
        (c.r() as f32 * f).round() as u8,
        (c.g() as f32 * f).round() as u8,
        (c.b() as f32 * f).round() as u8,
    )
}

impl Colors {
    /// Filled (primary) button colour — the "Register" button and its siblings. The accent
    /// darkened toward black, matching the website's `color-mix(in srgb, var(--accent) 62%, #000)`.
    pub fn accent_btn(&self) -> Color32 {
        toward_black(self.accent, 0.38)
    }
    /// Hover state of a filled button (72% accent, 28% black).
    pub fn accent_btn_hover(&self) -> Color32 {
        toward_black(self.accent, 0.28)
    }
}

impl Skin {
    fn from_vars(id: &str, name: &str, dark: bool, custom: bool, vars: &HashMap<String, String>) -> Skin {
        let get = |k: &str| -> Option<&str> { vars.get(k).map(|s| s.as_str()).or_else(|| CSS_DEFAULTS.iter().find(|d| d.0 == k).map(|d| d.1)) };
        let col = |k: &str| -> [f32; 4] { get(k).and_then(parse_color).or_else(|| CSS_DEFAULTS.iter().find(|d| d.0 == k).and_then(|d| parse_color(d.1))).unwrap_or([0.5, 0.5, 0.5, 1.0]) };
        let col_or = |k: &str, f: &str| -> [f32; 4] { vars.get(k).and_then(|v| parse_color(v)).unwrap_or_else(|| col(f)) };
        let bg = col("bg");
        let app_bg = vars.get("app-bg").cloned().unwrap_or_default();
        let layers = parse_background(&app_bg);
        // a vertical gradient that sums up the background (used where a texture cannot be)
        let (bg_top, bg_bottom) = ends(&app_bg).filter(|(a, b)| a[3] > 0.99 && b[3] > 0.99).unwrap_or((col_or("bg-alt", "bg"), bg));
        let panel = col("panel");
        let (panel_top, panel_bottom) = vars.get("panel-bg").and_then(|v| ends(v)).unwrap_or((panel, panel));
        let dialog = col("dialog");
        let (dialog_top, dialog_bottom) = vars.get("dialog-bg").and_then(|v| ends(v)).unwrap_or((dialog, dialog));
        let accent = col("accent");
        let glow = get("glow").and_then(parse_color).map_or(0.0, |g| (g[3] / 0.55).min(1.0));
        let radius = get("radius").and_then(Len::parse).map_or(10.0, |l| l.of(0.0)) / 14.0;
        let black = [0.0, 0.0, 0.0, 1.0];
        let field = if dark { mix(bg, black, 0.45) } else { col("panel") };
        let overlay = if dark { mix(bg, black, 0.6) } else { mix(bg, [1.0, 1.0, 1.0, 1.0], 0.3) };
        let mut c = Colors {
            dark,
            nikkoid: id == DEFAULT,
            bg: to_c32(bg),
            bg_top: to_c32(bg_top),
            bg_bottom: to_c32(bg_bottom),
            panel_top: to_c32(panel_top),
            panel_bottom: to_c32(panel_bottom),
            dialog_top: to_c32(dialog_top),
            dialog_bottom: to_c32(dialog_bottom),
            panel: to_c32(panel),
            accent: to_c32(accent),
            accent_soft: to_c32(col_or("accent-soft", "accent")),
            accent_hover: to_c32(col_or("accent-hover", "accent")),
            accent_fg: to_c32(col("accent-fg")),
            text: to_c32(col("fg")),
            text_dim: to_c32(col("fg-dim")),
            track: to_c32(col_or("track", "border")),
            ok: to_c32(col("ok")),
            err: to_c32(col("danger")),
            warn: to_c32(col("marked")),
            cursor: to_c32(col("cursor")),
            border: to_c32(col("border")),
            dir: to_c32(col("dir")),
            field: to_c32(field),
            overlay: to_c32(overlay),
            icon_dir: to_c32(col("icon-dir")),
            icon_arc: to_c32(col("icon-arc")),
            icon_img: to_c32(col("icon-img")),
            icon_code: to_c32(col("icon-code")),
            icon_media: to_c32(col("icon-media")),
            icon_file: to_c32(col("icon-file")),
            glow,
            radius,
        };
        if c.nikkoid {
            // exactly NikPak's original look
            c.field = Color32::from_rgb(0x02, 0x08, 0x1A);
            c.overlay = Color32::from_rgb(0x01, 0x04, 0x0E);
            c.warn = Color32::from_rgb(0xFF, 0xC4, 0x5C);
        }
        // large opaque shapes repeated all over the background would hide the text
        let veil = layers.iter().any(|l| {
            l.size.is_some()
                && matches!(&l.image, Image::Gradient { kind: Kind::Radial { .. }, stops, .. }
                    if stops.iter().any(|st| st.col[3] > 0.99 && matches!(st.pos, Some(Len::Px(v)) if v >= 8.0)))
        });
        let scene = crate::skin_scenes::has_scene(id);
        Skin { id: id.to_string(), name: name.to_string(), dark, custom, c, layers, veil, scene }
    }
}

// ---------------------------------------------------------------------------
// Custom skins (Nikkomander's .skin.json)
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl JsonParser<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        let c = *self.s.get(self.i).ok_or("unexpected end")?;
        match c {
            b'{' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(v));
                    }
                    let Json::Str(k) = self.value()? else { return Err("expected a key".into()) };
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') {
                        return Err("expected ':'".into());
                    }
                    self.i += 1;
                    let val = self.value()?;
                    v.push((k, val));
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {}
                        _ => return Err("expected ',' or '}'".into()),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(v));
                    }
                    v.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {}
                        _ => return Err("expected ',' or ']'".into()),
                    }
                }
            }
            b'"' => {
                self.i += 1;
                let mut out = String::new();
                let mut buf = Vec::new();
                loop {
                    let c = *self.s.get(self.i).ok_or("unterminated string")?;
                    self.i += 1;
                    match c {
                        b'"' => break,
                        b'\\' => {
                            let e = *self.s.get(self.i).ok_or("bad escape")?;
                            self.i += 1;
                            match e {
                                b'n' => buf.push(b'\n'),
                                b't' => buf.push(b'\t'),
                                b'r' => buf.push(b'\r'),
                                b'b' | b'f' => {}
                                b'u' => {
                                    let hex = std::str::from_utf8(self.s.get(self.i..self.i + 4).ok_or("bad escape")?).map_err(|_| "bad escape")?;
                                    let cp = u32::from_str_radix(hex, 16).map_err(|_| "bad escape")?;
                                    self.i += 4;
                                    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                                    let mut b = [0u8; 4];
                                    buf.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                                }
                                other => buf.push(other),
                            }
                        }
                        other => buf.push(other),
                    }
                }
                out.push_str(&String::from_utf8_lossy(&buf));
                Ok(Json::Str(out))
            }
            b't' if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(Json::Bool(true))
            }
            b'f' if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(Json::Bool(false))
            }
            b'n' if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(Json::Null)
            }
            _ => {
                let st = self.i;
                while self.i < self.s.len() && (self.s[self.i].is_ascii_digit() || b"+-.eE".contains(&self.s[self.i])) {
                    self.i += 1;
                }
                std::str::from_utf8(&self.s[st..self.i]).ok().and_then(|n| n.parse().ok()).map(Json::Num).ok_or_else(|| "not JSON".to_string())
            }
        }
    }
}

fn get<'a>(o: &'a [(String, Json)], k: &str) -> Option<&'a Json> {
    o.iter().find(|e| e.0 == k).map(|e| &e.1)
}

/// Read a Nikkomander skin file. Custom skins are layered over the default
/// skin (like Nikkomander does), so a partial skin still looks right.
fn parse_skin_json(text: &str) -> Result<Skin, String> {
    let text = text.trim_start_matches('\u{FEFF}');
    let mut p = JsonParser { s: text.as_bytes(), i: 0 };
    let Json::Obj(o) = p.value()? else { return Err("a skin is a JSON object".into()) };
    let Some(Json::Obj(vars)) = get(&o, "vars") else { return Err("the skin needs a \"vars\" object".into()) };
    let name = match get(&o, "name") {
        Some(Json::Str(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => "Custom skin".to_string(),
    };
    let slug: String = name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let id = match get(&o, "id") {
        Some(Json::Str(s)) if !s.trim().is_empty() => s.trim().to_string(),
        _ => format!("custom-{}", slug.trim_matches('-')),
    };
    let dark = !matches!(get(&o, "dark"), Some(Json::Bool(false)));
    let mut own = HashMap::new();
    for (k, v) in vars {
        let v = match v {
            Json::Str(s) => s.clone(),
            Json::Num(n) => n.to_string(),
            _ => continue,
        };
        own.insert(k.trim_start_matches("--").to_string(), v);
    }
    let mut merged: HashMap<String, String> = builtin_vars(DEFAULT);
    if own.contains_key("bg") {
        merged.remove("app-bg");
    }
    if own.contains_key("panel") {
        merged.remove("panel-bg");
    }
    if own.contains_key("dialog") {
        merged.remove("dialog-bg");
    }
    merged.extend(own);
    Ok(Skin::from_vars(&id, &name, dark, true, &merged))
}

fn builtin_vars(id: &str) -> HashMap<String, String> {
    BUILTIN.iter().find(|b| b.0 == id).map(|b| b.3.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()).unwrap_or_default()
}

/// Folder of the imported skins (next to the settings file).
pub fn custom_dir() -> Option<PathBuf> {
    crate::gui::settings_file()?.parent().map(|d| d.join("skins"))
}

fn load_custom() -> Vec<Skin> {
    let Some(dir) = custom_dir() else { return Vec::new() };
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<Skin> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter_map(|e| parse_skin_json(&std::fs::read_to_string(e.path()).ok()?).ok())
        .filter(|s| !is_builtin(&s.id))
        .collect();
    v.sort_by_key(|s| s.name.to_lowercase());
    v.dedup_by(|a, b| a.id == b.id);
    v
}

fn custom_file(id: &str) -> Option<PathBuf> {
    let safe: String = id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    custom_dir().map(|d| d.join(format!("{safe}.skin.json")))
}

// ---------------------------------------------------------------------------
// The current skin
// ---------------------------------------------------------------------------

struct State {
    skins: Vec<Arc<Skin>>,
    chosen: Arc<Skin>,
    preview: Option<Arc<Skin>>,
    colors: Colors,
}

static STATE: RwLock<Option<State>> = RwLock::new(None);
static VERSION: AtomicU64 = AtomicU64::new(1);

fn is_builtin(id: &str) -> bool {
    BUILTIN.iter().chain(EXTRA.iter()).any(|b| b.0 == id)
}

fn builtin_skins() -> Vec<Arc<Skin>> {
    BUILTIN
        .iter()
        .chain(EXTRA.iter())
        .map(|(id, name, dark, vars)| {
            let mut v: HashMap<String, String> = vars.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            // colour corrections for skins whose picture the generated
            // palette no longer suits (see TWEAKS in skins_data.rs)
            for &(sid, k, val) in TWEAKS.iter() {
                if sid == *id {
                    v.insert(k.to_string(), val.to_string());
                }
            }
            Arc::new(Skin::from_vars(id, name, *dark, false, &v))
        })
        .collect()
}

fn init() {
    if STATE.read().unwrap().is_some() {
        return;
    }
    let mut skins = builtin_skins();
    skins.extend(load_custom().into_iter().map(Arc::new));
    let wanted = std::env::var("NIKPAK_GUI_SKIN").ok().or_else(|| crate::gui::read_setting("skin")).unwrap_or_else(|| DEFAULT.into());
    let chosen = skins.iter().find(|s| s.id == wanted).unwrap_or(&skins[0]).clone();
    let colors = chosen.c;
    let mut st = STATE.write().unwrap();
    if st.is_none() {
        *st = Some(State { skins, chosen, preview: None, colors });
    }
}

fn with<R>(f: impl FnOnce(&State) -> R) -> R {
    init();
    f(STATE.read().unwrap().as_ref().unwrap())
}

fn update(f: impl FnOnce(&mut State)) {
    init();
    let mut g = STATE.write().unwrap();
    let st = g.as_mut().unwrap();
    f(st);
    st.colors = st.preview.as_ref().unwrap_or(&st.chosen).c;
    VERSION.fetch_add(1, Ordering::Relaxed);
}

/// The colours to draw with now.
pub fn c() -> Colors {
    with(|s| s.colors)
}

/// The skin shown now (the chosen one, or the one being previewed).
pub fn current() -> Arc<Skin> {
    with(|s| s.preview.clone().unwrap_or_else(|| s.chosen.clone()))
}

/// The skin chosen and saved.
pub fn chosen() -> Arc<Skin> {
    with(|s| s.chosen.clone())
}

pub fn all() -> Vec<Arc<Skin>> {
    with(|s| s.skins.clone())
}

/// Show a skin without saving it (the skin menu, on hover); `None` goes back.
pub fn preview(id: Option<&str>) {
    let cur = with(|s| s.preview.as_ref().map(|p| p.id.clone()));
    if cur.as_deref() == id {
        return;
    }
    update(|s| s.preview = id.and_then(|id| s.skins.iter().find(|k| k.id == id).cloned()));
}

/// Choose a skin and save it (NikPak and unNik share it).
pub fn apply(id: &str) {
    update(|s| {
        if let Some(k) = s.skins.iter().find(|k| k.id == id) {
            s.chosen = k.clone();
        }
        s.preview = None;
    });
    crate::gui::write_setting("skin", &chosen().id);
}

/// The sun / moon button: Nikkoid Blue <-> Nikkoid Bright.
pub fn toggle_bright() {
    apply(if chosen().dark { BRIGHT } else { DEFAULT });
}

/// Copy a Nikkomander skin file into the skins folder and choose it.
pub fn import(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let skin = parse_skin_json(&text)?;
    if is_builtin(&skin.id) {
        return Err(format!("\"{}\" is the name of a built-in skin", skin.id));
    }
    let file = custom_file(&skin.id).ok_or("no settings folder")?;
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&file, text.as_bytes()).map_err(|e| e.to_string())?;
    let (id, name) = (skin.id.clone(), skin.name.clone());
    update(|s| {
        s.skins.retain(|k| k.id != id);
        s.skins.push(Arc::new(skin));
    });
    apply(&id);
    Ok(name)
}

/// Remove an imported skin (built-in skins stay).
pub fn remove(id: &str) {
    if is_builtin(id) {
        return;
    }
    if let Some(dir) = custom_dir() {
        // the file may have any name: remove the one(s) holding this id
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let hit = std::fs::read_to_string(e.path()).ok().and_then(|t| parse_skin_json(&t).ok()).is_some_and(|s| s.id == id);
                if hit {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }
    }
    let was = chosen().id == id;
    update(|s| {
        s.skins.retain(|k| k.id != id);
        if s.preview.as_ref().is_some_and(|p| p.id == id) {
            s.preview = None;
        }
    });
    if was {
        apply(DEFAULT);
    }
}

/// Follow a skin chosen in the other program (called every few seconds).
pub fn follow() {
    if std::env::var_os("NIKPAK_GUI_SKIN").is_some() || with(|s| s.preview.is_some()) {
        return;
    }
    let Some(id) = crate::gui::read_setting("skin") else { return };
    if id != chosen().id {
        let known = with(|s| s.skins.iter().any(|k| k.id == id));
        if !known {
            // imported in the other program
            let extra = load_custom();
            update(|s| {
                for k in extra {
                    if !s.skins.iter().any(|x| x.id == k.id) {
                        s.skins.push(Arc::new(k));
                    }
                }
            });
        }
        update(|s| {
            if let Some(k) = s.skins.iter().find(|k| k.id == id) {
                s.chosen = k.clone();
            }
        });
    }
}

/// Keep egui's own colours (text fields, tooltips) in step with the skin.
pub fn sync_style(ctx: &egui::Context) {
    thread_local!(static SEEN: std::cell::Cell<u64> = const { std::cell::Cell::new(0) });
    let v = VERSION.load(Ordering::Relaxed);
    if SEEN.with(|s| s.replace(v)) == v {
        return;
    }
    let k = c();
    let mut style = (*ctx.style()).clone();
    style.visuals = if k.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let a = |c: Color32, x: u8| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), x);
    style.visuals.selection.bg_fill = a(k.accent, 140);
    style.visuals.selection.stroke = egui::Stroke::new(1.0_f32, k.text);
    style.visuals.text_cursor.stroke = egui::Stroke::new(1.5_f32, k.accent_soft);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, k.text);
    style.visuals.override_text_color = Some(k.text);
    style.visuals.window_fill = k.dialog_top;
    style.visuals.panel_fill = k.panel_top;
    style.visuals.extreme_bg_color = k.field;
    style.visuals.window_stroke = egui::Stroke::new(1.0_f32, a(k.accent_soft, 90));
    style.visuals.window_corner_radius = CornerRadius::same((8.0 * k.radius).round() as u8);
    ctx.set_style(style);
    ctx.request_repaint();
}

/// A colour that reads well on `bg` (black or white).
pub fn on(bg: Color32) -> Color32 {
    let l = 0.2126 * bg.r() as f32 + 0.7152 * bg.g() as f32 + 0.0722 * bg.b() as f32;
    if l > 150.0 {
        Color32::from_rgb(0x08, 0x10, 0x20)
    } else {
        Color32::WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_skin_parses() {
        assert_eq!(BUILTIN.len(), 75);
        assert_eq!(EXTRA.len(), 14);
        assert_eq!(BUILTIN[0].0, DEFAULT);
        assert!(BUILTIN.iter().any(|b| b.0 == BRIGHT && !b.2));
        for (id, _, _, vars) in BUILTIN.iter().chain(EXTRA.iter()) {
            for (k, v) in vars.iter() {
                if *k == "app-bg" {
                    let n = split_top(v, ',').len();
                    assert_eq!(parse_background(v).len(), n, "{id}: {k} = {v}");
                } else if *k == "panel-bg" || *k == "dialog-bg" {
                    assert!(ends(v).is_some(), "{id}: {k}");
                } else if *k != "radius" {
                    assert!(parse_color(v).is_some(), "{id}: {k} = {v}");
                }
            }
        }
    }

    #[test]
    fn nikkoid_is_nikpaks_look() {
        let s = builtin_skins();
        let k = s[0].c;
        assert_eq!(k.bg_top, Color32::from_rgb(0x10, 0x26, 0x58));
        assert_eq!(k.bg_bottom, Color32::from_rgb(0x04, 0x09, 0x1C));
        assert_eq!(k.panel_top, Color32::from_rgb(0x0E, 0x22, 0x4E));
        assert_eq!(k.accent, Color32::from_rgb(0x1E, 0x9B, 0xFF));
        assert_eq!(k.accent_hover, Color32::from_rgb(0x3A, 0xAC, 0xFF));
        assert!((k.radius - 1.0).abs() < 1e-6);
        assert!((k.glow - 1.0).abs() < 1e-6);
        assert!(!s[1].dark);
        // flat skins have no glow and square corners
        assert_eq!(s.iter().filter(|k| k.veil).map(|k| k.id.as_str()).collect::<Vec<_>>(), ["pizza"]);
        let tc = s.iter().find(|k| k.id == "tcblack").unwrap();
        assert_eq!(tc.c.glow, 0.0);
        assert_eq!(tc.c.radius, 0.0);
    }

    #[test]
    fn paints_patterns() {
        // 8-bit: a 16 px checkerboard (conic gradient) under a brick band at the bottom
        let s = builtin_skins();
        let k = s.iter().find(|k| k.id == "8bit").unwrap();
        let img = paint(k, 64.0, 80.0, 1.0);
        let px = |x: usize, y: usize| img.pixels[y * 64 + x];
        assert_ne!(px(4, 4), px(12, 4));
        assert_eq!(px(4, 4), px(20, 4));
        assert_eq!(px(5, 78), Color32::from_rgb(0xC8, 0x4C, 0x0C));
        // Nikkoid: the vertical gradient, with the glow at the top
        let n = &s[0];
        let img = paint(n, 100.0, 400.0, 1.0);
        let bottom = img.pixels[399 * 100 + 50];
        assert!((bottom.r() as i32 - 4).abs() <= 2 && (bottom.b() as i32 - 0x1C).abs() <= 3, "{bottom:?}");
    }

    #[test]
    fn painted_scenes() {
        let all = builtin_skins();
        for id in [
            "lightning", "rockies", "moon", "america", "pattaya", "burgers", "sushi", "amiga", "msx", "colecovision", "zx-spectrum",
            "catlife", "doglife", "mario", "cs", "chess",
        ] {
            let k = all.iter().find(|k| k.id == id).unwrap_or_else(|| panic!("{id} is missing"));
            assert!(k.scene, "{id} is not marked as a scene");
            let img = paint(k, 120.0, 90.0, 1.0);
            assert_eq!(img.size, [120, 90]);
            let first = img.pixels[0];
            assert!(img.pixels.iter().any(|p| *p != first), "{id} painted nothing");
        }
    }

    /// NIKPAK_SKIN_DUMP=<dir> cargo test --release --lib dump_backgrounds -- --ignored
    #[test]
    #[ignore]
    fn dump_backgrounds() {
        let dir = std::path::PathBuf::from(std::env::var("NIKPAK_SKIN_DUMP").unwrap());
        for s in builtin_skins() {
            let img = paint(&s, 500.0, 560.0, 1.0);
            let mut out = format!("P6\n{} {}\n255\n", img.size[0], img.size[1]).into_bytes();
            for p in &img.pixels {
                out.extend_from_slice(&[p.r(), p.g(), p.b()]);
            }
            std::fs::write(dir.join(format!("{}.ppm", s.id)), out).unwrap();
        }
    }

    #[test]
    fn colours() {
        assert_eq!(parse_color("#1E9BFF"), Some([30.0 / 255.0, 155.0 / 255.0, 1.0, 1.0]));
        assert_eq!(parse_color("rgba(0,0,0,.5)").unwrap()[3], 0.5);
        assert_eq!(parse_color("rgb(255 0 0 / 50%)").unwrap()[3], 0.5);
        assert_eq!(parse_color("#000").unwrap(), [0.0, 0.0, 0.0, 1.0]);
        assert!(parse_color("url(x.png)").is_none());
    }

    #[test]
    fn nikkomander_skin_files() {
        let ocean = r##"{ "id": "ocean-glass", "name": "Ocean Glass", "dark": true,
          "vars": { "bg": "#06121f", "panel": "#0b2236", "fg": "#d7e6f2", "accent": "#4cc2ff", "radius": "14px" } }"##;
        let s = parse_skin_json(ocean).unwrap();
        assert_eq!(s.id, "ocean-glass");
        assert!(s.custom && s.dark);
        assert_eq!(s.c.accent, Color32::from_rgb(0x4c, 0xc2, 0xff));
        // the default's gradients are dropped when the skin sets its own colours
        assert_eq!(s.c.panel_top, Color32::from_rgb(0x0b, 0x22, 0x36));
        assert!(s.layers.is_empty());
        assert!(parse_skin_json("{\"name\": \"x\"}").is_err());
        assert!(parse_skin_json("not json").is_err());
    }
}
