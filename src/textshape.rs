//! Text in scripts that egui cannot lay out by itself: Devanagari (Hindi),
//! Bengali and Arabic need *shaping* (letters change form and combine, vowel
//! signs move) and Arabic is written right to left. egui places one glyph per
//! character, which would print those languages wrongly.
//!
//! Here a text is ordered with the Unicode bidirectional algorithm
//! (unicode-bidi), shaped with rustybuzz (a Rust port of HarfBuzz), rasterised
//! with ab_glyph into an alpha mask, and handed to egui as a texture that the
//! caller tints with the wanted colour. Results are cached per text and size.
//! Latin letters and digits inside such a text use the same Inter fonts as the
//! rest of the interface.

use ab_glyph::{Font as _, FontRef, GlyphId, OutlinedGlyph, PxScale, PxScaleFactor};
use eframe::egui::{self, Color32, ColorImage, TextureHandle, TextureOptions, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// Does this text need the shaped path?
pub fn needs_shaping(s: &str) -> bool {
    s.chars().any(|c| script_of(c).is_some())
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Weight {
    Light,
    Medium,
    DisplayLight,
    DisplayMedium,
}

impl Weight {
    fn latin(self) -> usize {
        match self {
            Weight::Light => 0,
            Weight::Medium => 1,
            Weight::DisplayLight => 2,
            Weight::DisplayMedium => 3,
        }
    }
    fn medium(self) -> bool {
        matches!(self, Weight::Medium | Weight::DisplayMedium)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Script {
    Devanagari,
    Bengali,
    Arabic,
}

fn script_of(c: char) -> Option<Script> {
    match c as u32 {
        0x0900..=0x097F | 0xA8E0..=0xA8FF | 0x1CD0..=0x1CFF => Some(Script::Devanagari),
        0x0980..=0x09FF => Some(Script::Bengali),
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => Some(Script::Arabic),
        _ => None,
    }
}

struct Face {
    ab: FontRef<'static>,
    hb: rustybuzz::Face<'static>,
    upem: f32,
}

impl Face {
    fn new(data: &'static [u8]) -> Face {
        let ab = FontRef::try_from_slice(data).expect("embedded font");
        let hb = rustybuzz::Face::from_slice(data, 0).expect("embedded font");
        let upem = ab.units_per_em().unwrap_or(1000.0);
        Face { ab, hb, upem }
    }
    fn has(&self, c: char) -> bool {
        self.ab.glyph_id(c).0 != 0
    }
}

struct Faces {
    /// Inter: light, medium, display light, display medium (same files as the egui fonts)
    latin: [Face; 4],
    /// [regular, medium] per script
    deva: [Face; 2],
    beng: [Face; 2],
    arab: [Face; 2],
}

fn faces() -> &'static Faces {
    static F: OnceLock<Faces> = OnceLock::new();
    F.get_or_init(|| Faces {
        latin: [
            Face::new(include_bytes!("../assets/fonts/Inter-Light.otf")),
            Face::new(include_bytes!("../assets/fonts/Inter-Medium.otf")),
            Face::new(include_bytes!("../assets/fonts/InterDisplay-ExtraLight.otf")),
            Face::new(include_bytes!("../assets/fonts/InterDisplay-Medium.otf")),
        ],
        deva: [
            Face::new(include_bytes!("../assets/fonts/NotoSansDevanagari-Regular-subset.ttf")),
            Face::new(include_bytes!("../assets/fonts/NotoSansDevanagari-Medium-subset.ttf")),
        ],
        beng: [
            Face::new(include_bytes!("../assets/fonts/NotoSansBengali-Regular-subset.ttf")),
            Face::new(include_bytes!("../assets/fonts/NotoSansBengali-Medium-subset.ttf")),
        ],
        arab: [
            Face::new(include_bytes!("../assets/fonts/NotoSansArabic-Regular-subset.ttf")),
            Face::new(include_bytes!("../assets/fonts/NotoSansArabic-Medium-subset.ttf")),
        ],
    })
}

/// Index of a face: 0-3 Inter, 4/5 Devanagari, 6/7 Bengali, 8/9 Arabic.
fn face(i: usize) -> &'static Face {
    let f = faces();
    match i {
        0..=3 => &f.latin[i],
        4 | 5 => &f.deva[i - 4],
        6 | 7 => &f.beng[i - 6],
        _ => &f.arab[(i - 8).min(1)],
    }
}

fn script_face(s: Script, w: Weight) -> usize {
    let m = w.medium() as usize;
    match s {
        Script::Devanagari => 4 + m,
        Script::Bengali => 6 + m,
        Script::Arabic => 8 + m,
    }
}

/// Characters that belong to no script of their own (spaces, punctuation,
/// joiners, marks): they follow their neighbours.
fn is_common(c: char) -> bool {
    !c.is_alphanumeric() || matches!(c as u32, 0x200C | 0x200D)
}

struct Placed {
    face: usize,
    glyph: u16,
    x: f32,
    y: f32,
}

struct Layout {
    glyphs: Vec<Placed>,
    width: f32,
    ascent: f32,
    descent: f32,
}

/// Bidi ordering, font choice and shaping, in pixels for `px` pixels per em.
fn layout(text: &str, w: Weight, px: f32) -> Layout {
    use unicode_bidi::{BidiInfo, Level};
    let rtl_base = text.chars().any(|c| script_of(c) == Some(Script::Arabic));
    let info = BidiInfo::new(text, Some(if rtl_base { Level::rtl() } else { Level::ltr() }));
    let mut out = Layout { glyphs: Vec::new(), width: 0.0, ascent: 0.0, descent: 0.0 };
    let latin = w.latin();
    let mut used = vec![latin];
    for para in &info.paragraphs {
        let line = para.range.clone();
        let (levels, runs) = info.visual_runs(para, line);
        for run in runs {
            let rtl = levels[run.start].is_rtl();
            let s = &text[run.clone()];
            // font of each character
            let chars: Vec<(usize, char)> = s.char_indices().collect();
            let mut fidx: Vec<Option<usize>> = chars
                .iter()
                .map(|(_, c)| {
                    if let Some(sc) = script_of(*c) {
                        if is_common(*c) { None } else { Some(script_face(sc, w)) }
                    } else if is_common(*c) {
                        None
                    } else {
                        Some(latin)
                    }
                })
                .collect();
            // neutral characters join the previous (else next) font that has them
            for i in 0..chars.len() {
                if fidx[i].is_none() {
                    let c = chars[i].1;
                    let prev = (0..i).rev().find_map(|j| fidx[j]);
                    let next = (i + 1..chars.len()).find_map(|j| fidx[j]);
                    let pick = [prev, next].into_iter().flatten().find(|f| face(*f).has(c) || script_of(c).is_some());
                    fidx[i] = Some(pick.unwrap_or(latin));
                }
            }
            // segments of one font, in logical order
            let mut segs: Vec<(usize, usize, usize)> = Vec::new(); // (face, start, end) byte offsets in s
            for (k, (bi, c)) in chars.iter().enumerate() {
                let f = fidx[k].unwrap_or(latin);
                let end = bi + c.len_utf8();
                match segs.last_mut() {
                    Some(last) if last.0 == f => last.2 = end,
                    _ => segs.push((f, *bi, end)),
                }
            }
            if rtl {
                segs.reverse();
            }
            for (f, a, b) in segs {
                let fc = face(f);
                if !used.contains(&f) {
                    used.push(f);
                }
                let scale = px / fc.upem;
                let mut buf = rustybuzz::UnicodeBuffer::new();
                buf.push_str(&s[a..b]);
                buf.set_direction(if rtl { rustybuzz::Direction::RightToLeft } else { rustybuzz::Direction::LeftToRight });
                buf.guess_segment_properties();
                let shaped = rustybuzz::shape(&fc.hb, &[], buf);
                for (gi, gp) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                    out.glyphs.push(Placed {
                        face: f,
                        glyph: gi.glyph_id as u16,
                        x: out.width + gp.x_offset as f32 * scale,
                        y: -(gp.y_offset as f32) * scale,
                    });
                    out.width += gp.x_advance as f32 * scale;
                }
            }
        }
    }
    for f in used {
        let fc = face(f);
        let scale = px / fc.upem;
        out.ascent = out.ascent.max(fc.ab.ascent_unscaled() * scale);
        out.descent = out.descent.max(-fc.ab.descent_unscaled() * scale);
    }
    out
}

/// A shaped text ready to paint: an alpha mask (tint it with the text colour).
pub struct Shaped {
    pub texture: TextureHandle,
    /// size in points
    pub size: Vec2,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct Key(String, Weight, u32, u32);

fn cache() -> &'static Mutex<HashMap<Key, Arc<Shaped>>> {
    static C: OnceLock<Mutex<HashMap<Key, Arc<Shaped>>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Width in points of `text` at `size` points (no rasterising).
pub fn width(text: &str, w: Weight, size: f32) -> f32 {
    layout(text, w, size).width
}

/// Shape and rasterise `text` (one line) at `size` points.
pub fn shaped(ctx: &egui::Context, text: &str, w: Weight, size: f32) -> Arc<Shaped> {
    let ppp = ctx.pixels_per_point();
    let key = Key(text.to_string(), w, size.to_bits(), ppp.to_bits());
    if let Some(s) = cache().lock().unwrap().get(&key) {
        return s.clone();
    }
    let px = size * ppp;
    let l = layout(text, w, px);
    let pad = 2usize;
    let wpx = l.width.ceil() as usize + 2 * pad;
    let hpx = (l.ascent + l.descent).ceil() as usize + 2 * pad;
    let mut cov = vec![0f32; wpx * hpx];
    let base = pad as f32 + l.ascent;
    for g in &l.glyphs {
        let fc = face(g.face);
        let Some(outline) = fc.ab.outline(GlyphId(g.glyph)) else { continue };
        let s = px / fc.upem;
        let glyph = ab_glyph::Glyph {
            id: GlyphId(g.glyph),
            scale: PxScale::from(0.0),
            position: ab_glyph::point(pad as f32 + g.x, base + g.y),
        };
        let og = OutlinedGlyph::new(glyph, outline, PxScaleFactor { horizontal: s, vertical: s });
        let bb = og.px_bounds();
        og.draw(|x, y, c| {
            let px_ = bb.min.x as i32 + x as i32;
            let py_ = bb.min.y as i32 + y as i32;
            if px_ >= 0 && py_ >= 0 && (px_ as usize) < wpx && (py_ as usize) < hpx {
                let i = py_ as usize * wpx + px_ as usize;
                cov[i] = (cov[i] + c).min(1.0);
            }
        });
    }
    let pixels: Vec<Color32> = cov.iter().map(|c| Color32::from_white_alpha((c * 255.0).round() as u8)).collect();
    let image = ColorImage::new([wpx, hpx], pixels);
    let texture = ctx.load_texture(format!("shaped:{text}"), image, TextureOptions::LINEAR);
    let out = Arc::new(Shaped { texture, size: Vec2::new(wpx as f32 / ppp, hpx as f32 / ppp) });
    cache().lock().unwrap().insert(key, out.clone());
    out
}

/// Greedy line breaking at spaces for a shaped text.
pub fn wrap(text: &str, w: Weight, size: f32, max_width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split(' ').filter(|s| !s.is_empty()) {
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && width(&cand, w, size) > max_width {
            lines.push(std::mem::replace(&mut cur, word.to_string()));
        } else {
            cur = cand;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shaping_changes_devanagari_and_arabic() {
        // "कि": the vowel sign i is drawn before the consonant (glyph order differs from text order)
        let l = layout("कि", Weight::Light, 20.0);
        assert_eq!(l.glyphs.len(), 2);
        assert!(l.glyphs[0].x < l.glyphs[1].x);
        // Arabic letters join: shaped glyphs differ from the isolated forms
        let iso = layout("ل", Weight::Light, 20.0).glyphs[0].glyph;
        let joined = layout("لا", Weight::Light, 20.0);
        assert!(joined.glyphs.iter().all(|g| g.glyph != iso));
        // mixed text keeps Latin in Inter
        let m = layout("تسجيل NikPak", Weight::Medium, 20.0);
        assert!(m.glyphs.iter().any(|g| g.face == 1));
        assert!(m.glyphs.iter().any(|g| g.face == 9));
        assert!(needs_shaping("नाम") && needs_shaping("নাম") && needs_shaping("الاسم") && !needs_shaping("Name 中文 Имя"));
    }

    #[test]
    fn latin_width_matches_em_scale() {
        // Inter at 20 px: "NikPak" is about 3 em wide
        let w = width("NikPak", Weight::Medium, 20.0);
        assert!(w > 50.0 && w < 80.0, "{w}");
    }
}
