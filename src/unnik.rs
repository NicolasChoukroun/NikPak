//! unNik - the free browser and extractor for .nik archives.
//!
//! The same frameless window and look as NikPak. Empty, it shows the drop
//! ring with an arrow going up: drop a .nik archive (or click the ring) and
//! the window grows into an archive browser:
//!
//! * a folder tree, breadcrumbs, back / forward / up, and a search over the
//!   whole archive;
//! * a sortable list (name, size, type, date) with check boxes, Ctrl and
//!   Shift selection, a context menu and keyboard shortcuts;
//! * a details pane and a compression summary;
//! * extract all, extract the selection (with or without folders, overwrite /
//!   keep both / skip), delete, test;
//! * a built-in viewer for text (with find, wrap, copy) and a hex view for
//!   binary files; "Open in Notepad" and "Open" with the default program.
//!
//! unNik is free for everyone, with no registration. All archive work is done
//! by archive.rs, on a background thread, with progress and Cancel.

use crate::archive::{self, Entry, Index, Pick, PROGRESS};
use crate::error::NikError;
use crate::gui::{
    alpha, card, card_close, family, font_definitions, glow_text, gradient_panel, human, icon_button, lang_button,
    lang_menu, lay, lay_fit, logo, open_link, pill_button, progress_arc, radial_glow, read_setting,
    social_buttons, spaced, spaced_fit, text, text_fit, tip, wrapped, write_setting, arc_points, label_pos, Txt,
    ACCENT, ACCENT_FG, ACCENT_SOFT, COPYRIGHT, WARN, hi, rad, rr, DISTRIBUTOR, ERR_RED, OK_GREEN, RADIUS, TEXT, TEXT_DIM, TRACK,
};
use crate::i18n::{self, t, tf};
use crate::sfx::{self, Sfx};
use crate::textshape;
use eframe::egui::{
    self, epaint::PathShape, Align2, Color32, CornerRadius, FontData, FontId, Pos2, Rect, Sense, Shape,
    Stroke, Vec2,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Sizes
// ---------------------------------------------------------------------------

/// The empty window: exactly NikPak's (500 x 560).
const SMALL: Vec2 = Vec2::new(crate::gui::W, crate::gui::H);
/// The browser.
const BIG: Vec2 = Vec2::new(1120.0, 720.0);
const BIG_MIN: Vec2 = Vec2::new(880.0, 560.0);

const TITLE_H: f32 = 58.0;
const ROW_H: f32 = 32.0;
const TREE_ROW: f32 = 27.0;
const FOOT_H: f32 = 50.0;

/// Viewer: at most this much of a file is loaded.
const VIEW_LIMIT: usize = 16 << 20;

/// The browser's panels (tree, file list), over the skin's background.
#[allow(non_snake_case)]
fn PANEL_BG() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid {
        Color32::from_rgba_premultiplied(2, 7, 22, 150)
    } else {
        // Nikkomander's panels (panel-bg), nearly opaque over patterned backgrounds
        let c = k.panel_top;
        if c.a() == 255 { alpha(c, 225) } else { c }
    }
}
#[allow(non_snake_case)]
fn FOLDER() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid { Color32::from_rgb(0xF6, 0xBE, 0x4F) } else { k.icon_dir }
}
#[allow(non_snake_case)]
fn FOLDER_DARK() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid {
        Color32::from_rgb(0xD9, 0x95, 0x26)
    } else {
        let d = |v: u8| (v as f32 * 0.8) as u8;
        Color32::from_rgb(d(k.icon_dir.r()), d(k.icon_dir.g()), d(k.icon_dir.b()))
    }
}

/// The paper of the file icons.
#[allow(non_snake_case)]
fn PAGE() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid { Color32::from_rgb(0x1A, 0x2C, 0x52) } else { k.panel_top }
}
/// Top of the right-click menu.
#[allow(non_snake_case)]
fn MENU_TOP() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid { Color32::from_rgb(0x12, 0x28, 0x58) } else { k.dialog_top }
}
/// "Delete" in menus and dialogs.
#[allow(non_snake_case)]
fn DANGER_SOFT() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid {
        Color32::from_rgb(0xFF, 0x9A, 0xA4)
    } else if k.dark {
        k.err.lerp_to_gamma(Color32::WHITE, 0.3)
    } else {
        k.err
    }
}
#[allow(non_snake_case)]
fn DELETE_BTN() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid { Color32::from_rgb(0xE5, 0x4B, 0x5E) } else { k.err }
}
#[allow(non_snake_case)]
fn DELETE_HOVER() -> Color32 {
    let k = crate::skin::c();
    if k.nikkoid { Color32::from_rgb(0xFF, 0x80, 0x8E) } else { k.err.lerp_to_gamma(Color32::WHITE, 0.2) }
}

// ---------------------------------------------------------------------------
// Fonts: NikPak's, a monospaced font for the viewer, and the system's fonts
// as a last fallback, so file names in any script can be shown.
// ---------------------------------------------------------------------------

fn system_fallback_fonts() -> Vec<(String, Vec<u8>)> {
    let mut list: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let f = win.join("Fonts");
        for n in ["segoeui.ttf", "msyh.ttc", "malgun.ttf", "YuGothR.ttc", "Nirmala.ttf", "seguisym.ttf", "arial.ttf"] {
            list.push(f.join(n));
        }
    } else if cfg!(target_os = "macos") {
        for n in ["/System/Library/Fonts/Helvetica.ttc", "/System/Library/Fonts/PingFang.ttc", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"] {
            list.push(PathBuf::from(n));
        }
    } else {
        for n in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        ] {
            list.push(PathBuf::from(n));
        }
    }
    list.into_iter()
        .filter_map(|p| {
            let bytes = std::fs::read(&p).ok()?;
            let name = format!("sys-{}", p.file_name()?.to_string_lossy());
            (bytes.len() > 1000).then_some((name, bytes))
        })
        .collect()
}

fn install_fonts(ctx: &egui::Context) {
    let mut f = font_definitions();
    f.font_data.insert("mono".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular-subset.ttf"))));
    f.families.insert(family("mono"), vec!["mono".into(), "ui-light".into(), "sc".into()]);
    for (name, bytes) in system_fallback_fonts() {
        f.font_data.insert(name.clone(), Arc::new(FontData::from_owned(bytes)));
        for fam in f.families.values_mut() {
            fam.push(name.clone());
        }
    }
    ctx.set_fonts(f);
}

fn mono(size: f32) -> FontId {
    FontId::new(size, family("mono"))
}

/// Window icon: the blue ring with an arrow going up.
fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 32.0);
            let r = (fx * fx + fy * fy).sqrt();
            let a = if r < 31.0 { 255u8 } else { 0 };
            let mut col = (0x0Eu8, 0x22u8, 0x50u8);
            if (r - 25.0).abs() < 3.2 {
                col = (0x1E, 0x9B, 0xFF);
            }
            let shaft = fx.abs() < 2.6 && fy > -6.0 && fy < 14.0;
            let head = fy <= 0.0 && fy > -12.0 && fx.abs() < 12.0 + fy;
            if shaft || head {
                col = (0xEA, 0xF4, 0xFF);
            }
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[col.0, col.1, col.2, a]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

/// Offset of local time from UTC, in seconds (dates are stored in UTC).
fn local_offset() -> i64 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct SystemTime([u16; 8]);
        #[repr(C)]
        struct Tzi {
            bias: i32,
            standard_name: [u16; 32],
            standard_date: SystemTime,
            standard_bias: i32,
            daylight_name: [u16; 32],
            daylight_date: SystemTime,
            daylight_bias: i32,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetTimeZoneInformation(tzi: *mut Tzi) -> u32;
        }
        // SAFETY: the structure has the layout of TIME_ZONE_INFORMATION.
        let mut z: Tzi = unsafe { std::mem::zeroed() };
        let r = unsafe { GetTimeZoneInformation(&mut z) };
        let extra = match r {
            1 => z.standard_bias,
            2 => z.daylight_bias,
            _ => 0,
        };
        return -((z.bias + extra) as i64) * 60;
    }
    #[cfg(not(windows))]
    {
        std::env::var("NIKPAK_GUI_TZ_MINUTES").ok().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0) * 60
    }
}

fn date(secs: i64, offset: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    crate::util::format_time(secs + offset)
}

fn files_str(n: u64) -> String {
    if n == 1 { t("1 file").to_string() } else { tf("{} files", &[&n.to_string()]) }
}

fn folders_str(n: u64) -> String {
    if n == 1 { t("1 folder").to_string() } else { tf("{} folders", &[&n.to_string()]) }
}

fn items_str(n: usize) -> String {
    if n == 1 { t("1 item").to_string() } else { tf("{} items", &[&n.to_string()]) }
}

fn ext_of(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name);
    match base.rfind('.') {
        Some(p) if p > 0 && p + 1 < base.len() => base[p + 1..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Code,
    Doc,
    Sheet,
    Image,
    Audio,
    Video,
    Archive,
    Program,
    Other,
}

fn kind_of(ext: &str) -> Kind {
    match ext {
        "txt" | "md" | "log" | "csv" | "tsv" | "ini" | "cfg" | "conf" | "nfo" | "srt" | "rtf" => Kind::Text,
        "rs" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "java" | "kt" | "py" | "js" | "ts" | "jsx" | "tsx" | "go"
        | "php" | "rb" | "pl" | "sh" | "ps1" | "bat" | "cmd" | "json" | "xml" | "yml" | "yaml" | "toml" | "html"
        | "htm" | "css" | "scss" | "sql" | "lua" | "swift" | "m" | "vb" | "asm" | "iss" | "lock" | "gradle" => Kind::Code,
        "pdf" | "doc" | "docx" | "odt" | "epub" | "ppt" | "pptx" | "odp" | "pages" => Kind::Doc,
        "xls" | "xlsx" | "ods" | "numbers" => Kind::Sheet,
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tif" | "tiff" | "svg" | "ico" | "psd" | "heic" | "raw" => Kind::Image,
        "wav" | "mp3" | "flac" | "ogg" | "aac" | "m4a" | "aif" | "aiff" | "mod" | "xm" | "s3m" | "it" | "mid" | "opus" | "wma" => Kind::Audio,
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "m4v" | "mpg" | "mpeg" | "flv" => Kind::Video,
        "nik" | "zip" | "rar" | "7z" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "tar" | "cab" | "iso" | "jar" | "apk" => Kind::Archive,
        "exe" | "dll" | "msi" | "sys" | "so" | "dylib" | "app" | "com" | "scr" | "bin" => Kind::Program,
        _ => Kind::Other,
    }
}

fn kind_color(k: Kind) -> Color32 {
    let rgb = Color32::from_rgb;
    let c = crate::skin::c();
    if !c.nikkoid {
        // the skin's file colours (Nikkomander's icon colours)
        return match k {
            Kind::Text | Kind::Other => c.icon_file,
            Kind::Code => c.icon_code,
            Kind::Doc | Kind::Program => c.accent_soft,
            Kind::Sheet => c.ok,
            Kind::Image => c.icon_img,
            Kind::Audio | Kind::Video => c.icon_media,
            Kind::Archive => c.icon_arc,
        };
    }
    match k {
        Kind::Text => rgb(0x9A, 0xB4, 0xD6),
        Kind::Code => rgb(0x3D, 0xDC, 0x97),
        Kind::Doc => rgb(0x4C, 0x8D, 0xFF),
        Kind::Sheet => rgb(0x2E, 0xC4, 0x6A),
        Kind::Image => rgb(0xFF, 0x6E, 0xB4),
        Kind::Audio => rgb(0xB0, 0x7C, 0xFF),
        Kind::Video => rgb(0xFF, 0x6B, 0x5A),
        Kind::Archive => rgb(0xFF, 0xA9, 0x3D),
        Kind::Program => rgb(0x34, 0xD6, 0xE0),
        Kind::Other => rgb(0x7E, 0x93, 0xB8),
    }
}

/// Does this look like text (UTF-8, UTF-16 with a BOM, or 8-bit text)?
fn looks_like_text(b: &[u8]) -> bool {
    if b.is_empty() {
        return true;
    }
    if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        return true;
    }
    let head = &b[..b.len().min(64 * 1024)];
    let mut ctrl = 0usize;
    for &c in head {
        match c {
            0 => return false,
            b'\t' | b'\n' | b'\r' | 0x0c | 0x1b => {}
            1..=0x1f | 0x7f => ctrl += 1,
            _ => {}
        }
    }
    ctrl * 100 <= head.len()
}

/// Bytes -> text: UTF-8 (BOM allowed), UTF-16 with a BOM, else Windows-1252.
fn decode_text(b: &[u8], truncated: bool) -> String {
    let s = if let Some(rest) = b.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(rest).into_owned()
    } else if b.starts_with(&[0xFF, 0xFE]) || b.starts_with(&[0xFE, 0xFF]) {
        let le = b[0] == 0xFF;
        let units: Vec<u16> = b[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        match std::str::from_utf8(b) {
            Ok(s) => s.to_string(),
            // cut in the middle of a character at the end (truncated view)
            Err(e) if truncated && e.error_len().is_none() => String::from_utf8_lossy(&b[..e.valid_up_to()]).into_owned(),
            Err(_) => b.iter().map(|&c| cp1252(c)).collect(),
        }
    };
    s.replace('\t', "    ")
}

fn cp1252(c: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}', '\u{90}', '‘', '’', '“',
        '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
    ];
    match c {
        0x80..=0x9f => HIGH[(c - 0x80) as usize],
        _ => c as char,
    }
}

/// Text that fits `max_w`, cut with "…" when needed.
fn elide(p: &egui::Painter, s: &str, font: FontId, max_w: f32) -> Txt {
    let full = lay(p, s, font.clone());
    if full.size().x <= max_w || max_w < 16.0 || textshape::needs_shaping(s) {
        return full;
    }
    let chars: Vec<char> = s.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        let cand: String = chars[..mid].iter().collect::<String>() + "…";
        if lay(p, &cand, font.clone()).size().x <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lay(p, &(chars[..lo].iter().collect::<String>() + "…"), font)
}

/// Text drawn inside `clip` only.
fn clipped(p: &egui::Painter, clip: Rect) -> egui::Painter {
    p.with_clip_rect(clip.intersect(p.clip_rect()))
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("unNik")
}

/// Remove what earlier sessions left in the temporary folder (older than a day).
fn clean_temp() {
    if let Ok(rd) = std::fs::read_dir(temp_dir()) {
        for e in rd.flatten() {
            let old = e.metadata().and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default() > Duration::from_secs(86_400)).unwrap_or(false);
            if old {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
}

fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(not(windows))]
    open_link(&p.display().to_string());
}

fn open_in_notepad(p: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("notepad.exe").arg(p).spawn();
    }
    #[cfg(not(windows))]
    open_link(&p.display().to_string());
}

/// Recently opened archives (newest first), kept in NikPak's settings file.
fn recent() -> Vec<PathBuf> {
    read_setting("unnik_recent")
        .unwrap_or_default()
        .split('|')
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .take(3)
        .collect()
}

fn add_recent(p: &Path) {
    let mut list = vec![p.to_path_buf()];
    list.extend(recent().into_iter().filter(|q| q != p));
    list.truncate(3);
    write_setting("unnik_recent", &list.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("|"));
}

// ---------------------------------------------------------------------------
// The tree of the archive
// ---------------------------------------------------------------------------

struct Node {
    name: String,
    /// full stored path ("" for the root)
    path: String,
    is_dir: bool,
    entry: Option<usize>,
    parent: usize,
    children: Vec<usize>,
    /// files, folders and total size below a folder (or the file's size)
    size: u64,
    files: u32,
    dirs: u32,
    mtime: i64,
    depth: u16,
}

fn build_tree(idx: &Index) -> Vec<Node> {
    let mut nodes = vec![Node { name: String::new(), path: String::new(), is_dir: true, entry: None, parent: 0, children: Vec::new(), size: 0, files: 0, dirs: 0, mtime: 0, depth: 0 }];
    let mut dirs: HashMap<String, usize> = HashMap::new();
    dirs.insert(String::new(), 0);
    fn ensure_dir(nodes: &mut Vec<Node>, dirs: &mut HashMap<String, usize>, path: &str) -> usize {
        if let Some(&i) = dirs.get(path) {
            return i;
        }
        let (parent_path, name) = match path.rfind('/') {
            Some(p) => (&path[..p], &path[p + 1..]),
            None => ("", path),
        };
        let parent = ensure_dir(nodes, dirs, parent_path);
        let i = nodes.len();
        let depth = nodes[parent].depth + 1;
        nodes.push(Node { name: name.to_string(), path: path.to_string(), is_dir: true, entry: None, parent, children: Vec::new(), size: 0, files: 0, dirs: 0, mtime: 0, depth });
        nodes[parent].children.push(i);
        dirs.insert(path.to_string(), i);
        i
    }
    for (k, e) in idx.entries.iter().enumerate() {
        let path = e.name.trim_matches('/');
        if path.is_empty() {
            continue;
        }
        if e.is_dir {
            let i = ensure_dir(&mut nodes, &mut dirs, path);
            nodes[i].entry = Some(k);
            nodes[i].mtime = e.mtime;
        } else {
            let (parent_path, name) = match path.rfind('/') {
                Some(p) => (&path[..p], &path[p + 1..]),
                None => ("", path),
            };
            let parent = ensure_dir(&mut nodes, &mut dirs, parent_path);
            let i = nodes.len();
            let depth = nodes[parent].depth + 1;
            nodes.push(Node { name: name.to_string(), path: path.to_string(), is_dir: false, entry: Some(k), parent, children: Vec::new(), size: e.size, files: 1, dirs: 0, mtime: e.mtime, depth });
            nodes[parent].children.push(i);
        }
    }
    // totals, bottom-up (children are always created after their parent)
    for i in (1..nodes.len()).rev() {
        let (size, files, dirs, is_dir, mtime) = (nodes[i].size, nodes[i].files, nodes[i].dirs, nodes[i].is_dir, nodes[i].mtime);
        let p = nodes[i].parent;
        nodes[p].size += size;
        nodes[p].files += files;
        nodes[p].dirs += dirs + is_dir as u32;
        if nodes[p].entry.is_none() {
            nodes[p].mtime = nodes[p].mtime.max(mtime);
        }
    }
    // folders first, then by name
    let keys: Vec<(bool, String)> = nodes.iter().map(|n| (!n.is_dir, n.name.to_lowercase())).collect();
    for n in nodes.iter_mut() {
        n.children.sort_by(|a, b| keys[*a].cmp(&keys[*b]));
    }
    nodes
}

// ---------------------------------------------------------------------------
// Background work
// ---------------------------------------------------------------------------

enum Output {
    Index(Index),
    /// after a delete: the new index and the number of entries deleted
    Reloaded(Index, u64),
    Extracted { dest: PathBuf, files: usize, warnings: Vec<String>, show: bool },
    /// a file extracted to the temporary folder, to open (notepad: in Notepad)
    Temp { file: PathBuf, notepad: bool },
    Text { name: String, size: u64, data: Vec<u8>, more: bool },
    Tested,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Work {
    Open,
    Extract,
    Delete,
    Test,
    Read,
}

struct Job {
    work: Work,
    label: String,
    started: Instant,
    handle: JoinHandle<Result<Output, NikError>>,
}

fn spawn(work: Work, label: String, f: impl FnOnce() -> Result<Output, NikError> + Send + 'static) -> Job {
    PROGRESS.cancel.store(false, Relaxed);
    PROGRESS.done.store(0, Relaxed);
    PROGRESS.total.store(0, Relaxed);
    if let Ok(mut w) = archive::WARNINGS.lock() {
        w.clear();
    }
    Job { work, label, started: Instant::now(), handle: std::thread::spawn(f) }
}

// ---------------------------------------------------------------------------
// Browser state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Col {
    Name,
    Size,
    Type,
    Date,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Exists {
    Overwrite,
    Rename,
    Skip,
}

struct Browser {
    path: PathBuf,
    password: Option<String>,
    index: Index,
    nodes: Vec<Node>,
    cwd: usize,
    back: Vec<usize>,
    fwd: Vec<usize>,
    selected: HashSet<usize>,
    anchor: Option<usize>,
    focus: Option<usize>,
    sort: (Col, bool),
    search: String,
    rows: Vec<usize>,
    dirty: bool,
    expanded: HashSet<usize>,
    list_scroll: f32,
    tree_scroll: f32,
    scroll_to_focus: bool,
    show_details: bool,
    tz: i64,
    /// the row of the last click: a double click only counts on the same row
    last_click: Option<usize>,
}

impl Browser {
    fn new(path: PathBuf, password: Option<String>, index: Index) -> Self {
        let nodes = build_tree(&index);
        let mut expanded = HashSet::new();
        expanded.insert(0);
        // a single top folder is opened in the tree
        if nodes[0].children.len() == 1 && nodes[nodes[0].children[0]].is_dir {
            expanded.insert(nodes[0].children[0]);
        }
        Browser {
            path,
            password,
            index,
            nodes,
            cwd: 0,
            back: Vec::new(),
            fwd: Vec::new(),
            selected: HashSet::new(),
            anchor: None,
            focus: None,
            sort: (Col::Name, true),
            search: String::new(),
            rows: Vec::new(),
            dirty: true,
            expanded,
            list_scroll: 0.0,
            tree_scroll: 0.0,
            scroll_to_focus: false,
            show_details: read_setting("unnik_details").as_deref() != Some("0"),
            tz: local_offset(),
            last_click: None,
        }
    }

    /// Keep the place (folder, selection by path) after the index was read again.
    fn reloaded(&mut self, index: Index) {
        let cwd_path = self.nodes[self.cwd].path.clone();
        let mut b = Browser::new(self.path.clone(), self.password.clone(), index);
        if let Some(i) = b.nodes.iter().position(|n| n.is_dir && n.path == cwd_path) {
            b.cwd = i;
        }
        b.sort = self.sort;
        b.show_details = self.show_details;
        let mut exp = HashSet::new();
        for &e in &self.expanded {
            if let Some(i) = b.nodes.iter().position(|n| n.is_dir && n.path == self.nodes[e].path) {
                exp.insert(i);
            }
        }
        b.expanded.extend(exp);
        *self = b;
    }

    fn name_of_archive(&self) -> String {
        self.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn entry(&self, n: usize) -> Option<&Entry> {
        self.nodes[n].entry.map(|k| &self.index.entries[k])
    }

    fn go(&mut self, n: usize) {
        if n != self.cwd && self.nodes[n].is_dir {
            self.back.push(self.cwd);
            self.fwd.clear();
            self.enter(n);
        }
    }

    fn enter(&mut self, n: usize) {
        let from = self.cwd;
        self.cwd = n;
        self.search.clear();
        self.selected.clear();
        self.anchor = None;
        // coming back up: the folder we came from is focused
        self.focus = if self.nodes[from].parent == n && from != n { Some(from) } else { None };
        if let Some(f) = self.focus {
            self.selected.insert(f);
            self.anchor = Some(f);
            self.scroll_to_focus = true;
        }
        self.list_scroll = 0.0;
        self.dirty = true;
        let mut p = n;
        while p != 0 {
            p = self.nodes[p].parent;
            self.expanded.insert(p);
        }
    }

    fn up(&mut self) {
        if self.cwd != 0 {
            let p = self.nodes[self.cwd].parent;
            self.back.push(self.cwd);
            self.fwd.clear();
            self.enter(p);
        }
    }

    fn go_back(&mut self) {
        if let Some(n) = self.back.pop() {
            self.fwd.push(self.cwd);
            self.enter(n);
        }
    }

    fn go_forward(&mut self) {
        if let Some(n) = self.fwd.pop() {
            self.back.push(self.cwd);
            self.enter(n);
        }
    }

    fn refresh_rows(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let q = self.search.trim().to_lowercase();
        let mut rows: Vec<usize> = if q.is_empty() {
            self.nodes[self.cwd].children.clone()
        } else {
            (1..self.nodes.len()).filter(|&i| self.nodes[i].name.to_lowercase().contains(&q)).collect()
        };
        let nodes = &self.nodes;
        let (col, asc) = self.sort;
        rows.sort_by(|&a, &b| {
            let (na, nb) = (&nodes[a], &nodes[b]);
            let dirs = nb.is_dir.cmp(&na.is_dir);
            if dirs != std::cmp::Ordering::Equal {
                return dirs;
            }
            let o = match col {
                Col::Name => na.name.to_lowercase().cmp(&nb.name.to_lowercase()),
                Col::Size => na.size.cmp(&nb.size),
                Col::Type => ext_of(&na.name).cmp(&ext_of(&nb.name)).then_with(|| na.name.to_lowercase().cmp(&nb.name.to_lowercase())),
                Col::Date => na.mtime.cmp(&nb.mtime),
            };
            if asc { o } else { o.reverse() }
        });
        self.rows = rows;
        self.selected.retain(|n| self.rows.contains(n));
    }

    /// Selected nodes in list order (or the focused one).
    fn selection(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.rows.iter().copied().filter(|n| self.selected.contains(n)).collect();
        if v.is_empty() {
            if let Some(f) = self.focus.filter(|f| self.rows.contains(f)) {
                v.push(f);
            }
        }
        v
    }

    /// Entry names of the nodes and of everything below them.
    fn entry_names(&self, nodes: &[usize]) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut stack: Vec<usize> = nodes.to_vec();
        while let Some(n) = stack.pop() {
            if let Some(e) = self.entry(n) {
                out.insert(e.name.clone());
            }
            stack.extend(self.nodes[n].children.iter().copied());
        }
        out
    }

    /// The entries to extract: everything (None) or the given nodes, with the
    /// current folder removed from their paths (not while searching).
    fn pick(&self, nodes: Option<&[usize]>) -> Pick {
        match nodes {
            None => Pick { names: self.index.entries.iter().map(|e| e.name.clone()).collect(), strip: String::new() },
            Some(ns) => {
                let strip = if self.search.trim().is_empty() && self.cwd != 0 { format!("{}/", self.nodes[self.cwd].path) } else { String::new() };
                Pick { names: self.entry_names(ns), strip }
            }
        }
    }

    fn sum(&self, nodes: &[usize]) -> (u64, u32) {
        let mut size = 0;
        let mut files = 0;
        for &n in nodes {
            size += self.nodes[n].size;
            files += self.nodes[n].files;
        }
        (size, files)
    }

    fn click(&mut self, n: usize, ctrl: bool, shift: bool) {
        if shift {
            let a = self.anchor.unwrap_or(n);
            let (ia, ib) = (self.rows.iter().position(|&x| x == a).unwrap_or(0), self.rows.iter().position(|&x| x == n).unwrap_or(0));
            let (lo, hi) = (ia.min(ib), ia.max(ib));
            if !ctrl {
                self.selected.clear();
            }
            self.selected.extend(self.rows[lo..=hi].iter().copied());
        } else if ctrl {
            if !self.selected.remove(&n) {
                self.selected.insert(n);
            }
            self.anchor = Some(n);
        } else {
            self.selected.clear();
            self.selected.insert(n);
            self.anchor = Some(n);
        }
        self.focus = Some(n);
    }

    fn move_focus(&mut self, delta: i64, shift: bool) {
        if self.rows.is_empty() {
            return;
        }
        let cur = self.focus.and_then(|f| self.rows.iter().position(|&x| x == f));
        let next = match cur {
            None => 0,
            Some(i) => (i as i64 + delta).clamp(0, self.rows.len() as i64 - 1) as usize,
        };
        let n = self.rows[next];
        self.click(n, false, shift);
        self.scroll_to_focus = true;
    }
}

// ---------------------------------------------------------------------------
// Viewer
// ---------------------------------------------------------------------------

struct Viewer {
    node: usize,
    name: String,
    size: u64,
    data: Vec<u8>,
    more: bool,
    text: Option<String>,
    hex: bool,
    wrap: bool,
    find: String,
    found: Vec<usize>,
    found_for: String,
    current: usize,
    scroll: f32,
    /// visual rows: (line number, start byte, end byte) in `text`
    rows: Vec<(u32, usize, usize)>,
    rows_cols: usize,
    jump: bool,
}

impl Viewer {
    fn new(node: usize, name: String, size: u64, data: Vec<u8>, more: bool) -> Self {
        let is_text = looks_like_text(&data);
        let text = is_text.then(|| decode_text(&data, more));
        Viewer {
            node,
            name,
            size,
            hex: text.is_none(),
            text,
            data,
            more,
            wrap: false,
            find: String::new(),
            found: Vec::new(),
            found_for: String::new(),
            current: 0,
            scroll: 0.0,
            rows: Vec::new(),
            rows_cols: 0,
            jump: false,
        }
    }

    fn layout(&mut self, cols: usize) {
        let cols = if self.wrap { cols.max(1) } else { usize::MAX };
        if self.rows_cols == cols && !self.rows.is_empty() {
            return;
        }
        self.rows_cols = cols;
        self.rows.clear();
        let Some(text) = &self.text else { return };
        let mut start = 0usize;
        for (ln, line) in text.split('\n').enumerate() {
            let end = start + line.len();
            let content_end = if line.ends_with('\r') { end - 1 } else { end };
            if cols == usize::MAX {
                self.rows.push((ln as u32, start, content_end));
            } else {
                let mut s = start;
                let mut count = 0usize;
                let mut pushed = false;
                for (i, _) in text[start..content_end].char_indices() {
                    if count == cols {
                        self.rows.push((ln as u32, s, start + i));
                        s = start + i;
                        count = 0;
                        pushed = true;
                    }
                    count += 1;
                }
                if s < content_end || !pushed {
                    self.rows.push((ln as u32, s, content_end));
                }
            }
            start = end + 1;
        }
    }

    fn search(&mut self) {
        if self.found_for == self.find {
            return;
        }
        self.found_for = self.find.clone();
        self.found.clear();
        self.current = 0;
        let q = self.find.to_ascii_lowercase();
        if q.is_empty() {
            return;
        }
        if let Some(text) = &self.text {
            let low = text.to_ascii_lowercase();
            let mut from = 0;
            while let Some(i) = low[from..].find(&q) {
                self.found.push(from + i);
                from += i + q.len().max(1);
                if self.found.len() >= 100_000 {
                    break;
                }
            }
        } else {
            let needle = q.as_bytes();
            let low: Vec<u8> = self.data.iter().map(|b| b.to_ascii_lowercase()).collect();
            let mut i = 0;
            while i + needle.len() <= low.len() && self.found.len() < 100_000 {
                if &low[i..i + needle.len()] == needle {
                    self.found.push(i);
                    i += needle.len();
                } else {
                    i += 1;
                }
            }
        }
        self.jump = !self.found.is_empty();
    }

    fn row_of(&self, byte: usize) -> usize {
        if self.hex {
            return byte / 16;
        }
        match self.rows.binary_search_by(|r| r.1.cmp(&byte)) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        }
    }
}

// ---------------------------------------------------------------------------
// The application
// ---------------------------------------------------------------------------

struct ExtractDlg {
    nodes: Option<Vec<usize>>,
    dest: String,
    keep_paths: bool,
    exists: Exists,
    show: bool,
}

enum Overlay {
    None,
    Password { path: PathBuf, password: String, error: Option<String> },
    Extract(ExtractDlg),
    Delete(Vec<usize>),
    Viewer(Box<Viewer>),
    Message { title: String, body: String, ok: bool },
    About,
    /// NikPak is not next to unNik: explain that it is shareware and where to
    /// find it, with a button to open the website.
    NikPakMissing,
}

struct Toast {
    text: String,
    folder: Option<PathBuf>,
    until: Instant,
}

struct App {
    browser: Option<Browser>,
    overlay: Overlay,
    job: Option<Job>,
    /// the archive being opened (before the browser exists)
    opening: Option<(PathBuf, Option<String>)>,
    toast: Option<Toast>,
    menu: Option<(Pos2, usize)>,
    lang_open: bool,
    /// skins menu
    skins: crate::gui::SkinMenu,
    start: Instant,
    shown: f32,
    big: bool,
    elevated: bool,
    search_focus: bool,
    viewer_find_focus: bool,
    error_idle: Option<String>,
    /// the node whose contents are being read for the viewer
    pending_view: Option<usize>,
    /// first frame of the password box (focus its field)
    pw_focus: bool,
    /// files were hovering over the window in the last frame (sound)
    was_hover: bool,
    /// started from the right-click menu: what to do with each archive
    quick: Option<Quick>,
    /// the archives still to extract (multiple selection)
    queue: VecDeque<PathBuf>,
}

/// What the right-click menu asked unNik to do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quick {
    /// "Extract files…": open the archive with its Extract box
    Dialog,
    /// "Extract here": next to the archive
    Here,
    /// "Extract to its own folder": into a folder named after the archive
    ToFolder,
}

fn start_args(b: &Option<Browser>) -> Vec<String> {
    b.as_ref().map(|b| vec![b.path.display().to_string()]).unwrap_or_default()
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, arg: Option<PathBuf>, quick: Option<(Quick, Vec<PathBuf>)>) -> Self {
        install_fonts(&cc.egui_ctx);
        crate::skin::sync_style(&cc.egui_ctx);
        clean_temp();
        let mut app = App {
            browser: None,
            overlay: Overlay::None,
            job: None,
            opening: None,
            toast: None,
            menu: None,
            lang_open: std::env::var_os("NIKPAK_GUI_LANGMENU").is_some(),
            skins: crate::gui::SkinMenu { open: std::env::var_os("NIKPAK_GUI_SKINMENU").is_some(), ..Default::default() },
            start: Instant::now(),
            shown: 0.0,
            big: false,
            elevated: crate::gui::is_elevated(),
            search_focus: false,
            viewer_find_focus: false,
            error_idle: None,
            pending_view: None,
            pw_focus: true,
            was_hover: false,
            quick: None,
            queue: VecDeque::new(),
        };
        if let Some((mode, archives)) = quick {
            app.quick = Some(mode);
            app.queue = archives.into();
            app.next_quick();
        }
        if let Some(p) = arg {
            app.open(p, std::env::var("NIKPAK_GUI_PASSWORD").ok());
        }
        if std::env::var("NIKPAK_GUI_MODAL").as_deref() == Ok("about") {
            app.overlay = Overlay::About;
        }
        app
    }

    // ---- actions ------------------------------------------------------------

    fn open(&mut self, path: PathBuf, password: Option<String>) {
        if self.job.is_some() {
            return;
        }
        match archive::peek(&path) {
            Err(e) => self.fail(t("Cannot open the archive"), e.to_string()),
            Ok(true) if password.is_none() => {
                self.overlay = Overlay::Password { path, password: String::new(), error: None };
                self.pw_focus = true;
                sfx::play(Sfx::Pop);
            }
            Ok(_) => {
                let label = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let (p2, pw2) = (path.clone(), password.clone());
                self.opening = Some((path, password));
                self.shown = 0.0;
                self.set_job(spawn(Work::Open, label, move || archive::read_index(&p2, pw2.as_deref()).map(Output::Index)));
            }
        }
    }

    /// Right-click "Extract here / to its own folder": the small window with the
    /// progress ring, never the browser.
    fn quick_small(&self) -> bool {
        matches!(self.quick, Some(Quick::Here | Quick::ToFolder))
    }

    /// The browser is on screen.
    fn browsing(&self) -> bool {
        self.browser.is_some() && !self.quick_small()
    }

    /// Open the next archive of the right-click selection; close the window when all are done.
    fn next_quick(&mut self) -> bool {
        self.browser = None;
        while let Some(p) = self.queue.pop_front() {
            if p.is_file() {
                // NIKPAK_GUI_PASSWORD: test hook, as for `unNik archive.nik`
                self.open(p, std::env::var("NIKPAK_GUI_PASSWORD").ok());
                return true;
            }
        }
        false
    }

    /// The archive is open: extract it as the right-click menu asked.
    fn quick_extract(&mut self) {
        let Some(b) = &self.browser else { return };
        let dir = b.path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let stem = b.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "archive".into());
        let dest = if self.quick == Some(Quick::ToFolder) { dir.join(stem) } else { dir };
        let exists = match read_setting("unnik_exists").as_deref() {
            Some("overwrite") => Exists::Overwrite,
            Some("skip") => Exists::Skip,
            _ => Exists::Rename,
        };
        self.extract(ExtractDlg { nodes: None, dest: dest.display().to_string(), keep_paths: true, exists, show: false });
    }

    fn set_job(&mut self, job: Job) {
        self.shown = 0.0;
        if matches!(job.work, Work::Extract | Work::Delete | Work::Test) {
            sfx::play(Sfx::Start);
        }
        self.job = Some(job);
    }

    fn fail(&mut self, title: &str, body: String) {
        self.overlay = Overlay::Message { title: title.to_string(), body, ok: false };
    }

    fn extract(&mut self, d: ExtractDlg) {
        let Some(b) = &self.browser else { return };
        let dest = PathBuf::from(d.dest.trim());
        if d.dest.trim().is_empty() {
            return;
        }
        if !self.quick_small() {
            // remember the choices of the Extract box (not those of the right-click menu)
            write_setting("unnik_keep_paths", if d.keep_paths { "1" } else { "0" });
            write_setting("unnik_exists", match d.exists { Exists::Overwrite => "overwrite", Exists::Rename => "rename", Exists::Skip => "skip" });
            write_setting("unnik_show", if d.show { "1" } else { "0" });
            write_setting("unnik_last_dest", d.dest.trim());
        }
        let pick = b.pick(d.nodes.as_deref());
        let files = pick.names.iter().filter(|n| b.index.entries.iter().any(|e| &e.name == *n && !e.is_dir)).count();
        let mut o = archive::gui_options(&b.path, b.password.as_deref());
        o.command = if d.keep_paths { crate::cli::Command::ExtractFull } else { crate::cli::Command::ExtractFlat };
        o.dest = Some(dest.clone());
        o.overwrite = match d.exists {
            Exists::Overwrite => crate::cli::Overwrite::All,
            Exists::Rename => crate::cli::Overwrite::Rename,
            Exists::Skip => crate::cli::Overwrite::Skip,
        };
        let show = d.show;
        self.set_job(spawn(Work::Extract, files_str(files as u64), move || {
            archive::extract_pick(&o, Some(&pick)).map(|_| Output::Extracted {
                dest,
                files,
                warnings: archive::WARNINGS.lock().map(|w| w.clone()).unwrap_or_default(),
                show,
            })
        }));
    }

    /// Extract one file to the temporary folder and open it.
    fn open_file(&mut self, n: usize, notepad: bool) {
        let Some(b) = &self.browser else { return };
        let Some(e) = b.entry(n) else { return };
        let dir = temp_dir().join(format!("{}-{}", std::process::id(), self.start.elapsed().as_millis()));
        let name = e.name.clone();
        let strip = match name.rfind('/') {
            Some(p) => name[..p + 1].to_string(),
            None => String::new(),
        };
        let pick = Pick { names: [name.clone()].into_iter().collect(), strip };
        let mut o = archive::gui_options(&b.path, b.password.as_deref());
        o.dest = Some(dir.clone());
        let file = dir.join(name.rsplit('/').next().unwrap_or(&name));
        self.set_job(spawn(Work::Read, b.nodes[n].name.clone(), move || {
            archive::extract_pick(&o, Some(&pick)).map(|_| Output::Temp { file, notepad })
        }));
    }

    fn view(&mut self, n: usize) {
        let Some(b) = &self.browser else { return };
        let Some(e) = b.entry(n) else { return };
        if e.is_dir {
            return;
        }
        let (path, pw, name, size) = (b.path.clone(), b.password.clone(), e.name.clone(), e.size);
        let label = b.nodes[n].name.clone();
        self.set_job(spawn(Work::Read, label, move || {
            archive::read_entry(&path, pw.as_deref(), &name, VIEW_LIMIT).map(|(data, more)| Output::Text { name, size, data, more })
        }));
        if let Overlay::Viewer(v) = &mut self.overlay {
            v.node = n;
        } else {
            self.overlay = Overlay::None;
        }
        self.pending_view = Some(n);
    }

    fn activate(&mut self, n: usize) {
        let Some(b) = &mut self.browser else { return };
        if b.nodes[n].is_dir {
            sfx::play(Sfx::Click);
            b.go(n);
            return;
        }
        let k = kind_of(&ext_of(&b.nodes[n].name));
        if matches!(k, Kind::Text | Kind::Code) || ext_of(&b.nodes[n].name).is_empty() {
            self.view(n);
        } else {
            self.open_file(n, false);
        }
    }

    fn delete(&mut self, nodes: Vec<usize>) {
        let Some(b) = &self.browser else { return };
        let pick = Pick { names: b.entry_names(&nodes), strip: String::new() };
        let o = archive::gui_options(&b.path, b.password.as_deref());
        let (path, pw) = (b.path.clone(), b.password.clone());
        self.set_job(spawn(Work::Delete, b.name_of_archive(), move || {
            let n = archive::delete(&o, Some(&pick))?;
            archive::read_index(&path, pw.as_deref()).map(|i| Output::Reloaded(i, n))
        }));
    }

    fn test(&mut self) {
        let Some(b) = &self.browser else { return };
        let o = archive::gui_options(&b.path, b.password.as_deref());
        self.set_job(spawn(Work::Test, b.name_of_archive(), move || archive::test(&o).map(|_| Output::Tested)));
    }

    fn extract_dialog(&mut self, nodes: Option<Vec<usize>>) {
        let Some(b) = &self.browser else { return };
        sfx::play(Sfx::Pop);
        let dir = b.path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let stem = b.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "archive".into());
        self.overlay = Overlay::Extract(ExtractDlg {
            nodes,
            dest: dir.join(stem).display().to_string(),
            keep_paths: read_setting("unnik_keep_paths").as_deref() != Some("0"),
            exists: match read_setting("unnik_exists").as_deref() {
                Some("overwrite") => Exists::Overwrite,
                Some("skip") => Exists::Skip,
                _ => Exists::Rename,
            },
            show: read_setting("unnik_show").as_deref() != Some("0"),
        });
    }

    fn close_archive(&mut self, ctx: &egui::Context) {
        self.browser = None;
        self.overlay = Overlay::None;
        self.menu = None;
        self.resize(ctx, false);
    }

    fn resize(&mut self, ctx: &egui::Context, big: bool) {
        if self.big == big {
            return;
        }
        self.big = big;
        let size = if big { BIG } else { SMALL };
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(big));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(if big { BIG_MIN } else { SMALL }));
        let size = match monitor {
            Some(m) if big => Vec2::new(size.x.min(m.x - 40.0), size.y.min(m.y - 60.0)),
            _ => size,
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        if let Some(o) = outer {
            let mut pos = o.center() - size / 2.0;
            pos.y = pos.y.max(o.top().min(40.0).max(0.0));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos.max(Pos2::ZERO)));
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let done = self.job.as_ref().is_some_and(|j| j.handle.is_finished());
        if !done {
            return;
        }
        let job = self.job.take().unwrap();
        let cancelled = PROGRESS.cancel.load(Relaxed);
        let res = job.handle.join().unwrap_or_else(|_| Err(NikError::Fatal(t("Internal error.").into())));
        let sound = match &res {
            Ok(Output::Index(_)) => Some(Sfx::Open),
            Ok(Output::Reloaded(..)) => Some(Sfx::Delete),
            Ok(Output::Extracted { warnings, .. }) if warnings.is_empty() => Some(Sfx::Success),
            Ok(Output::Extracted { .. }) => Some(Sfx::Error),
            Ok(Output::Temp { .. }) => None,
            Ok(Output::Text { .. }) => Some(Sfx::Pop),
            Ok(Output::Tested) => Some(Sfx::Success),
            Err(_) if cancelled => Some(Sfx::Cancel),
            Err(_) => Some(Sfx::Error),
        };
        if let Some(snd) = sound {
            sfx::play(snd);
        }
        match res {
            Ok(Output::Index(idx)) => {
                if let Some((path, pw)) = self.opening.take() {
                    add_recent(&path);
                    self.browser = Some(Browser::new(path, pw, idx));
                    self.overlay = Overlay::None;
                    self.error_idle = None;
                    match self.quick {
                        Some(Quick::Here | Quick::ToFolder) => self.quick_extract(),
                        Some(Quick::Dialog) => {
                            // from now on a normal browser window
                            self.quick = None;
                            self.queue.clear();
                            self.resize(ctx, true);
                            self.extract_dialog(None);
                        }
                        None => self.resize(ctx, true),
                    }
                }
            }
            Ok(Output::Reloaded(idx, n)) => {
                if let Some(b) = &mut self.browser {
                    b.reloaded(idx);
                }
                let text = if n == 1 { t("1 item deleted from the archive.").to_string() } else { tf("{} items deleted from the archive.", &[&n.to_string()]) };
                self.toast = Some(Toast { text, folder: None, until: Instant::now() + Duration::from_secs(6) });
            }
            Ok(Output::Extracted { dest, files, warnings, show }) if self.quick_small() => {
                if warnings.is_empty() {
                    // next archive of the selection, or done: close the window
                    if !self.next_quick() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else {
                    let _ = (dest, files, show);
                    self.queue.clear();
                    let mut body = warnings.iter().take(8).cloned().collect::<Vec<_>>().join("\n");
                    if warnings.len() > 8 {
                        body.push_str(&format!("\n… (+{})", warnings.len() - 8));
                    }
                    self.overlay = Overlay::Message { title: t("Extracted, with warnings").into(), body, ok: false };
                }
            }
            Ok(Output::Extracted { dest, files, warnings, show }) => {
                if warnings.is_empty() {
                    let text = tf("{} extracted to {}", &[&files_str(files as u64), &dest.display().to_string()]);
                    self.toast = Some(Toast { text, folder: Some(dest.clone()), until: Instant::now() + Duration::from_secs(8) });
                    if show {
                        open_folder(&dest);
                    }
                } else {
                    let mut body = warnings.iter().take(8).cloned().collect::<Vec<_>>().join("\n");
                    if warnings.len() > 8 {
                        body.push_str(&format!("\n… (+{})", warnings.len() - 8));
                    }
                    self.overlay = Overlay::Message { title: t("Extracted, with warnings").into(), body, ok: false };
                }
            }
            Ok(Output::Temp { file, notepad }) => {
                if notepad {
                    open_in_notepad(&file);
                } else {
                    open_link(&file.display().to_string());
                }
            }
            Ok(Output::Text { name, size, data, more }) => {
                let n = self.pending_view.take().unwrap_or(0);
                let base = name.rsplit('/').next().unwrap_or(&name).to_string();
                let mut v = Viewer::new(n, base, size, data, more);
                if let Overlay::Viewer(old) = &self.overlay {
                    v.wrap = old.wrap;
                }
                self.overlay = Overlay::Viewer(Box::new(v));
            }
            Ok(Output::Tested) => {
                self.overlay = Overlay::Message { title: t("The archive is intact").into(), body: t("Every file was decompressed and its checksum verified.").into(), ok: true };
            }
            Err(_) if cancelled => {
                self.queue.clear();
                if job.work == Work::Open {
                    self.opening = None;
                }
                self.pending_view = None;
                self.toast = Some(Toast { text: t("Cancelled.").into(), folder: None, until: Instant::now() + Duration::from_secs(4) });
            }
            Err(e) => {
                self.pending_view = None;
                if !matches!(e, NikError::Password(_)) {
                    self.queue.clear();
                }
                if job.work == Work::Open {
                    let (path, _) = self.opening.take().unwrap_or_default();
                    if let NikError::Password(_) = e {
                        self.overlay = Overlay::Password { path, password: String::new(), error: Some(t("Wrong password.").into()) };
                        self.pw_focus = true;
                        return;
                    }
                    self.error_idle = Some(e.to_string());
                }
                let title = match job.work {
                    Work::Open => t("Cannot open the archive"),
                    Work::Extract => t("Extraction failed"),
                    Work::Delete => t("Nothing was deleted"),
                    Work::Test => t("The archive is damaged"),
                    Work::Read => t("Cannot read this file"),
                };
                self.fail(title, i18n::t_dyn(&e.to_string()));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Ic {
    Back,
    Forward,
    Up,
    Search,
    Extract,
    View,
    Open,
    Delete,
    Test,
    Details,
    Notepad,
    Copy,
    Wrap,
    Hex,
    Archive,
    Lock,
    Folder,
    Prev,
    Next,
    Maximize,
    Eject,
    Reveal,
}

/// Line icons, drawn in a box of `s` x `s` around `c`.
fn glyph(p: &egui::Painter, c: Pos2, ic: Ic, col: Color32, s: f32) {
    let k = s / 18.0;
    let v = |x: f32, y: f32| c + Vec2::new(x * k, y * k);
    let st = Stroke::new(1.6_f32.max(1.5 * k), col);
    let line = |pts: Vec<Pos2>| {
        p.add(PathShape::line(pts, st));
    };
    match ic {
        Ic::Back => line(vec![v(4.0, -6.0), v(-3.0, 0.0), v(4.0, 6.0)]),
        Ic::Forward => line(vec![v(-4.0, -6.0), v(3.0, 0.0), v(-4.0, 6.0)]),
        Ic::Prev => line(vec![v(-6.0, 3.0), v(0.0, -3.5), v(6.0, 3.0)]),
        Ic::Next => line(vec![v(-6.0, -3.0), v(0.0, 3.5), v(6.0, -3.0)]),
        Ic::Up => {
            line(vec![v(0.0, 7.0), v(0.0, -7.0)]);
            line(vec![v(-6.0, -1.0), v(0.0, -7.0), v(6.0, -1.0)]);
        }
        Ic::Search => {
            p.circle_stroke(v(-1.5, -1.5), 5.5 * k, st);
            line(vec![v(2.6, 2.6), v(7.0, 7.0)]);
        }
        Ic::Extract => {
            line(vec![v(0.0, -8.0), v(0.0, 3.0)]);
            line(vec![v(-4.5, -1.5), v(0.0, 3.0), v(4.5, -1.5)]);
            line(vec![v(-8.0, 2.0), v(-8.0, 7.5), v(8.0, 7.5), v(8.0, 2.0)]);
        }
        Ic::View => {
            let mut top = Vec::new();
            let mut bot = Vec::new();
            for i in 0..=16 {
                let x = -8.5 + 17.0 * i as f32 / 16.0;
                let y = 5.5 * (1.0 - (x / 8.5).powi(2)).sqrt();
                top.push(v(x, -y));
                bot.push(v(x, y));
            }
            line(top);
            line(bot);
            p.circle_stroke(c, 2.6 * k, st);
        }
        Ic::Open => {
            line(vec![v(1.0, -7.0), v(-7.0, -7.0), v(-7.0, 7.0), v(7.0, 7.0), v(7.0, -1.0)]);
            line(vec![v(-1.0, 1.0), v(7.5, -7.5)]);
            line(vec![v(2.5, -7.5), v(7.5, -7.5), v(7.5, -2.5)]);
        }
        Ic::Delete => {
            line(vec![v(-7.5, -5.0), v(7.5, -5.0)]);
            line(vec![v(-3.0, -5.0), v(-3.0, -7.5), v(3.0, -7.5), v(3.0, -5.0)]);
            line(vec![v(-5.5, -5.0), v(-4.5, 7.5), v(4.5, 7.5), v(5.5, -5.0)]);
            line(vec![v(-1.5, -1.5), v(-1.5, 4.5)]);
            line(vec![v(1.5, -1.5), v(1.5, 4.5)]);
        }
        Ic::Test => {
            line(vec![v(0.0, -8.0), v(7.0, -5.0), v(6.0, 2.0), v(0.0, 8.0), v(-6.0, 2.0), v(-7.0, -5.0), v(0.0, -8.0)]);
            line(vec![v(-3.5, 0.0), v(-1.0, 2.8), v(3.8, -2.6)]);
        }
        Ic::Details => {
            p.rect_stroke(Rect::from_center_size(c, Vec2::new(16.0 * k, 14.0 * k)), rr(2), st, egui::StrokeKind::Middle);
            line(vec![v(3.0, -7.0), v(3.0, 7.0)]);
        }
        Ic::Notepad => {
            line(vec![v(-6.0, -8.0), v(-6.0, 8.0), v(6.0, 8.0), v(6.0, -8.0), v(-6.0, -8.0)]);
            for y in [-3.5, 0.0, 3.5] {
                line(vec![v(-3.0, y), v(3.0, y)]);
            }
        }
        Ic::Copy => {
            p.rect_stroke(Rect::from_min_size(v(-7.0, -4.0), Vec2::new(11.0 * k, 12.0 * k)), rr(2), st, egui::StrokeKind::Middle);
            line(vec![v(-3.0, -4.5), v(-3.0, -8.0), v(7.5, -8.0), v(7.5, 4.0), v(4.5, 4.0)]);
        }
        Ic::Wrap => {
            line(vec![v(-8.0, -5.0), v(8.0, -5.0)]);
            line(vec![v(-8.0, 0.0), v(5.0, 0.0)]);
            let mut arc = arc_points(v(5.0, 2.75), 2.75 * k, -std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
            arc.push(v(1.0, 5.5));
            line(arc);
            line(vec![v(3.0, 3.5), v(1.0, 5.5), v(3.0, 7.5)]);
            line(vec![v(-8.0, 5.5), v(-3.0, 5.5)]);
        }
        Ic::Hex => {
            text(p, c, Align2::CENTER_CENTER, "0x", FontId::new(11.0 * k, family("ui")), col);
        }
        Ic::Archive => {
            p.rect_stroke(Rect::from_center_size(v(0.0, 1.0), Vec2::new(16.0 * k, 13.0 * k)), rr(2), st, egui::StrokeKind::Middle);
            line(vec![v(-8.0, -2.5), v(8.0, -2.5)]);
            line(vec![v(-6.0, -5.5), v(-4.0, -8.0), v(4.0, -8.0), v(6.0, -5.5)]);
            line(vec![v(-2.5, 1.5), v(2.5, 1.5)]);
        }
        Ic::Lock => {
            p.rect_filled(Rect::from_center_size(v(0.0, 2.5), Vec2::new(12.0 * k, 9.0 * k)), rr(2), col);
            let mut arc = arc_points(v(0.0, -2.0), 4.0 * k, std::f32::consts::PI, std::f32::consts::TAU);
            arc.insert(0, v(-4.0, 0.0));
            arc.push(v(4.0, 0.0));
            line(arc);
        }
        Ic::Folder => {
            p.add(Shape::convex_polygon(vec![v(-8.0, -6.0), v(-2.5, -6.0), v(-0.5, -4.0), v(8.0, -4.0), v(8.0, 6.5), v(-8.0, 6.5)], col, Stroke::NONE));
        }
        Ic::Reveal => crate::gui::folder_open_glyph(p, c, col),
        Ic::Eject => {
            p.add(Shape::convex_polygon(vec![v(0.0, -7.5), v(7.0, 1.0), v(-7.0, 1.0)], Color32::TRANSPARENT, st));
            line(vec![v(-7.0, 6.0), v(7.0, 6.0)]);
        }
        Ic::Maximize => {
            p.rect_stroke(Rect::from_center_size(c, Vec2::splat(10.0 * k)), rr(1), Stroke::new(1.3_f32, col), egui::StrokeKind::Middle);
        }
    }
}

/// A file or folder icon: a folder, or a page with a coloured band and its extension.
fn file_icon(p: &egui::Painter, r: Rect, is_dir: bool, ext: &str) {
    let w = r.width();
    let k = w / 24.0;
    if is_dir {
        let tab = Rect::from_min_size(r.min + Vec2::new(1.0 * k, 3.5 * k), Vec2::new(10.0 * k, 4.0 * k));
        p.rect_filled(tab, CornerRadius::same(rad(2.0 * k) as u8), FOLDER_DARK());
        let body = Rect::from_min_max(r.min + Vec2::new(1.0 * k, 6.0 * k), r.max - Vec2::new(1.0 * k, 3.0 * k));
        p.rect_filled(body, CornerRadius::same(rad(2.5 * k) as u8), FOLDER());
        p.line_segment([Pos2::new(body.left() + 1.0, body.top() + 2.5 * k), Pos2::new(body.right() - 1.0, body.top() + 2.5 * k)], Stroke::new(1.0_f32, alpha(Color32::WHITE, if crate::skin::c().dark { 70 } else { 110 })));
        return;
    }
    let col = kind_color(kind_of(ext));
    let page = Rect::from_min_max(r.min + Vec2::new(3.5 * k, 1.0 * k), r.max - Vec2::new(3.5 * k, 1.0 * k));
    let fold = 5.0 * k;
    let pts = vec![
        page.left_top(),
        Pos2::new(page.right() - fold, page.top()),
        Pos2::new(page.right(), page.top() + fold),
        page.right_bottom(),
        page.left_bottom(),
    ];
    p.add(Shape::convex_polygon(pts, PAGE(), Stroke::new(1.0_f32, alpha(col, 200))));
    p.add(Shape::convex_polygon(
        vec![Pos2::new(page.right() - fold, page.top()), Pos2::new(page.right(), page.top() + fold), Pos2::new(page.right() - fold, page.top() + fold)],
        alpha(col, 160),
        Stroke::NONE,
    ));
    let band = Rect::from_min_max(Pos2::new(page.left(), page.bottom() - 8.5 * k), Pos2::new(page.right(), page.bottom() - 2.5 * k));
    p.rect_filled(band, rr(1), col);
    if !ext.is_empty() && w >= 20.0 {
        let e: String = ext.chars().take(4).collect::<String>().to_uppercase();
        let size = if e.chars().count() > 3 { 5.2 * k } else { 6.0 * k };
        text(p, band.center() + Vec2::new(0.0, 0.3), Align2::CENTER_CENTER, e, FontId::new(size.max(5.0), family("ui")), if crate::skin::c().nikkoid { Color32::from_rgb(0x05, 0x0C, 0x24) } else { crate::skin::on(col) });
    }
}

fn checkbox(p: &egui::Painter, c: Pos2, on: bool, hover: bool) {
    let r = Rect::from_center_size(c, Vec2::splat(15.0));
    if on {
        p.rect_filled(r, rr(4), ACCENT());
        p.add(PathShape::line(vec![c + Vec2::new(-3.8, 0.2), c + Vec2::new(-1.0, 3.0), c + Vec2::new(4.0, -2.8)], Stroke::new(1.8_f32, ACCENT_FG())));
    } else {
        p.rect_stroke(r, rr(4), Stroke::new(1.2_f32, alpha(ACCENT_SOFT(), if hover { 200 } else { 90 })), egui::StrokeKind::Inside);
    }
}

/// Button with an icon and an optional label. Returns the response.
fn tool_button(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, ic: Ic, label: Option<&str>, enabled: bool, primary: bool, id: &str) -> egui::Response {
    let resp = ui.interact(r, egui::Id::new(id), if enabled { Sense::click() } else { Sense::hover() });
    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
    if enabled && resp.clicked() {
        sfx::play(Sfx::Click);
    }
    let h = resp.hovered() && enabled;
    let round = CornerRadius::same(rad((r.height() / 2.0).min(11.0)) as u8);
    if primary && enabled {
        if h {
            radial_glow(p, r.center(), r.width() * 0.7, alpha(ACCENT(), 50));
        }
        p.rect_filled(r, round, if h { crate::skin::c().accent_btn_hover() } else { crate::skin::c().accent_btn() });
    } else {
        p.rect_filled(r, round, alpha(ACCENT(), if h { 52 } else { 16 }));
        p.rect_stroke(r, round, Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if h { 170 } else if enabled { 70 } else { 30 })), egui::StrokeKind::Inside);
    }
    let col = if !enabled {
        alpha(TEXT_DIM(), 110)
    } else if primary {
        ACCENT_FG()
    } else if h {
        hi(255)
    } else {
        alpha(TEXT(), 225)
    };
    match label {
        None => glyph(p, r.center(), ic, col, 16.0),
        Some(l) => {
            let rtl = i18n::current().rtl();
            let g = lay_fit(p, l, FontId::new(13.0, family("ui")), r.width() - 44.0);
            let total = 18.0 + 8.0 + g.size().x;
            let x0 = r.center().x - total / 2.0;
            let (icx, tx) = if rtl { (x0 + g.size().x + 8.0 + 9.0, x0) } else { (x0 + 9.0, x0 + 26.0) };
            glyph(p, Pos2::new(icx, r.center().y), ic, col, 16.0);
            g.paint(p, Pos2::new(tx, r.center().y - g.size().y / 2.0), col);
        }
    }
    resp
}

fn button_width(p: &egui::Painter, label: &str) -> f32 {
    (lay(p, label, FontId::new(13.0, family("ui"))).size().x + 52.0).clamp(84.0, 190.0)
}

/// Vertical scrolling of `area` (mouse wheel and a slim scroll bar).
fn scroller(ui: &mut egui::Ui, p: &egui::Painter, area: Rect, content_h: f32, offset: &mut f32, id: &str, wheel: bool) {
    let max = (content_h - area.height()).max(0.0);
    if wheel && ui.rect_contains_pointer(area) {
        let d = ui.input(|i| i.smooth_scroll_delta.y);
        *offset -= d;
    }
    *offset = offset.clamp(0.0, max);
    if max <= 0.0 {
        return;
    }
    let track = Rect::from_min_max(Pos2::new(area.right() - 9.0, area.top() + 3.0), Pos2::new(area.right() - 3.0, area.bottom() - 3.0));
    let th = (area.height() / content_h * track.height()).max(30.0).min(track.height());
    let ty = track.top() + (track.height() - th) * (*offset / max);
    let thumb = Rect::from_min_size(Pos2::new(track.left(), ty), Vec2::new(track.width(), th));
    let r = ui.interact(track.expand2(Vec2::new(4.0, 0.0)), egui::Id::new(id), Sense::click_and_drag());
    if r.dragged() {
        *offset += r.drag_delta().y * max / (track.height() - th).max(1.0);
    } else if r.clicked() {
        if let Some(pos) = r.interact_pointer_pos() {
            *offset += if pos.y < thumb.top() { -area.height() } else if pos.y > thumb.bottom() { area.height() } else { 0.0 };
        }
    }
    *offset = offset.clamp(0.0, max);
    let ty = track.top() + (track.height() - th) * (*offset / max);
    let thumb = Rect::from_min_size(Pos2::new(track.left(), ty), Vec2::new(track.width(), th));
    p.rect_filled(track, rr(3), hi(8));
    p.rect_filled(thumb, rr(3), alpha(ACCENT_SOFT(), if r.hovered() || r.dragged() { 170 } else { 80 }));
}

fn pane(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, rr(14), PANEL_BG());
    p.rect_stroke(r, rr(14), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 38)), egui::StrokeKind::Inside);
}

fn small_label(p: &egui::Painter, pos: Pos2, s: &str) -> Rect {
    let g = spaced(p, s, FontId::new(9.5, family("ui")), TEXT_DIM(), 1.6);
    let at = label_pos(&g, pos);
    g.paint(p, at, TEXT_DIM());
    Rect::from_min_size(at, g.size())
}

/// A text field drawn in NikPak's style.
fn field(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, value: &mut String, hint: &'static str, id: &str, password: bool) -> egui::Response {
    p.rect_filled(r, rr(10), alpha(crate::skin::c().field, 190));
    let focused = ui.memory(|m| m.has_focus(egui::Id::new(id)));
    p.rect_stroke(r, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if focused { 170 } else { 70 })), egui::StrokeKind::Inside);
    let edit = egui::TextEdit::singleline(value)
        .id(egui::Id::new(id))
        .font(FontId::new(13.0, family("ui-light")))
        .text_color(TEXT())
        .frame(false)
        .password(password)
        .margin(egui::Margin::symmetric(12, 0))
        .vertical_align(egui::Align::Center)
        .hint_text(tip(hint));
    ui.put(r, edit)
}

/// The compression ring of the summary card.
fn donut(p: &egui::Painter, c: Pos2, r: f32, frac: f32) {
    p.circle_stroke(c, r, Stroke::new(7.0_f32, TRACK()));
    if frac > 0.0 {
        let a0 = -std::f32::consts::FRAC_PI_2;
        let pts = arc_points(c, r, a0, a0 + frac.clamp(0.0, 1.0) * std::f32::consts::TAU);
        p.add(PathShape::line(pts.clone(), Stroke::new(12.0_f32, alpha(ACCENT(), 30))));
        p.add(PathShape::line(pts, Stroke::new(7.0_f32, ACCENT())));
    }
}

fn method_name(idx: &Index) -> String {
    match idx.method {
        crate::format::METHOD_STORE => t("Stored (no compression)").to_string(),
        crate::format::METHOD_ZSTD => "Zstandard".to_string(),
        _ => {
            let mut names: Vec<String> = Vec::new();
            for e in &idx.engines {
                let n = e.split(',').next().unwrap_or(e).split(' ').next().unwrap_or(e).to_string();
                if !names.contains(&n) {
                    names.push(n);
                }
            }
            if names.is_empty() { "LZMA2".into() } else { names.join(" + ") }
        }
    }
}

fn type_name(node: &Node) -> String {
    if node.is_dir {
        return t("Folder").to_string();
    }
    let e = ext_of(&node.name);
    if e.is_empty() { t("File").to_string() } else { tf("{} file", &[&e.to_uppercase()]) }
}

#[derive(Clone)]
enum Act {
    Activate(usize),
    View(usize),
    Open(usize),
    Notepad(usize),
    Extract(Option<Vec<usize>>),
    Delete(Vec<usize>),
    Test,
    Pick,
    Go(usize),
    Up,
    Back,
    Forward,
    Menu(Pos2, usize),
    CopyName(usize),
    SelectAll,
    Close,
    SwitchNikPak,
}

// ---------------------------------------------------------------------------
// The window
// ---------------------------------------------------------------------------

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(job) = self.job.take() {
            PROGRESS.cancel.store(true, Relaxed);
            let _ = job.handle.join(); // a cancelled delete removes its temporary file
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll(ctx);
        crate::gui::follow_language(ctx);
        crate::skin::sync_style(ctx);
        let parent: &eframe::Frame = frame;

        // ---- dropped archives ------------------------------------------------------
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        let files_over = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if files_over && !self.was_hover {
            sfx::play(Sfx::Hover);
        }
        self.was_hover = files_over;
        if let Some(first) = dropped.first() {
            if self.job.is_none() && matches!(self.overlay, Overlay::None | Overlay::Message { .. }) {
                let nik = dropped.iter().find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nik"))).cloned();
                match nik.or_else(|| archive::peek(first).ok().map(|_| first.clone())) {
                    Some(p) => {
                        self.overlay = Overlay::None;
                        self.open(p, None);
                    }
                    None => self.fail(t("This is not a .nik archive"), t("unNik opens .nik archives. To create one, drop your files on NikPak.").into()),
                }
            }
        }

        let mut acts: Vec<Act> = Vec::new();
        let mut panel_rect = Rect::NOTHING;
        let mut window_rect = Rect::NOTHING;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let full = ui.max_rect();
            window_rect = full;
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            let panel = if maximized { full } else { full.shrink(1.0) };
            panel_rect = panel;
            if self.browsing() {
                self.draw_browser(ui, ctx, panel, maximized, &mut acts);
            } else {
                self.draw_idle(ui, ctx, panel, parent, &mut acts);
            }
        });

        self.keyboard(ctx, &mut acts);
        self.draw_menu(ctx, panel_rect, &mut acts);
        self.draw_overlay(ctx, panel_rect, parent, &mut acts);
        self.draw_busy(ctx, panel_rect);
        self.draw_toast(ctx, panel_rect);
        if matches!(self.overlay, Overlay::None) && self.job.is_none() {
            let lb = self.lang_rect(panel_rect);
            lang_menu(ctx, panel_rect, lb, &mut self.lang_open, &start_args(&self.browser));
            crate::gui::skin_menu(ctx, parent, panel_rect, window_rect, &mut self.skins);
        }
        self.run(ctx, parent, acts);
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(if self.browsing() { 250 } else { 33 }));
        }
    }
}

impl App {
    fn lang_rect(&self, panel: Rect) -> Rect {
        if self.browsing() {
            Rect::from_min_size(Pos2::new(panel.left() + 16.0, panel.bottom() - FOOT_H + 10.0), Vec2::new(170.0, 30.0))
        } else {
            crate::gui::lang_button_rect(panel)
        }
    }

    fn run(&mut self, ctx: &egui::Context, parent: &eframe::Frame, acts: Vec<Act>) {
        for a in acts {
            if self.job.is_some() {
                break;
            }
            match a {
                Act::Activate(n) => self.activate(n),
                Act::View(n) => self.view(n),
                Act::Open(n) => self.open_file(n, false),
                Act::Notepad(n) => self.open_file(n, true),
                Act::Extract(ns) => self.extract_dialog(ns),
                Act::Delete(ns) if !ns.is_empty() => {
                    sfx::play(Sfx::Pop);
                    self.overlay = Overlay::Delete(ns);
                }
                Act::Delete(_) => {}
                Act::Test => self.test(),
                Act::Pick => {
                    let f = rfd::FileDialog::new().set_parent(parent).set_title(t("Choose a .nik archive")).add_filter("NikPak", &["nik"]).pick_file();
                    if let Some(f) = f {
                        self.open(f, None);
                    }
                }
                Act::Go(n) => {
                    if let Some(b) = &mut self.browser {
                        b.go(n);
                    }
                }
                Act::Up => {
                    if let Some(b) = &mut self.browser {
                        b.up();
                    }
                }
                Act::Back => {
                    if let Some(b) = &mut self.browser {
                        b.go_back();
                    }
                }
                Act::Forward => {
                    if let Some(b) = &mut self.browser {
                        b.go_forward();
                    }
                }
                Act::Menu(pos, n) => {
                    sfx::play(Sfx::Pop);
                    self.menu = Some((pos, n));
                }
                Act::CopyName(n) => {
                    if let Some(b) = &self.browser {
                        ctx.copy_text(b.nodes[n].path.clone());
                    }
                }
                Act::SelectAll => {
                    if let Some(b) = &mut self.browser {
                        b.selected = b.rows.iter().copied().collect();
                    }
                }
                Act::Close => self.close_archive(ctx),
                Act::SwitchNikPak => {
                    if crate::gui::sibling_exe("nikpak").is_none() {
                        // NikPak is not next to unNik: tell the user it is
                        // shareware and where to find it.
                        self.overlay = Overlay::NikPakMissing;
                    } else if let Err(e) = crate::gui::switch_to(ctx, "nikpak", &[]) {
                        self.fail(t("Cannot start NikPak"), e);
                    }
                }
            }
        }
    }

    // ---- keyboard ---------------------------------------------------------------

    fn keyboard(&mut self, ctx: &egui::Context, acts: &mut Vec<Act>) {
        let typing = ctx.memory(|m| m.focused().is_some());
        if self.job.is_some() {
            return;
        }
        if let Overlay::Viewer(_) = &self.overlay {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                let _ = typing;
                self.overlay = Overlay::None;
            }
            if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                self.viewer_find_focus = true;
            }
            return;
        }
        if !matches!(self.overlay, Overlay::None) || self.menu.is_some() || self.lang_open || self.skins.open {
            return;
        }
        let Some(b) = &mut self.browser else { return };
        // the modifiers of each key press (not of the frame: fast typists, remote desktops)
        let presses: Vec<(egui::Key, egui::Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        let mods = |k: egui::Key| presses.iter().find(|(kk, _)| *kk == k).map(|(_, m)| *m);
        let key = |k: egui::Key| mods(k).is_some();
        let cmd = presses.iter().any(|(_, m)| m.command);
        let shift = presses.iter().any(|(_, m)| m.shift);
        let alt = presses.iter().any(|(_, m)| m.alt);
        if cmd && key(egui::Key::F) {
            self.search_focus = true;
        }
        if typing {
            if key(egui::Key::Escape) {
                b.search.clear();
                b.dirty = true;
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("list-focus")));
            }
            if key(egui::Key::ArrowDown) {
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("list-focus")));
                b.move_focus(1, false);
            }
            return;
        }
        if cmd && key(egui::Key::A) {
            acts.push(Act::SelectAll);
        }
        if key(egui::Key::ArrowDown) {
            b.move_focus(1, shift);
        }
        if key(egui::Key::ArrowUp) {
            b.move_focus(-1, shift);
        }
        if key(egui::Key::PageDown) {
            b.move_focus(12, shift);
        }
        if key(egui::Key::PageUp) {
            b.move_focus(-12, shift);
        }
        if key(egui::Key::Home) {
            b.move_focus(-1_000_000, shift);
        }
        if key(egui::Key::End) {
            b.move_focus(1_000_000, shift);
        }
        if key(egui::Key::Enter) {
            if let Some(f) = b.focus.filter(|f| b.rows.contains(f)) {
                acts.push(Act::Activate(f));
            }
        }
        if key(egui::Key::Backspace) || (alt && key(egui::Key::ArrowUp)) {
            acts.push(Act::Up);
        }
        if alt && key(egui::Key::ArrowLeft) {
            acts.push(Act::Back);
        }
        if alt && key(egui::Key::ArrowRight) {
            acts.push(Act::Forward);
        }
        if key(egui::Key::Delete) {
            acts.push(Act::Delete(b.selection()));
        }
        if key(egui::Key::F3) {
            if let Some(&n) = b.selection().first() {
                if !b.nodes[n].is_dir {
                    acts.push(Act::View(n));
                }
            }
        }
        if cmd && key(egui::Key::E) {
            let sel = b.selection();
            acts.push(Act::Extract(if sel.is_empty() { None } else { Some(sel) }));
        }
        if key(egui::Key::Escape) {
            if !b.search.is_empty() {
                b.search.clear();
                b.dirty = true;
            } else {
                b.selected.clear();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The empty window: the drop ring with an arrow going up
// ---------------------------------------------------------------------------

impl App {
    fn draw_idle(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, panel: Rect, _parent: &eframe::Frame, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        let now = self.start.elapsed().as_secs_f32();
        crate::skin::paint_background(ui.ctx(), &p, panel, rad(RADIUS));
        let ring_c = Pos2::new(panel.center().x, panel.top() + 208.0);
        radial_glow(&p, ring_c, 200.0, alpha(ACCENT(), 34));
        radial_glow(&p, Pos2::new(panel.center().x, panel.top() - 40.0), 260.0, alpha(ACCENT_SOFT(), 18));
        p.rect_stroke(panel, rr(RADIUS as u8), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 46)), egui::StrokeKind::Inside);

        let bar = Rect::from_min_size(panel.min, Vec2::new(panel.width(), 110.0));
        let drag = ui.interact(bar, egui::Id::new("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        let close = Rect::from_center_size(Pos2::new(panel.right() - 26.0, panel.top() + 26.0), Vec2::splat(26.0));
        if crate::gui::window_buttons(ui, &p, ctx, close, "About unNik") && matches!(self.overlay, Overlay::None) {
            self.overlay = Overlay::About;
            sfx::play(Sfx::Pop);
        }
        let sw = close.translate(Vec2::new(-96.0, 0.0));
        let busy = self.job.is_some();
        if crate::gui::switch_button(ui, &p, sw, false, !busy).clicked() && !busy && matches!(self.overlay, Overlay::None) {
            acts.push(Act::SwitchNikPak);
        }
        crate::gui::sound_button(ui, &p, sw.translate(Vec2::new(-32.0, 0.0)));
        crate::gui::theme_button(ui, &p, sw.translate(Vec2::new(-64.0, 0.0)));
        if crate::gui::skin_button(ui, &p, sw.translate(Vec2::new(-96.0, 0.0)), self.skins.open) && matches!(self.overlay, Overlay::None) {
            self.skins.toggle();
            self.lang_open = false;
        }
        // free: no evaluation notice, no registration
        let g = spaced_fit(&p, t("FREEWARE  ·  NO REGISTRATION"), FontId::new(9.5, family("ui")), TEXT_DIM(), 1.2, 232.0);
        g.paint(&p, label_pos(&g, Pos2::new(panel.left() + 22.0, panel.top() + 19.0)), TEXT_DIM());
        logo(&p, Pos2::new(panel.center().x, panel.top() + 56.0), "unNik", 30.0);

        let r = 92.0;
        let status_y = ring_c.y + r + 34.0;
        if let Some(job) = &self.job {
            // opening: NikPak's progress ring
            let total = PROGRESS.total.load(Relaxed).max(1);
            let done = PROGRESS.done.load(Relaxed).min(total);
            let target = (done as f32 / total as f32 * 99.0).min(99.0);
            self.shown += (target - self.shown) * 0.18;
            p.circle_stroke(ring_c, r, Stroke::new(9.0_f32, TRACK()));
            progress_arc(&p, ring_c, r, self.shown / 100.0, ACCENT());
            big_percent(&p, ring_c, self.shown);
            let verb = if job.work == Work::Extract { t("EXTRACTING") } else { t("OPENING") };
            glow_text(&p, Pos2::new(ring_c.x, status_y), verb, FontId::new(12.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
            text_fit(&p, Pos2::new(ring_c.x, status_y + 21.0), Align2::CENTER_CENTER, &job.label, FontId::new(14.0, family("ui-light")), TEXT(), panel.width() - 60.0);
            text(&p, Pos2::new(ring_c.x, status_y + 39.0), Align2::CENTER_CENTER, format!("{}  ·  {:.0} s", tf("{} of {}", &[&human(done), &human(total)]), job.started.elapsed().as_secs_f32()), FontId::new(12.0, family("ui-light")), TEXT_DIM());
            let cancel = Rect::from_center_size(Pos2::new(ring_c.x, status_y + 68.0), Vec2::new(140.0, 32.0));
            if !PROGRESS.cancel.load(Relaxed) && (pill_button(ui, &p, cancel, t("Cancel"), false, "cancel-open") || ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                sfx::play(Sfx::Cancel);
                PROGRESS.cancel.store(true, Relaxed);
            }
        } else {
            let zone = Rect::from_center_size(ring_c, Vec2::splat(2.0 * r));
            let zr = ui.interact(zone, egui::Id::new("dropzone"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let files_hover = ctx.input(|i| !i.raw.hovered_files.is_empty()) || std::env::var_os("NIKPAK_GUI_HOVER").is_some();
            let hovering = files_hover || zr.hovered();
            let (ring_a, glow_a) = if hovering { (230u8, 60u8) } else { (110u8, 0u8) };
            if hovering {
                radial_glow(&p, ring_c, r + 40.0, alpha(ACCENT(), 50));
            }
            let segs = 48;
            for k in 0..segs {
                let a0 = -now * 0.25 + k as f32 / segs as f32 * std::f32::consts::TAU;
                let a1 = a0 + std::f32::consts::TAU / segs as f32 * 0.55;
                let pts = arc_points(ring_c, r, a0, a1);
                if glow_a > 0 {
                    p.add(PathShape::line(pts.clone(), Stroke::new(8.0_f32, alpha(ACCENT(), glow_a))));
                }
                p.add(PathShape::line(pts, Stroke::new(2.0_f32, alpha(ACCENT_SOFT(), ring_a))));
            }
            p.circle_stroke(ring_c, r + 14.0, Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 30)));
            // the unNik icon: an arrow going UP out of the tray
            let ic = ring_c + Vec2::new(0.0, -(now * 2.2).sin().abs() * if hovering { 5.0 } else { 2.0 });
            let s = Stroke::new(3.0_f32, if hovering { TEXT() } else { ACCENT_SOFT() });
            let halo = Stroke::new(9.0_f32, alpha(ACCENT(), 28));
            let arrow = [
                vec![ic + Vec2::new(0.0, 8.0), ic + Vec2::new(0.0, -40.0)],
                vec![ic + Vec2::new(-17.0, -23.0), ic + Vec2::new(0.0, -40.0), ic + Vec2::new(17.0, -23.0)],
                vec![ring_c + Vec2::new(-36.0, 14.0), ring_c + Vec2::new(-36.0, 36.0), ring_c + Vec2::new(36.0, 36.0), ring_c + Vec2::new(36.0, 14.0)],
            ];
            for line in &arrow {
                p.add(PathShape::line(line.clone(), halo));
            }
            for line in arrow {
                p.add(PathShape::line(line, s));
            }
            let msg = if files_hover { t("Release to open") } else { t("Drop a .nik archive to open it") };
            text_fit(&p, Pos2::new(ring_c.x, status_y), Align2::CENTER_CENTER, msg, FontId::new(16.0, family("ui-light")), TEXT(), panel.width() - 70.0);
            let small = FontId::new(13.0, family("ui-light"));
            let mut parts: Vec<(usize, &str)> = vec![(0, t("or")), (1, t("choose an archive"))];
            if i18n::current().rtl() {
                parts.reverse();
            }
            let laid: Vec<Txt> = parts.iter().map(|(k, s)| lay(&p, s, if *k == 1 { crate::gui::link_font() } else { small.clone() })).collect();
            let gap = 7.0;
            let total: f32 = laid.iter().map(|l| l.size().x).sum::<f32>() + gap;
            let mut x = ring_c.x - total / 2.0;
            let row_c = status_y + 26.0;
            for ((k, s), l) in parts.iter().zip(&laid) {
                let pos = Pos2::new(x, row_c - l.size().y / 2.0);
                if *k == 1 {
                    if crate::gui::text_link(ui, &p, pos, s, "pick-archive") {
                        acts.push(Act::Pick);
                    }
                } else {
                    l.paint(&p, pos, TEXT_DIM());
                }
                x += l.size().x + gap;
            }
            if zr.clicked() {
                acts.push(Act::Pick);
            }
            let note_y = status_y + 50.0;
            if let Some(e) = &self.error_idle {
                let short: String = e.lines().next().unwrap_or("").chars().take(80).collect();
                text_fit(&p, Pos2::new(ring_c.x, note_y), Align2::CENTER_CENTER, short, FontId::new(12.0, family("ui-light")), ERR_RED(), panel.width() - 40.0);
            } else if self.elevated {
                text_fit(&p, Pos2::new(ring_c.x, note_y), Align2::CENTER_CENTER, t("Running as administrator: Windows blocks drag and drop from Explorer."), FontId::new(11.5, family("ui-light")), WARN(), panel.width() - 40.0);
            }
        }

        // ---- recent archives -------------------------------------------------------------
        let label_y = panel.bottom() - 140.0;
        let rec = recent();
        if rec.is_empty() {
            wrapped(&p, Pos2::new(panel.left() + 30.0, label_y + 10.0), panel.width() - 60.0, t("unNik is free for everyone. No registration, no time limit."), 12.5, TEXT_DIM());
        } else {
            let lab = spaced(&p, t("RECENT ARCHIVES"), FontId::new(10.5, family("ui")), TEXT_DIM(), 2.0);
            crate::gui::paint_label(&p, &lab, panel.left() + 28.0, panel.right() - 28.0, label_y, TEXT_DIM());
            for (k, path) in rec.iter().take(2).enumerate() {
                let row = Rect::from_min_size(Pos2::new(panel.left() + 20.0, label_y + 20.0 + k as f32 * 30.0), Vec2::new(panel.width() - 40.0, 28.0));
                let resp = ui.interact(row, egui::Id::new(("recent", k)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() {
                    p.rect_filled(row, rr(9), alpha(ACCENT(), 34));
                }
                file_icon(&p, Rect::from_center_size(Pos2::new(row.left() + 18.0, row.center().y), Vec2::splat(20.0)), false, "nik");
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ng = elide(&p, &name, FontId::new(13.0, family("ui")), 180.0);
                ng.paint(&p, Pos2::new(row.left() + 36.0, row.center().y - ng.size().y / 2.0), TEXT());
                let dir = path.parent().map(|d| d.display().to_string()).unwrap_or_default();
                let dx = row.left() + 36.0 + ng.size().x + 10.0;
                let dg = elide(&p, &dir, FontId::new(11.5, family("ui-light")), row.right() - 10.0 - dx);
                dg.paint(&p, Pos2::new(dx, row.center().y - dg.size().y / 2.0), TEXT_DIM());
                if resp.clicked() && self.job.is_none() {
                    self.open(path.clone(), None);
                }
            }
        }

        let icon_y = panel.bottom() - 32.0;
        social_buttons(ui, &p, [0, 1, 2].map(|k| Pos2::new(panel.right() - 44.0 - (2 - k) as f32 * 58.0, icon_y)), 19.0);
        let lb = crate::gui::lang_button_rect(panel);
        if lang_button(ui, &p, lb, self.lang_open, self.job.is_some()) {
            self.lang_open = !self.lang_open;
        }
    }
}

fn big_percent(p: &egui::Painter, c: Pos2, shown: f32) {
    let num = p.layout_no_wrap(format!("{}", shown.round() as u32), FontId::new(84.0, family("display-light")), TEXT());
    let sign = p.layout_no_wrap("%".into(), FontId::new(26.0, family("display")), TEXT());
    let wsum = num.size().x + 4.0 + sign.size().x;
    let npos = Pos2::new(c.x - wsum / 2.0, c.y - num.size().y / 2.0 - 2.0);
    let spos = Pos2::new(npos.x + num.size().x + 4.0, npos.y + num.size().y * 0.80 - sign.size().y);
    p.galley(npos, num, TEXT());
    p.galley(spos, sign, TEXT());
}

// ---------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------

impl App {
    fn draw_browser(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, panel: Rect, maximized: bool, acts: &mut Vec<Act>) {
        let p = ui.painter().clone();
        let radius = if maximized { 0.0 } else { RADIUS };
        crate::skin::paint_background(ctx, &p, panel, rad(radius));
        radial_glow(&p, Pos2::new(panel.left() + 180.0, panel.top() - 60.0), 420.0, alpha(ACCENT_SOFT(), 16));
        radial_glow(&p, Pos2::new(panel.center().x, panel.bottom() + 120.0), 520.0, alpha(ACCENT(), 14));
        p.rect_stroke(panel, CornerRadius::same(rad(radius) as u8), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 46)), egui::StrokeKind::Inside);
        let quiet = matches!(self.overlay, Overlay::None) && self.menu.is_none() && self.job.is_none() && !self.lang_open;
        let search_focus = std::mem::take(&mut self.search_focus);
        let busy = self.job.is_some();
        let b = self.browser.as_mut().unwrap();
        b.refresh_rows();

        // ---- title bar ------------------------------------------------------------------
        let title = Rect::from_min_size(panel.min, Vec2::new(panel.width() - 284.0, TITLE_H));
        let drag = ui.interact(title, egui::Id::new("titlebar"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        let ic = Pos2::new(panel.left() + 32.0, panel.top() + TITLE_H / 2.0);
        p.circle_filled(ic, 15.0, crate::skin::c().panel_top);
        p.circle_stroke(ic, 12.0, Stroke::new(2.0_f32, ACCENT()));
        p.add(PathShape::line(vec![ic + Vec2::new(0.0, 6.0), ic + Vec2::new(0.0, -6.5)], Stroke::new(2.0_f32, TEXT())));
        p.add(PathShape::line(vec![ic + Vec2::new(-4.5, -2.0), ic + Vec2::new(0.0, -6.5), ic + Vec2::new(4.5, -2.0)], Stroke::new(2.0_f32, TEXT())));
        let lr = glow_text(&p, Pos2::new(panel.left() + 88.0, panel.top() + TITLE_H / 2.0), "unNik", FontId::new(21.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.5);
        let x0 = lr.right() + 22.0;
        p.line_segment([Pos2::new(x0 - 11.0, ic.y - 13.0), Pos2::new(x0 - 11.0, ic.y + 13.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 50)));
        let (files, dirs) = (b.nodes[0].files, b.nodes[0].dirs);
        let orig = b.nodes[0].size;
        let packed = b.index.archive_size;
        let name = b.name_of_archive();
        let ng = elide(&p, &name, FontId::new(15.0, family("ui")), (title.right() - x0 - 20.0).max(60.0));
        ng.paint(&p, Pos2::new(x0, ic.y - 9.0 - ng.size().y / 2.0), TEXT());
        let mut info = format!("{}  ·  {}  ·  {} → {}", files_str(files as u64), folders_str(dirs as u64), human(orig), human(packed));
        if orig > 0 {
            info.push_str(&format!(" ({:.0} %)", packed as f64 * 100.0 / orig as f64));
        }
        info.push_str(&format!("  ·  {}", method_name(&b.index)));
        let mut ix = x0;
        if b.index.encrypted {
            glyph(&p, Pos2::new(ix + 6.0, ic.y + 10.0), Ic::Lock, OK_GREEN(), 12.0);
            ix += 16.0;
        }
        let ig = elide(&p, &info, FontId::new(11.5, family("ui-light")), (title.right() - ix - 20.0).max(60.0));
        ig.paint(&p, Pos2::new(ix, ic.y + 10.0 - ig.size().y / 2.0), TEXT_DIM());

        // close, maximise, minimise, about
        let close = Rect::from_center_size(Pos2::new(panel.right() - 26.0, panel.top() + TITLE_H / 2.0), Vec2::splat(26.0));
        let (cr, col) = icon_button(ui, close, "close");
        p.line_segment([close.center() + Vec2::new(-5.0, -5.0), close.center() + Vec2::new(5.0, 5.0)], Stroke::new(1.4_f32, col));
        p.line_segment([close.center() + Vec2::new(-5.0, 5.0), close.center() + Vec2::new(5.0, -5.0)], Stroke::new(1.4_f32, col));
        if cr.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        let maxr = close.translate(Vec2::new(-32.0, 0.0));
        let (mr, col) = icon_button(ui, maxr, "maximise");
        glyph(&p, maxr.center(), Ic::Maximize, col, 18.0);
        if mr.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        let minr = maxr.translate(Vec2::new(-32.0, 0.0));
        let (mnr, col) = icon_button(ui, minr, "minimise");
        p.line_segment([minr.center() + Vec2::new(-5.0, 0.0), minr.center() + Vec2::new(5.0, 0.0)], Stroke::new(1.4_f32, col));
        if mnr.clicked() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        let info_r = minr.translate(Vec2::new(-32.0, 0.0));
        let (ir, col) = icon_button(ui, info_r, "about");
        let ir = ir.on_hover_text(tip("About unNik"));
        p.circle_stroke(info_r.center(), 7.5, Stroke::new(1.2_f32, col));
        text(&p, info_r.center() + Vec2::new(0.0, 0.5), Align2::CENTER_CENTER, "i", FontId::new(11.0, family("ui")), col);
        let mut about = ir.clicked();
        let sw = info_r.translate(Vec2::new(-32.0, 0.0));
        if crate::gui::switch_button(ui, &p, sw, false, !busy).clicked() && quiet {
            acts.push(Act::SwitchNikPak);
        }
        crate::gui::sound_button(ui, &p, sw.translate(Vec2::new(-32.0, 0.0)));
        crate::gui::theme_button(ui, &p, sw.translate(Vec2::new(-64.0, 0.0)));
        if crate::gui::skin_button(ui, &p, sw.translate(Vec2::new(-96.0, 0.0)), self.skins.open) && quiet {
            self.skins.toggle();
            self.lang_open = false;
        }

        // ---- row 1: back / forward / up, breadcrumbs, search -----------------------------------
        let y1 = panel.top() + TITLE_H + 2.0;
        let nav = [(Ic::Back, !b.back.is_empty(), "nav-back", "Back (Alt+Left)"), (Ic::Forward, !b.fwd.is_empty(), "nav-fwd", "Forward (Alt+Right)"), (Ic::Up, b.cwd != 0 || !b.search.is_empty(), "nav-up", "Up one folder (Backspace)")];
        for (k, (icn, en, id, hint)) in nav.iter().enumerate() {
            let r = Rect::from_min_size(Pos2::new(panel.left() + 16.0 + k as f32 * 40.0, y1), Vec2::splat(34.0));
            let resp = tool_button(ui, &p, r, *icn, None, *en && !busy, false, id).on_hover_text(tip(hint));
            if resp.clicked() {
                acts.push(match k {
                    0 => Act::Back,
                    1 => Act::Forward,
                    _ => Act::Up,
                });
            }
        }
        let search_w = 290.0f32.min(panel.width() * 0.3);
        let sr = Rect::from_min_size(Pos2::new(panel.right() - 16.0 - search_w, y1), Vec2::new(search_w, 34.0));
        let crumbs_r = Rect::from_min_max(Pos2::new(panel.left() + 16.0 + 3.0 * 40.0 + 6.0, y1), Pos2::new(sr.left() - 12.0, y1 + 34.0));
        p.rect_filled(crumbs_r, rr(11), alpha(crate::skin::c().field, 130));
        p.rect_stroke(crumbs_r, rr(11), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 45)), egui::StrokeKind::Inside);
        {
            // archive > folder > folder ... (the first ones fold into "…" when there is no room)
            let mut chain = vec![b.cwd];
            let mut c = b.cwd;
            while c != 0 {
                c = b.nodes[c].parent;
                chain.push(c);
            }
            chain.reverse();
            let font = FontId::new(13.0, family("ui"));
            let labels: Vec<String> = chain.iter().map(|&n| if n == 0 { name.clone() } else { b.nodes[n].name.clone() }).collect();
            let widths: Vec<f32> = labels.iter().map(|l| lay(&p, l, font.clone()).size().x.min(220.0) + 16.0).collect();
            let room = crumbs_r.width() - 40.0;
            let mut first = 0;
            while first + 1 < chain.len() && widths[first..].iter().sum::<f32>() + 18.0 * (chain.len() - first) as f32 > room {
                first += 1;
            }
            let cp = clipped(&p, crumbs_r.shrink(2.0));
            let mut x = crumbs_r.left() + 10.0;
            glyph(&cp, Pos2::new(x + 8.0, crumbs_r.center().y), Ic::Archive, ACCENT_SOFT(), 15.0);
            x += 22.0;
            if first > 0 {
                text(&cp, Pos2::new(x + 6.0, crumbs_r.center().y), Align2::CENTER_CENTER, "…", font.clone(), TEXT_DIM());
                x += 18.0;
                glyph(&cp, Pos2::new(x + 3.0, crumbs_r.center().y), Ic::Forward, alpha(TEXT_DIM(), 160), 10.0);
                x += 12.0;
            }
            for (i, &n) in chain.iter().enumerate().skip(first) {
                let g = elide(&cp, &labels[i], font.clone(), 220.0);
                let r = Rect::from_min_size(Pos2::new(x, crumbs_r.top() + 4.0), Vec2::new(g.size().x + 14.0, crumbs_r.height() - 8.0));
                let last = n == b.cwd && b.search.trim().is_empty();
                let resp = ui.interact(r.intersect(crumbs_r), egui::Id::new(("crumb", n)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() || last {
                    cp.rect_filled(r, rr(8), alpha(ACCENT(), if resp.hovered() { 60 } else { 30 }));
                }
                g.paint(&cp, Pos2::new(r.left() + 7.0, r.center().y - g.size().y / 2.0), if last { TEXT() } else { alpha(TEXT(), 200) });
                if resp.clicked() && !busy {
                    sfx::play(Sfx::Click);
                    acts.push(Act::Go(n));
                    if n == b.cwd && !b.search.is_empty() {
                        b.search.clear();
                        b.dirty = true;
                    }
                }
                x = r.right() + 2.0;
                if i + 1 < chain.len() {
                    glyph(&cp, Pos2::new(x + 5.0, crumbs_r.center().y), Ic::Forward, alpha(TEXT_DIM(), 160), 10.0);
                    x += 14.0;
                }
            }
            if !b.search.trim().is_empty() {
                let g = lay(&cp, &tf("Search: “{}”", &[b.search.trim()]), FontId::new(12.5, family("ui-light")));
                g.paint(&cp, Pos2::new(x + 10.0, crumbs_r.center().y - g.size().y / 2.0), ACCENT_SOFT());
            }
        }
        // search over the whole archive
        let before = b.search.clone();
        let resp = field(ui, &p, sr.with_min_x(sr.left()), &mut b.search, "Search in the whole archive (Ctrl+F)", "search", false);
        glyph(&p, Pos2::new(sr.right() - 18.0, sr.center().y), Ic::Search, if resp.has_focus() { TEXT() } else { TEXT_DIM() }, 15.0);
        if search_focus {
            resp.request_focus();
        }
        if b.search != before {
            b.dirty = true;
            b.list_scroll = 0.0;
        }

        // ---- row 2: actions ---------------------------------------------------------------------------
        let y2 = y1 + 46.0;
        let sel = b.selection();
        let one_file = (sel.len() == 1 && !b.nodes[sel[0]].is_dir).then(|| sel[0]);
        let n_sel = sel.len();
        let extract_sel_label = if n_sel > 0 { tf("Extract {}", &[&n_sel.to_string()]) } else { t("Extract…").to_string() };
        let actions: [(Ic, String, bool, bool, &str); 6] = [
            (Ic::Extract, t("Extract all").to_string(), true, true, "a-all"),
            (Ic::Extract, extract_sel_label, n_sel > 0, false, "a-sel"),
            (Ic::View, t("View").to_string(), one_file.is_some(), false, "a-view"),
            (Ic::Open, t("Open").to_string(), one_file.is_some(), false, "a-open"),
            (Ic::Delete, t("Delete").to_string(), n_sel > 0, false, "a-del"),
            (Ic::Test, t("Test").to_string(), true, false, "a-test"),
        ];
        let mut x = panel.left() + 16.0;
        for (k, (icn, label, en, primary, id)) in actions.iter().enumerate() {
            let w = button_width(&p, label);
            let r = Rect::from_min_size(Pos2::new(x, y2), Vec2::new(w, 36.0));
            if r.right() > panel.right() - 190.0 {
                break;
            }
            let hint = ["Extract everything (Ctrl+E with nothing selected)", "Extract the selection (Ctrl+E)", "View the file (F3)", "Open with its program", "Delete from the archive (Del)", "Test the whole archive"][k];
            if tool_button(ui, &p, r, *icn, Some(label), *en && !busy, *primary, id).on_hover_text(tip(hint)).clicked() {
                acts.push(match k {
                    0 => Act::Extract(None),
                    1 => Act::Extract(Some(sel.clone())),
                    2 => Act::View(one_file.unwrap()),
                    3 => Act::Open(one_file.unwrap()),
                    4 => Act::Delete(sel.clone()),
                    _ => Act::Test,
                });
            }
            x = r.right() + 8.0;
        }
        let det = Rect::from_min_size(Pos2::new(panel.right() - 16.0 - 36.0, y2), Vec2::splat(36.0));
        if tool_button(ui, &p, det, Ic::Details, None, true, b.show_details, "a-details").on_hover_text(tip("Show or hide the details")).clicked() {
            b.show_details = !b.show_details;
            write_setting("unnik_details", if b.show_details { "1" } else { "0" });
        }
        let openr = Rect::from_min_size(Pos2::new(det.left() - 8.0 - 36.0, y2), Vec2::splat(36.0));
        if tool_button(ui, &p, openr, Ic::Folder, None, !busy, false, "a-openarc").on_hover_text(tip("Open another archive")).clicked() {
            acts.push(Act::Pick);
        }
        let closer = Rect::from_min_size(Pos2::new(openr.left() - 8.0 - 36.0, y2), Vec2::splat(36.0));
        let destr = Rect::from_min_size(Pos2::new(closer.left() - 8.0 - 36.0, y2), Vec2::splat(36.0));
        let dresp = tool_button(ui, &p, destr, Ic::Reveal, None, true, false, "a-dest").on_hover_text(tip("Open the destination folder (last extraction, or the archive's folder)"));
        if dresp.clicked() {
            let last = read_setting("unnik_last_dest").map(PathBuf::from).filter(|d| d.is_dir());
            let dir = last.or_else(|| b.path.parent().map(|d| d.to_path_buf()).filter(|d| d.is_dir()));
            if let Some(d) = dir {
                sfx::play(Sfx::Pop);
                open_folder(&d);
            }
        }
        if tool_button(ui, &p, closer, Ic::Eject, None, !busy, false, "a-close").on_hover_text(tip("Close the archive")).clicked() {
            acts.push(Act::Close);
        }

        // ---- body ---------------------------------------------------------------------------------------------
        let body_top = y2 + 50.0;
        let body_bottom = panel.bottom() - FOOT_H;
        let tree_w = if panel.width() > 1000.0 { 250.0 } else { 220.0 };
        let tree_r = Rect::from_min_max(Pos2::new(panel.left() + 16.0, body_top), Pos2::new(panel.left() + 16.0 + tree_w, body_bottom));
        let det_w = 280.0;
        let show_det = b.show_details && panel.width() > 900.0;
        let list_right = if show_det { panel.right() - 16.0 - det_w - 12.0 } else { panel.right() - 16.0 };
        let list_r = Rect::from_min_max(Pos2::new(tree_r.right() + 12.0, body_top), Pos2::new(list_right, body_bottom));

        // ---- tree + summary --------------------------------------------------------------------------
        pane(&p, tree_r);
        small_label(&p, Pos2::new(tree_r.left() + 16.0, tree_r.top() + 14.0), t("FOLDERS"));
        let sum_h = 128.0;
        let tree_area = Rect::from_min_max(Pos2::new(tree_r.left() + 6.0, tree_r.top() + 34.0), Pos2::new(tree_r.right() - 4.0, tree_r.bottom() - sum_h - 6.0));
        {
            let mut vis: Vec<usize> = Vec::new();
            let mut stack = vec![0usize];
            while let Some(n) = stack.pop() {
                vis.push(n);
                if b.expanded.contains(&n) {
                    for &c in b.nodes[n].children.iter().rev() {
                        if b.nodes[c].is_dir {
                            stack.push(c);
                        }
                    }
                }
            }
            let content_h = vis.len() as f32 * TREE_ROW;
            scroller(ui, &p, tree_area, content_h, &mut b.tree_scroll, "tree-scroll", quiet);
            let tp = clipped(&p, tree_area);
            let first = (b.tree_scroll / TREE_ROW) as usize;
            let count = (tree_area.height() / TREE_ROW) as usize + 2;
            for (i, &n) in vis.iter().enumerate().skip(first).take(count) {
                let y = tree_area.top() + i as f32 * TREE_ROW - b.tree_scroll;
                let row = Rect::from_min_size(Pos2::new(tree_area.left(), y), Vec2::new(tree_area.width() - 10.0, TREE_ROW - 2.0));
                let depth = b.nodes[n].depth as f32;
                let resp = ui.interact(row.intersect(tree_area), egui::Id::new(("tree", n)), Sense::click());
                let current = n == b.cwd && b.search.trim().is_empty();
                if current || resp.hovered() {
                    tp.rect_filled(row, rr(8), alpha(ACCENT(), if current { 70 } else { 30 }));
                }
                let x = row.left() + 8.0 + depth * 14.0;
                let has_sub = b.nodes[n].children.iter().any(|&c| b.nodes[c].is_dir);
                let tri = Rect::from_center_size(Pos2::new(x + 6.0, row.center().y), Vec2::splat(16.0));
                if has_sub && n != 0 {
                    let open = b.expanded.contains(&n);
                    let c = tri.center();
                    let pts = if open {
                        vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(4.0, -2.0), c + Vec2::new(0.0, 3.0)]
                    } else {
                        vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(3.0, 0.0), c + Vec2::new(-2.0, 4.0)]
                    };
                    tp.add(Shape::convex_polygon(pts, TEXT_DIM(), Stroke::NONE));
                    let tr = ui.interact(tri.intersect(tree_area), egui::Id::new(("tree-tri", n)), Sense::click());
                    if tr.clicked() {
                        if !b.expanded.remove(&n) {
                            b.expanded.insert(n);
                        }
                    }
                }
                if n == 0 {
                    glyph(&tp, Pos2::new(x + 6.0, row.center().y), Ic::Archive, ACCENT_SOFT(), 15.0);
                } else {
                    file_icon(&tp, Rect::from_center_size(Pos2::new(x + 24.0, row.center().y), Vec2::splat(18.0)), true, "");
                }
                let label = if n == 0 { name.clone() } else { b.nodes[n].name.clone() };
                let tx = if n == 0 { x + 20.0 } else { x + 38.0 };
                let g = elide(&tp, &label, FontId::new(12.5, family(if current { "ui" } else { "ui-light" })), row.right() - tx - 6.0);
                g.paint(&tp, Pos2::new(tx, row.center().y - g.size().y / 2.0), if current { TEXT() } else { alpha(TEXT(), 215) });
                if resp.clicked() && !busy {
                    sfx::play(Sfx::Click);
                    acts.push(Act::Go(n));
                    if n == b.cwd {
                        b.search.clear();
                        b.dirty = true;
                    }
                    b.expanded.insert(n);
                }
                if resp.double_clicked() && n != 0 && !b.expanded.remove(&n) {
                    b.expanded.insert(n);
                }
            }
        }
        // compression summary
        let sum_r = Rect::from_min_max(Pos2::new(tree_r.left() + 10.0, tree_r.bottom() - sum_h), tree_r.max - Vec2::splat(10.0));
        p.rect_filled(sum_r, rr(12), alpha(ACCENT(), 16));
        p.rect_stroke(sum_r, rr(12), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 40)), egui::StrokeKind::Inside);
        let frac = if orig > 0 { (packed as f64 / orig as f64) as f32 } else { 1.0 };
        let dc = Pos2::new(sum_r.left() + 46.0, sum_r.top() + 50.0);
        donut(&p, dc, 30.0, frac.min(1.0));
        text(&p, dc, Align2::CENTER_CENTER, format!("{:.0}%", (frac * 100.0).min(999.0)), FontId::new(15.0, family("display")), TEXT());
        let tx = sum_r.left() + 90.0;
        small_label(&p, Pos2::new(tx, sum_r.top() + 18.0), t("SPACE SAVED"));
        let saved = if orig > packed { orig - packed } else { 0 };
        text(&p, Pos2::new(tx, sum_r.top() + 44.0), Align2::LEFT_CENTER, human(saved), FontId::new(20.0, family("display")), OK_GREEN());
        text_fit(&p, Pos2::new(tx, sum_r.top() + 68.0), Align2::LEFT_CENTER, format!("{} → {}", human(orig), human(packed)), FontId::new(11.5, family("ui-light")), TEXT_DIM(), sum_r.right() - tx - 8.0);
        let meth = if b.index.encrypted { format!("{}  ·  XChaCha20", method_name(&b.index)) } else { method_name(&b.index) };
        if b.index.encrypted {
            glyph(&p, Pos2::new(sum_r.left() + 20.0, sum_r.bottom() - 22.0), Ic::Lock, OK_GREEN(), 11.0);
        }
        text_fit(&p, Pos2::new(sum_r.left() + if b.index.encrypted { 32.0 } else { 14.0 }, sum_r.bottom() - 22.0), Align2::LEFT_CENTER, meth, FontId::new(11.5, family("ui")), alpha(TEXT(), 200), sum_r.width() - 40.0);

        // ---- list ------------------------------------------------------------------------------------------
        pane(&p, list_r);
        let searching = !b.search.trim().is_empty();
        let narrow = list_r.width() < 620.0;
        let x_check = list_r.left() + 20.0;
        let x_icon = list_r.left() + 50.0;
        let x_name = list_r.left() + 70.0;
        let w_date = 140.0;
        let w_type = if narrow { 0.0 } else { 120.0 };
        let w_size = 96.0;
        let x_date = list_r.right() - 14.0 - w_date;
        let x_type = x_date - w_type;
        let x_size_r = x_type - 18.0;
        let name_w = x_size_r - w_size - x_name;
        let head = Rect::from_min_size(list_r.min, Vec2::new(list_r.width(), 36.0));
        p.line_segment([Pos2::new(list_r.left() + 10.0, head.bottom()), Pos2::new(list_r.right() - 10.0, head.bottom())], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 36)));
        // select-all box
        let all_on = !b.rows.is_empty() && b.selected.len() == b.rows.len();
        let ab = Rect::from_center_size(Pos2::new(x_check, head.center().y), Vec2::splat(22.0));
        let abr = ui.interact(ab, egui::Id::new("check-all"), Sense::click());
        checkbox(&p, ab.center(), all_on, abr.hovered());
        if abr.clicked() {
            if all_on {
                b.selected.clear();
            } else {
                acts.push(Act::SelectAll);
            }
        }
        let cols: Vec<(Col, &str, f32, f32, bool)> = {
            let mut v = vec![(Col::Name, t("FILE NAME"), x_icon - 8.0, x_icon - 8.0 + name_w, false), (Col::Size, t("SIZE"), x_size_r - w_size, x_size_r, true)];
            if !narrow {
                v.push((Col::Type, t("TYPE"), x_type, x_type + w_type - 10.0, false));
            }
            v.push((Col::Date, t("MODIFIED"), x_date, x_date + w_date, false));
            v
        };
        for (col, label, l, r, right) in cols {
            let hr = Rect::from_min_max(Pos2::new(l - 4.0, head.top() + 6.0), Pos2::new(r + 4.0, head.bottom() - 6.0));
            let resp = ui.interact(hr, egui::Id::new(("col", label)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            if resp.hovered() {
                p.rect_filled(hr, rr(6), alpha(ACCENT(), 26));
            }
            let active = b.sort.0 == col;
            let g = spaced(&p, label, FontId::new(9.5, family("ui")), TEXT_DIM(), 1.4);
            let gx = if right { r - g.size().x - if active { 14.0 } else { 0.0 } } else { l + 4.0 };
            let at = label_pos(&g, Pos2::new(gx, head.center().y - 6.0));
            g.paint(&p, at, if active { TEXT() } else { TEXT_DIM() });
            if active {
                let ax = if right { r - 5.0 } else { gx + g.size().x + 8.0 };
                let c = Pos2::new(ax, head.center().y);
                let pts = if b.sort.1 {
                    vec![c + Vec2::new(-3.5, 2.0), c + Vec2::new(3.5, 2.0), c + Vec2::new(0.0, -2.5)]
                } else {
                    vec![c + Vec2::new(-3.5, -2.0), c + Vec2::new(3.5, -2.0), c + Vec2::new(0.0, 2.5)]
                };
                p.add(Shape::convex_polygon(pts, ACCENT_SOFT(), Stroke::NONE));
            }
            if resp.clicked() {
                b.sort = if b.sort.0 == col { (col, !b.sort.1) } else { (col, col != Col::Date && col != Col::Size) };
                b.dirty = true;
            }
        }
        b.refresh_rows();
        let area = Rect::from_min_max(Pos2::new(list_r.left() + 6.0, head.bottom() + 4.0), Pos2::new(list_r.right() - 4.0, list_r.bottom() - 6.0));
        let content_h = b.rows.len() as f32 * ROW_H;
        if b.scroll_to_focus {
            b.scroll_to_focus = false;
            if let Some(i) = b.focus.and_then(|f| b.rows.iter().position(|&x| x == f)) {
                let top = i as f32 * ROW_H;
                if top < b.list_scroll {
                    b.list_scroll = top;
                } else if top + ROW_H > b.list_scroll + area.height() {
                    b.list_scroll = top + ROW_H - area.height();
                }
            }
        }
        scroller(ui, &p, area, content_h, &mut b.list_scroll, "list-scroll", quiet);
        let lp = clipped(&p, area);
        // clicks on the empty part of the list clear the selection
        let bg = ui.interact(area, egui::Id::new("list-focus"), Sense::click());
        if bg.clicked() {
            b.selected.clear();
        }
        let (cmd, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
        let first = (b.list_scroll / ROW_H) as usize;
        let count = (area.height() / ROW_H) as usize + 2;
        let rows: Vec<usize> = b.rows.iter().copied().skip(first).take(count).collect();
        for (k, n) in rows.into_iter().enumerate() {
            let i = first + k;
            let y = area.top() + i as f32 * ROW_H - b.list_scroll;
            let row = Rect::from_min_size(Pos2::new(area.left(), y), Vec2::new(area.width() - 12.0, ROW_H - 2.0));
            let resp = ui.interact(row.intersect(area), egui::Id::new(("row", n)), Sense::click());
            let on = b.selected.contains(&n);
            if on {
                lp.rect_filled(row, rr(9), alpha(ACCENT(), 64));
                lp.rect_stroke(row, rr(9), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 70)), egui::StrokeKind::Inside);
            } else if resp.hovered() {
                lp.rect_filled(row, rr(9), alpha(ACCENT(), 26));
            } else if i % 2 == 1 {
                lp.rect_filled(row, rr(9), hi(4));
            }
            if b.focus == Some(n) && !on {
                lp.rect_stroke(row, rr(9), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 50)), egui::StrokeKind::Inside);
            }
            let node = &b.nodes[n];
            let cb = Rect::from_center_size(Pos2::new(x_check, row.center().y), Vec2::splat(24.0));
            let cbr = ui.interact(cb.intersect(area), egui::Id::new(("check", n)), Sense::click());
            checkbox(&lp, cb.center(), on, cbr.hovered() || resp.hovered());
            let ext = ext_of(&node.name);
            file_icon(&lp, Rect::from_center_size(Pos2::new(x_icon + 4.0, row.center().y), Vec2::splat(24.0)), node.is_dir, &ext);
            let np = clipped(&lp, Rect::from_min_max(Pos2::new(x_name, row.top()), Pos2::new(x_name + name_w, row.bottom())));
            let g = elide(&np, &node.name, FontId::new(13.5, family(if node.is_dir { "ui" } else { "ui-light" })), name_w - 8.0);
            g.paint(&np, Pos2::new(x_name, row.center().y - g.size().y / 2.0), TEXT());
            if searching {
                let folder = match node.path.rfind('/') {
                    Some(q) => node.path[..q].to_string(),
                    None => format!("({})", t("top level")),
                };
                let fx = x_name + g.size().x + 10.0;
                let fg = elide(&np, &folder, FontId::new(11.5, family("ui-light")), x_name + name_w - fx - 8.0);
                fg.paint(&np, Pos2::new(fx, row.center().y - fg.size().y / 2.0 + 1.0), TEXT_DIM());
            }
            let sz = if node.is_dir { human(node.size) } else { human(node.size) };
            text(&lp, Pos2::new(x_size_r, row.center().y), Align2::RIGHT_CENTER, sz, FontId::new(12.5, family("ui-light")), if node.is_dir { TEXT_DIM() } else { alpha(TEXT(), 220) });
            if !narrow {
                let tg = elide(&lp, &type_name(node), FontId::new(12.0, family("ui-light")), w_type - 14.0);
                tg.paint(&lp, Pos2::new(x_type, row.center().y - tg.size().y / 2.0), TEXT_DIM());
            }
            text(&lp, Pos2::new(x_date, row.center().y), Align2::LEFT_CENTER, date(node.mtime, b.tz), FontId::new(12.0, family("ui-light")), TEXT_DIM());

            if cbr.clicked() {
                if !b.selected.remove(&n) {
                    b.selected.insert(n);
                }
                b.anchor = Some(n);
                b.focus = Some(n);
            } else if resp.double_clicked() && b.last_click == Some(n) {
                acts.push(Act::Activate(n));
            } else if resp.clicked() {
                b.last_click = Some(n);
                b.click(n, cmd, shift);
                ctx.memory_mut(|m| m.surrender_focus(egui::Id::new("search")));
            } else if resp.secondary_clicked() {
                if !b.selected.contains(&n) {
                    b.click(n, false, false);
                }
                if let Some(pos) = resp.interact_pointer_pos() {
                    acts.push(Act::Menu(pos, n));
                }
            }
        }
        if b.rows.is_empty() {
            let msg = if searching { tf("Nothing matches “{}”.", &[b.search.trim()]) } else { t("This folder is empty.").to_string() };
            text(&p, area.center(), Align2::CENTER_CENTER, msg, FontId::new(14.0, family("ui-light")), TEXT_DIM());
        }

        // ---- details ----------------------------------------------------------------------------------------
        if show_det {
            let dr = Rect::from_min_max(Pos2::new(list_right + 12.0, body_top), Pos2::new(panel.right() - 16.0, body_bottom));
            pane(&p, dr);
            let sel = b.selection();
            let cx = dr.center().x;
            let inner_w = dr.width() - 40.0;
            let mut y = dr.top() + 22.0;
            small_label(&p, Pos2::new(dr.left() + 20.0, y - 8.0), t("DETAILS"));
            y += 22.0;
            let target: Option<usize> = match sel.len() {
                0 => Some(b.cwd),
                1 => Some(sel[0]),
                _ => None,
            };
            match target {
                Some(n) => {
                    let node = &b.nodes[n];
                    let ext = ext_of(&node.name);
                    if n == 0 {
                        radial_glow(&p, Pos2::new(cx, y + 34.0), 70.0, alpha(ACCENT(), 40));
                        glyph(&p, Pos2::new(cx, y + 34.0), Ic::Archive, ACCENT_SOFT(), 48.0);
                    } else {
                        radial_glow(&p, Pos2::new(cx, y + 34.0), 70.0, alpha(if node.is_dir { FOLDER() } else { kind_color(kind_of(&ext)) }, 34));
                        file_icon(&p, Rect::from_center_size(Pos2::new(cx, y + 34.0), Vec2::splat(64.0)), node.is_dir, &ext);
                    }
                    y += 82.0;
                    let title = if n == 0 { name.clone() } else { node.name.clone() };
                    y += wrapped(&p, Pos2::new(dr.left() + 20.0, y), inner_w, &title, 14.5, TEXT()) + 4.0;
                    let typ = if n == 0 { t("NikPak archive").to_string() } else { type_name(node) };
                    text_fit(&p, Pos2::new(cx, y + 6.0), Align2::CENTER_CENTER, typ, FontId::new(12.0, family("ui-light")), TEXT_DIM(), inner_w);
                    y += 26.0;
                    let mut props: Vec<(&str, String)> = Vec::new();
                    if node.is_dir {
                        props.push((t("CONTAINS"), format!("{}, {}", files_str(node.files as u64), folders_str(node.dirs as u64))));
                        props.push((t("SIZE"), human(node.size)));
                    } else {
                        props.push((t("SIZE"), format!("{}  ({} B)", human(node.size), node.size)));
                    }
                    if node.mtime > 0 {
                        props.push((t("MODIFIED"), date(node.mtime, b.tz)));
                    }
                    if n != 0 {
                        let folder = match node.path.rfind('/') {
                            Some(q) => node.path[..q].to_string(),
                            None => format!("({})", t("top level")),
                        };
                        props.push((t("FOLDER()"), folder));
                    }
                    if let Some(e) = b.entry(n).filter(|e| !e.is_dir) {
                        props.push(("CRC32", format!("{:08X}", e.crc)));
                        let stored = if e.filter.is_empty() { method_name(&b.index) } else { format!("{} + {}", e.filter, method_name(&b.index)) };
                        props.push((t("STORED WITH"), stored));
                        props.push((t("ATTRIBUTES"), if e.mode & 0o200 == 0 { t("Read-only").to_string() } else { t("Normal").to_string() }));
                    }
                    if n == 0 {
                        props.push((t("ARCHIVE SIZE"), human(packed)));
                        props.push((t("METHOD"), method_name(&b.index)));
                        props.push((t("ENCRYPTION"), if b.index.encrypted { "Argon2id + XChaCha20-Poly1305".into() } else { t("None").to_string() }));
                    }
                    for (label, value) in props {
                        if y > dr.bottom() - 130.0 {
                            break;
                        }
                        small_label(&p, Pos2::new(dr.left() + 20.0, y), label);
                        y += 16.0;
                        let g = elide(&p, &value, FontId::new(12.5, family("ui-light")), inner_w);
                        g.paint(&p, Pos2::new(dr.left() + 20.0, y), alpha(TEXT(), 230));
                        y += g.size().y + 10.0;
                    }
                    if n != 0 || sel.len() == 1 {
                        let bw = (inner_w - 8.0) / 2.0;
                        let by = dr.bottom() - 98.0;
                        let r1 = Rect::from_min_size(Pos2::new(dr.left() + 20.0, by), Vec2::new(bw, 36.0));
                        let r2 = r1.translate(Vec2::new(bw + 8.0, 0.0));
                        let r3 = r1.translate(Vec2::new(0.0, 44.0));
                        let r4 = r2.translate(Vec2::new(0.0, 44.0));
                        if node.is_dir {
                            if tool_button(ui, &p, r1.union(r2), Ic::Folder, Some(t("Open folder")), !busy, false, "d-enter").clicked() {
                                acts.push(Act::Go(n));
                            }
                        } else {
                            if tool_button(ui, &p, r1, Ic::View, Some(t("View")), !busy, false, "d-view").clicked() {
                                acts.push(Act::View(n));
                            }
                            if tool_button(ui, &p, r2, Ic::Open, Some(t("Open")), !busy, false, "d-open").clicked() {
                                acts.push(Act::Open(n));
                            }
                        }
                        if tool_button(ui, &p, r3, Ic::Extract, Some(t("Extract…")), !busy, true, "d-extract").clicked() {
                            acts.push(Act::Extract(Some(vec![n])));
                        }
                        if tool_button(ui, &p, r4, Ic::Delete, Some(t("Delete")), !busy, false, "d-delete").clicked() {
                            acts.push(Act::Delete(vec![n]));
                        }
                    }
                }
                None => {
                    let (size, files) = b.sum(&sel);
                    let c = Pos2::new(cx, y + 34.0);
                    radial_glow(&p, c, 70.0, alpha(ACCENT(), 40));
                    for k in 0..3 {
                        let off = Vec2::new(-10.0 + k as f32 * 10.0, -6.0 + k as f32 * 6.0);
                        file_icon(&p, Rect::from_center_size(c + off, Vec2::splat(50.0)), false, "");
                    }
                    y += 82.0;
                    text(&p, Pos2::new(cx, y + 8.0), Align2::CENTER_CENTER, tf("{} items selected", &[&sel.len().to_string()]), FontId::new(15.0, family("ui")), TEXT());
                    y += 34.0;
                    for (label, value) in [(t("FILES"), files.to_string()), (t("SIZE"), human(size))] {
                        small_label(&p, Pos2::new(dr.left() + 20.0, y), label);
                        y += 16.0;
                        text(&p, Pos2::new(dr.left() + 20.0, y), Align2::LEFT_TOP, value, FontId::new(12.5, family("ui-light")), alpha(TEXT(), 230));
                        y += 28.0;
                    }
                    let by = dr.bottom() - 98.0;
                    let r1 = Rect::from_min_size(Pos2::new(dr.left() + 20.0, by), Vec2::new(inner_w, 36.0));
                    if tool_button(ui, &p, r1, Ic::Extract, Some(&tf("Extract {}", &[&sel.len().to_string()])), !busy, true, "d-extract-n").clicked() {
                        acts.push(Act::Extract(Some(sel.clone())));
                    }
                    if tool_button(ui, &p, r1.translate(Vec2::new(0.0, 44.0)), Ic::Delete, Some(t("Delete")), !busy, false, "d-delete-n").clicked() {
                        acts.push(Act::Delete(sel.clone()));
                    }
                }
            }
        }

        // ---- footer ------------------------------------------------------------------------------------------------
        let fy = panel.bottom() - FOOT_H / 2.0;
        let lb = Rect::from_min_size(Pos2::new(panel.left() + 16.0, panel.bottom() - FOOT_H + 10.0), Vec2::new(170.0, 30.0));
        if lang_button(ui, &p, lb, self.lang_open, busy) {
            self.lang_open = !self.lang_open;
        }
        let (ssize, _) = b.sum(&b.selection());
        let mut status = items_str(b.rows.len());
        if !b.selected.is_empty() {
            status.push_str(&format!("  ·  {}  ·  {}", tf("{} selected", &[&b.selected.len().to_string()]), human(ssize)));
        }
        text_fit(&p, Pos2::new(lb.right() + 18.0, fy), Align2::LEFT_CENTER, status, FontId::new(12.0, family("ui-light")), TEXT_DIM(), panel.width() - 520.0);
        let hint = t("Double-click to open · Right-click for more · Del deletes");
        text_fit(&p, Pos2::new(panel.right() - 150.0, fy), Align2::RIGHT_CENTER, hint, FontId::new(11.0, family("ui-light")), alpha(TEXT_DIM(), 150), (panel.width() - 760.0).max(0.0));
        social_buttons(ui, &p, [0, 1, 2].map(|k| Pos2::new(panel.right() - 30.0 - (2 - k) as f32 * 40.0, fy)), 14.0);

        // ---- resize from the edges ---------------------------------------------------------------------------------
        if !maximized {
            use egui::ResizeDirection as D;
            let e = 6.0;
            let zones = [
                (Rect::from_min_max(Pos2::new(panel.left() + 16.0, panel.top()), Pos2::new(panel.right() - 16.0, panel.top() + e)), D::North, egui::CursorIcon::ResizeVertical),
                (Rect::from_min_max(Pos2::new(panel.left() + 16.0, panel.bottom() - e), Pos2::new(panel.right() - 16.0, panel.bottom())), D::South, egui::CursorIcon::ResizeVertical),
                (Rect::from_min_max(Pos2::new(panel.left(), panel.top() + 16.0), Pos2::new(panel.left() + e, panel.bottom() - 16.0)), D::West, egui::CursorIcon::ResizeHorizontal),
                (Rect::from_min_max(Pos2::new(panel.right() - e, panel.top() + 16.0), Pos2::new(panel.right(), panel.bottom() - 16.0)), D::East, egui::CursorIcon::ResizeHorizontal),
                (Rect::from_min_size(panel.left_top(), Vec2::splat(16.0)), D::NorthWest, egui::CursorIcon::ResizeNwSe),
                (Rect::from_min_size(panel.right_bottom() - Vec2::splat(16.0), Vec2::splat(16.0)), D::SouthEast, egui::CursorIcon::ResizeNwSe),
                (Rect::from_min_size(Pos2::new(panel.right() - 16.0, panel.top()), Vec2::splat(16.0)), D::NorthEast, egui::CursorIcon::ResizeNeSw),
                (Rect::from_min_size(Pos2::new(panel.left(), panel.bottom() - 16.0), Vec2::splat(16.0)), D::SouthWest, egui::CursorIcon::ResizeNeSw),
            ];
            for (k, (r, dir, cur)) in zones.into_iter().enumerate() {
                let resp = ui.interact(r, egui::Id::new(("resize", k)), Sense::drag()).on_hover_cursor(cur);
                if resp.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            }
        }
        if about && quiet {
            self.overlay = Overlay::About;
            sfx::play(Sfx::Pop);
            about = false;
        }
        let _ = about;
    }
}

// ---------------------------------------------------------------------------
// Context menu, boxes, progress, notices
// ---------------------------------------------------------------------------

/// Dim the window and return a painter in the foreground.
fn veil(ui: &mut egui::Ui, panel: Rect, id: &str) -> egui::Painter {
    ui.set_min_size(panel.size());
    let p = ui.painter().clone();
    let _ = ui.interact(panel, egui::Id::new(id), Sense::click_and_drag());
    p.rect_filled(panel, rr(RADIUS as u8), alpha(crate::skin::c().overlay, 190));
    p
}

fn radio(ui: &mut egui::Ui, p: &egui::Painter, r: Rect, on: bool, label: &str, id: &str) -> bool {
    let resp = ui.interact(r, egui::Id::new(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    p.rect_filled(r, rr(10), alpha(ACCENT(), if on { 90 } else if resp.hovered() { 40 } else { 14 }));
    p.rect_stroke(r, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if on { 200 } else { 60 })), egui::StrokeKind::Inside);
    text_fit(p, r.center(), Align2::CENTER_CENTER, label, FontId::new(12.5, family("ui")), if on { hi(255) } else { alpha(TEXT(), 210) }, r.width() - 10.0);
    if resp.clicked() {
        sfx::play(Sfx::Click);
    }
    resp.clicked()
}

fn check_row(ui: &mut egui::Ui, p: &egui::Painter, pos: Pos2, w: f32, on: &mut bool, label: &str, id: &str) {
    let r = Rect::from_min_size(pos, Vec2::new(w, 24.0));
    let resp = ui.interact(r, egui::Id::new(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    checkbox(p, Pos2::new(r.left() + 9.0, r.center().y), *on, resp.hovered());
    text_fit(p, Pos2::new(r.left() + 26.0, r.center().y), Align2::LEFT_CENTER, label, FontId::new(13.0, family("ui-light")), TEXT(), w - 30.0);
    if resp.clicked() {
        sfx::play(Sfx::Click);
        *on = !*on;
    }
}

impl App {
    fn draw_menu(&mut self, ctx: &egui::Context, panel: Rect, acts: &mut Vec<Act>) {
        let Some((pos, n)) = self.menu else { return };
        let Some(b) = &self.browser else {
            self.menu = None;
            return;
        };
        let sel = b.selection();
        let is_dir = b.nodes[n].is_dir;
        let multi = sel.len() > 1;
        let mut items: Vec<(Ic, String, Act)> = Vec::new();
        if multi {
            items.push((Ic::Extract, tf("Extract {}", &[&sel.len().to_string()]), Act::Extract(Some(sel.clone()))));
            items.push((Ic::Delete, t("Delete").into(), Act::Delete(sel.clone())));
        } else if is_dir {
            items.push((Ic::Folder, t("Open folder").into(), Act::Go(n)));
            items.push((Ic::Extract, t("Extract…").into(), Act::Extract(Some(vec![n]))));
            items.push((Ic::Delete, t("Delete").into(), Act::Delete(vec![n])));
            items.push((Ic::Copy, t("Copy the path").into(), Act::CopyName(n)));
        } else {
            items.push((Ic::View, t("View").into(), Act::View(n)));
            items.push((Ic::Open, t("Open").into(), Act::Open(n)));
            items.push((Ic::Notepad, t("Open in Notepad").into(), Act::Notepad(n)));
            items.push((Ic::Extract, t("Extract…").into(), Act::Extract(Some(vec![n]))));
            items.push((Ic::Delete, t("Delete").into(), Act::Delete(vec![n])));
            items.push((Ic::Copy, t("Copy the path").into(), Act::CopyName(n)));
        }
        let row_h = 32.0;
        let w = 230.0;
        let h = items.len() as f32 * row_h + 12.0;
        let mut origin = pos;
        if origin.x + w > panel.right() - 8.0 {
            origin.x -= w;
        }
        if origin.y + h > panel.bottom() - 8.0 {
            origin.y -= h;
        }
        let menu = Rect::from_min_size(origin, Vec2::new(w, h));
        let mut close = false;
        egui::Area::new(egui::Id::new("ctxmenu")).order(egui::Order::Foreground).fixed_pos(panel.min).show(ctx, |ui| {
            ui.set_min_size(panel.size());
            let p = ui.painter().clone();
            let outside = ui.interact(panel, egui::Id::new("menu-blocker"), Sense::click());
            if outside.clicked() || outside.secondary_clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            radial_glow(&p, menu.center(), menu.width(), alpha(Color32::BLACK, 90));
            gradient_panel(&p, menu, 12.0, MENU_TOP(), crate::skin::c().dialog_bottom);
            p.rect_stroke(menu, rr(12), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 110)), egui::StrokeKind::Inside);
            for (k, (icn, label, act)) in items.iter().enumerate() {
                let r = Rect::from_min_size(Pos2::new(menu.left() + 6.0, menu.top() + 6.0 + k as f32 * row_h), Vec2::new(w - 12.0, row_h));
                let resp = ui.interact(r, egui::Id::new(("menu", k)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let danger = matches!(act, Act::Delete(_));
                if resp.hovered() {
                    p.rect_filled(r, rr(8), if danger { alpha(ERR_RED(), 50) } else { alpha(ACCENT(), 70) });
                }
                let col = if danger { DANGER_SOFT() } else { TEXT() };
                glyph(&p, Pos2::new(r.left() + 18.0, r.center().y), *icn, col, 15.0);
                text_fit(&p, Pos2::new(r.left() + 38.0, r.center().y), Align2::LEFT_CENTER, label, FontId::new(13.0, family("ui-light")), col, w - 60.0);
                if resp.clicked() {
                    sfx::play(Sfx::Click);
                    acts.push(act.clone());
                    close = true;
                }
            }
        });
        if close {
            self.menu = None;
        }
    }

    fn draw_overlay(&mut self, ctx: &egui::Context, panel: Rect, parent: &eframe::Frame, acts: &mut Vec<Act>) {
        if matches!(self.overlay, Overlay::None) || panel == Rect::NOTHING {
            return;
        }
        let busy = self.job.is_some();
        let mut next: Option<Overlay> = None;
        let mut run_extract: Option<ExtractDlg> = None;
        let mut run_delete: Option<Vec<usize>> = None;
        let mut open_with: Option<(PathBuf, String)> = None;
        let pw_focus = std::mem::take(&mut self.pw_focus);
        let find_focus = std::mem::take(&mut self.viewer_find_focus);
        egui::Area::new(egui::Id::new("overlay")).order(egui::Order::Foreground).fixed_pos(panel.min).show(ctx, |ui| {
            let p = veil(ui, panel, "overlay-blocker");
            let cx = panel.center().x;
            match &mut self.overlay {
                Overlay::None => {}
                Overlay::About => {
                    let c = Rect::from_center_size(panel.center(), Vec2::new(372.0, 390.0));
                    card(&p, c);
                    glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("ABOUT"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                    glow_text(&p, Pos2::new(cx, c.top() + 76.0), "unNik", FontId::new(32.0, family("display")), TEXT(), alpha(ACCENT(), 8), 1.5);
                    text(&p, Pos2::new(cx, c.top() + 110.0), Align2::CENTER_CENTER, tf("Version {}", &[crate::cli::VERSION]), FontId::new(13.0, family("ui-light")), TEXT_DIM());
                    text_fit(&p, Pos2::new(cx, c.top() + 132.0), Align2::CENTER_CENTER, t("The free browser for .nik archives"), FontId::new(13.0, family("ui-light")), TEXT(), c.width() - 40.0);
                    p.line_segment([Pos2::new(cx - 60.0, c.top() + 152.0), Pos2::new(cx + 60.0, c.top() + 152.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 60)));
                    text(&p, Pos2::new(cx, c.top() + 176.0), Align2::CENTER_CENTER, COPYRIGHT, FontId::new(15.0, family("ui")), TEXT());
                    text(&p, Pos2::new(cx, c.top() + 200.0), Align2::CENTER_CENTER, t(DISTRIBUTOR), FontId::new(13.0, family("ui-light")), TEXT_DIM());
                    let sh = wrapped(&p, Pos2::new(c.left() + 30.0, c.top() + 226.0), c.width() - 60.0, t("unNik is free for everyone. No registration, no time limit."), 13.5, OK_GREEN());
                    let link_c = Pos2::new(cx, c.top() + 244.0 + sh);
                    let lr = ui.interact(Rect::from_center_size(link_c, Vec2::new(150.0, 22.0)), egui::Id::new("about-site"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let lt = text(&p, link_c, Align2::CENTER_CENTER, "nikpak.nikkoid.com", FontId::new(13.0, family("ui")), if lr.hovered() { TEXT() } else { ACCENT_SOFT() });
                    p.line_segment([Pos2::new(lt.left(), lt.bottom() + 1.0), Pos2::new(lt.right(), lt.bottom() + 1.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 120)));
                    if lr.clicked() {
                        open_link("https://nikpak.nikkoid.com");
                    }
                    let terms = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(150.0, 44.0));
                    let ok = terms.translate(Vec2::new(162.0, 0.0));
                    if pill_button(ui, &p, terms, t("Licence terms"), false, "about-terms") {
                        let lic = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("LICENSE.txt")));
                        match lic {
                            Some(f) if f.is_file() => open_link(&f.display().to_string()),
                            _ => open_link("https://nikpak.nikkoid.com"),
                        }
                    }
                    if pill_button(ui, &p, ok, t("Close"), true, "about-close") || card_close(ui, &p, c, "about-x") {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::Message { title, body, ok } => {
                    let tw = 340.0;
                    let bh = crate::gui::wrapped_height(&p, tw, body, 13.0).min(panel.height() - 300.0);
                    let c = Rect::from_center_size(panel.center(), Vec2::new(400.0, 230.0 + bh));
                    card(&p, c);
                    let ic = Pos2::new(cx, c.top() + 52.0);
                    let col = if *ok { OK_GREEN() } else { ERR_RED() };
                    p.circle_stroke(ic, 24.0, Stroke::new(2.5_f32, col));
                    if *ok {
                        p.add(PathShape::line(vec![ic + Vec2::new(-10.0, 0.0), ic + Vec2::new(-3.0, 7.0), ic + Vec2::new(11.0, -8.0)], Stroke::new(2.5_f32, col)));
                    } else {
                        p.line_segment([ic + Vec2::new(0.0, -11.0), ic + Vec2::new(0.0, 4.0)], Stroke::new(2.6_f32, col));
                        p.circle_filled(ic + Vec2::new(0.0, 10.0), 1.8, col);
                    }
                    text_fit(&p, Pos2::new(cx, c.top() + 104.0), Align2::CENTER_CENTER, title.as_str(), FontId::new(19.0, family("display")), TEXT(), c.width() - 40.0);
                    let bp = clipped(&p, Rect::from_min_max(Pos2::new(c.left(), c.top() + 124.0), Pos2::new(c.right(), c.bottom() - 76.0)));
                    wrapped(&bp, Pos2::new(c.left() + 30.0, c.top() + 128.0), tw, body, 13.0, TEXT_DIM());
                    let b = Rect::from_center_size(Pos2::new(cx, c.bottom() - 46.0), Vec2::new(160.0, 42.0));
                    if pill_button(ui, &p, b, "OK", true, "msg-ok") || ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape)) {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::NikPakMissing => {
                    let tw = 340.0;
                    let body = format!(
                        "{}\n{}",
                        t("NikPak is shareware. You may test it for free, without time limit, for personal use."),
                        t("Companies and organisations need one licence per seat.")
                    );
                    let bh = crate::gui::wrapped_height(&p, tw, &body, 13.5);
                    let c = Rect::from_center_size(panel.center(), Vec2::new(400.0, 250.0 + bh));
                    card(&p, c);
                    glow_text(&p, Pos2::new(cx, c.top() + 30.0), t("SHAREWARE"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                    text(&p, Pos2::new(cx, c.top() + 68.0), Align2::CENTER_CENTER, "NikPak", FontId::new(32.0, family("display")), TEXT());
                    let bp = clipped(&p, Rect::from_min_max(Pos2::new(c.left(), c.top() + 108.0), Pos2::new(c.right(), c.bottom() - 90.0)));
                    wrapped(&bp, Pos2::new(c.left() + 30.0, c.top() + 112.0), tw, &body, 13.5, TEXT_DIM());
                    let link_c = Pos2::new(cx, c.top() + bh + 146.0);
                    let lr = ui.interact(Rect::from_center_size(link_c, Vec2::new(150.0, 20.0)), egui::Id::new("nikpak-missing-site"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let lt = text(&p, link_c, Align2::CENTER_CENTER, "nikpak.nikkoid.com", FontId::new(13.0, family("ui")), if lr.hovered() { TEXT() } else { ACCENT_SOFT() });
                    p.line_segment([Pos2::new(lt.left(), lt.bottom() + 1.0), Pos2::new(lt.right(), lt.bottom() + 1.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 120)));
                    if lr.clicked() {
                        open_link("https://nikpak.nikkoid.com");
                    }
                    let close = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(160.0, 44.0));
                    let site = close.translate(Vec2::new(172.0, 0.0));
                    if pill_button(ui, &p, site, t("Go to the website"), true, "nikpak-missing-go") {
                        open_link("https://nikpak.nikkoid.com");
                        next = Some(Overlay::None);
                    }
                    if pill_button(ui, &p, close, t("Close"), false, "nikpak-missing-close") || card_close(ui, &p, c, "nikpak-missing-x") {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::Password { path, password, error } => {
                    let c = Rect::from_center_size(panel.center(), Vec2::new(392.0, 330.0));
                    card(&p, c);
                    glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("ENCRYPTED ARCHIVE"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                    glyph(&p, Pos2::new(cx, c.top() + 74.0), Ic::Lock, OK_GREEN(), 30.0);
                    text_fit(&p, Pos2::new(cx, c.top() + 112.0), Align2::CENTER_CENTER, t("Enter the password"), FontId::new(21.0, family("display")), TEXT(), c.width() - 40.0);
                    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    let ng = elide(&p, &name, FontId::new(13.0, family("ui-light")), c.width() - 60.0);
                    ng.paint(&p, Pos2::new(cx - ng.size().x / 2.0, c.top() + 134.0), TEXT_DIM());
                    let fr = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.top() + 166.0), Vec2::new(c.width() - 60.0, 38.0));
                    let resp = field(ui, &p, fr, password, "Password", "pw-field", true);
                    if pw_focus {
                        resp.request_focus();
                    }
                    if let Some(e) = error {
                        text_fit(&p, Pos2::new(cx, fr.bottom() + 18.0), Align2::CENTER_CENTER, e.as_str(), FontId::new(12.5, family("ui-light")), ERR_RED(), c.width() - 60.0);
                    }
                    let cancel = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(160.0, 44.0));
                    let okr = cancel.translate(Vec2::new(172.0, 0.0));
                    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if (pill_button(ui, &p, okr, t("Open"), true, "pw-ok") || enter) && !password.is_empty() {
                        open_with = Some((path.clone(), password.clone()));
                        next = Some(Overlay::None);
                    }
                    if pill_button(ui, &p, cancel, t("Cancel"), false, "pw-cancel") || card_close(ui, &p, c, "pw-x") {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::Delete(nodes) => {
                    let Some(b) = &self.browser else { return };
                    let (size, files) = b.sum(nodes);
                    let c = Rect::from_center_size(panel.center(), Vec2::new(410.0, 330.0));
                    card(&p, c);
                    glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("DELETE"), FontId::new(11.0, family("ui")), DANGER_SOFT(), alpha(ERR_RED(), 5), 3.0);
                    glyph(&p, Pos2::new(cx, c.top() + 76.0), Ic::Delete, ERR_RED(), 34.0);
                    let title = if nodes.len() == 1 { tf("Delete “{}”?", &[&b.nodes[nodes[0]].name]) } else { tf("Delete {} items?", &[&nodes.len().to_string()]) };
                    text_fit(&p, Pos2::new(cx, c.top() + 122.0), Align2::CENTER_CENTER, title, FontId::new(19.0, family("display")), TEXT(), c.width() - 40.0);
                    let body = tf("{} ({}) will be removed from the archive. The archive is written again without them: this cannot be undone.", &[&files_str(files as u64), &human(size)]);
                    wrapped(&p, Pos2::new(c.left() + 30.0, c.top() + 146.0), c.width() - 60.0, &body, 13.0, TEXT_DIM());
                    let cancel = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(165.0, 44.0));
                    let del = cancel.translate(Vec2::new(185.0, 0.0));
                    let resp = ui.interact(del, egui::Id::new("del-ok"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    p.rect_filled(del, rr(12), if resp.hovered() { DELETE_HOVER() } else { DELETE_BTN() });
                    text_fit(&p, del.center(), Align2::CENTER_CENTER, t("Delete"), FontId::new(14.0, family("ui")), crate::skin::on(DELETE_BTN()), del.width() - 16.0);
                    if resp.clicked() && !busy {
                        run_delete = Some(nodes.clone());
                        next = Some(Overlay::None);
                    }
                    if pill_button(ui, &p, cancel, t("Cancel"), false, "del-cancel") || card_close(ui, &p, c, "del-x") {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::Extract(d) => {
                    let Some(b) = &self.browser else { return };
                    let (size, files) = match &d.nodes {
                        None => (b.nodes[0].size, b.nodes[0].files),
                        Some(ns) => b.sum(ns),
                    };
                    let c = Rect::from_center_size(panel.center(), Vec2::new(470.0, 470.0));
                    card(&p, c);
                    glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("EXTRACT"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                    let title = match &d.nodes {
                        None => t("Extract the whole archive").to_string(),
                        Some(ns) if ns.len() == 1 => tf("Extract “{}”", &[&b.nodes[ns[0]].name]),
                        Some(ns) => tf("Extract {} items", &[&ns.len().to_string()]),
                    };
                    text_fit(&p, Pos2::new(cx, c.top() + 70.0), Align2::CENTER_CENTER, title, FontId::new(21.0, family("display")), TEXT(), c.width() - 50.0);
                    text(&p, Pos2::new(cx, c.top() + 98.0), Align2::CENTER_CENTER, format!("{}  ·  {}", files_str(files as u64), human(size)), FontId::new(12.5, family("ui-light")), TEXT_DIM());
                    let left = c.left() + 30.0;
                    let w = c.width() - 60.0;
                    let mut y = c.top() + 126.0;
                    small_label(&p, Pos2::new(left + 2.0, y), t("EXTRACT TO"));
                    y += 18.0;
                    let fr = Rect::from_min_size(Pos2::new(left, y), Vec2::new(w - 96.0, 36.0));
                    field(ui, &p, fr, &mut d.dest, "Destination folder", "x-dest", false);
                    let br = Rect::from_min_size(Pos2::new(fr.right() + 8.0, y), Vec2::new(88.0, 36.0));
                    if tool_button(ui, &p, br, Ic::Folder, Some(t("Browse")), true, false, "x-browse").clicked() {
                        let mut dlg = rfd::FileDialog::new().set_parent(parent).set_title(t("Where should the files be extracted?"));
                        let cur = PathBuf::from(d.dest.trim());
                        if let Some(existing) = cur.ancestors().find(|a| a.is_dir()) {
                            dlg = dlg.set_directory(existing);
                        }
                        if let Some(dir) = dlg.pick_folder() {
                            d.dest = dir.display().to_string();
                        }
                    }
                    y += 46.0;
                    if let Some(dir) = b.path.parent() {
                        let here = dir.display().to_string();
                        if crate::gui::text_link(ui, &p, Pos2::new(left + 2.0, y - 4.0), t("Next to the archive, without a new folder"), "x-here") {
                            d.dest = here;
                        }
                    }
                    y += 26.0;
                    check_row(ui, &p, Pos2::new(left, y), w, &mut d.keep_paths, t("Keep the folders"), "x-paths");
                    y += 40.0;
                    small_label(&p, Pos2::new(left + 2.0, y), t("IF A FILE ALREADY EXISTS"));
                    y += 18.0;
                    let rw = (w - 16.0) / 3.0;
                    for (k, (mode, label)) in [(Exists::Overwrite, t("Replace it")), (Exists::Rename, t("Keep both")), (Exists::Skip, t("Skip it"))].into_iter().enumerate() {
                        let r = Rect::from_min_size(Pos2::new(left + k as f32 * (rw + 8.0), y), Vec2::new(rw, 34.0));
                        if radio(ui, &p, r, d.exists == mode, label, &format!("x-exists-{k}")) {
                            d.exists = mode;
                        }
                    }
                    y += 50.0;
                    check_row(ui, &p, Pos2::new(left, y), w, &mut d.show, t("Show the files when done"), "x-show");
                    let cancel = Rect::from_min_size(Pos2::new(left, c.bottom() - 70.0), Vec2::new((w - 12.0) / 2.0, 44.0));
                    let go = cancel.translate(Vec2::new(cancel.width() + 12.0, 0.0));
                    if pill_button(ui, &p, go, t("Extract"), true, "x-go") && !busy && !d.dest.trim().is_empty() {
                        run_extract = Some(ExtractDlg { nodes: d.nodes.clone(), dest: d.dest.clone(), keep_paths: d.keep_paths, exists: d.exists, show: d.show });
                        next = Some(Overlay::None);
                    }
                    if pill_button(ui, &p, cancel, t("Cancel"), false, "x-cancel") || card_close(ui, &p, c, "x-x") {
                        next = Some(Overlay::None);
                    }
                }
                Overlay::Viewer(v) => {
                    if let Some(n) = draw_viewer(ui, ctx, &p, panel, v, find_focus, busy, acts) {
                        if n == usize::MAX {
                            next = Some(Overlay::None);
                        }
                    }
                }
            }
        });
        if let Some(o) = next {
            self.overlay = o;
        }
        if let Some(d) = run_extract {
            self.extract(d);
        }
        if let Some(ns) = run_delete {
            self.delete(ns);
        }
        if let Some((path, pw)) = open_with {
            self.open(path, Some(pw));
        }
    }

    fn draw_busy(&mut self, ctx: &egui::Context, panel: Rect) {
        let Some(job) = &self.job else { return };
        if !self.browsing() || panel == Rect::NOTHING {
            return; // the empty window shows its own ring
        }
        // short jobs (reading one file) only show a ring after a moment
        if job.work == Work::Read && job.started.elapsed() < Duration::from_millis(300) {
            return;
        }
        let total = PROGRESS.total.load(Relaxed).max(1);
        let done = PROGRESS.done.load(Relaxed).min(total);
        let target = (done as f32 / total as f32 * 99.0).min(99.0);
        self.shown += (target - self.shown) * 0.2;
        let shown = self.shown;
        let label = job.label.clone();
        let work = job.work;
        let secs = job.started.elapsed().as_secs_f32();
        egui::Area::new(egui::Id::new("busy")).order(egui::Order::Foreground).fixed_pos(panel.min).show(ctx, |ui| {
            let p = veil(ui, panel, "busy-blocker");
            let c = Rect::from_center_size(panel.center(), Vec2::new(360.0, 330.0));
            card(&p, c);
            let rc = Pos2::new(c.center().x, c.top() + 100.0);
            p.circle_stroke(rc, 56.0, Stroke::new(7.0_f32, TRACK()));
            progress_arc(&p, rc, 56.0, shown / 100.0, ACCENT());
            let num = p.layout_no_wrap(format!("{}", shown.round() as u32), FontId::new(44.0, family("display-light")), TEXT());
            let sign = p.layout_no_wrap("%".into(), FontId::new(16.0, family("display")), TEXT());
            let wsum = num.size().x + 3.0 + sign.size().x;
            let npos = Pos2::new(rc.x - wsum / 2.0, rc.y - num.size().y / 2.0 - 1.0);
            let spos = Pos2::new(npos.x + num.size().x + 3.0, npos.y + num.size().y * 0.80 - sign.size().y);
            p.galley(npos, num, TEXT());
            p.galley(spos, sign, TEXT());
            let verb = match work {
                Work::Open => t("OPENING"),
                Work::Extract => t("EXTRACTING"),
                Work::Delete => t("DELETING"),
                Work::Test => t("TESTING"),
                Work::Read => t("READING"),
            };
            glow_text(&p, Pos2::new(c.center().x, c.top() + 186.0), verb, FontId::new(12.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
            text_fit(&p, Pos2::new(c.center().x, c.top() + 208.0), Align2::CENTER_CENTER, &label, FontId::new(14.0, family("ui-light")), TEXT(), c.width() - 40.0);
            text(&p, Pos2::new(c.center().x, c.top() + 228.0), Align2::CENTER_CENTER, format!("{}  ·  {:.0} s", tf("{} of {}", &[&human(done), &human(total)]), secs), FontId::new(12.0, family("ui-light")), TEXT_DIM());
            let cancel = Rect::from_center_size(Pos2::new(c.center().x, c.bottom() - 44.0), Vec2::new(150.0, 38.0));
            if !PROGRESS.cancel.load(Relaxed) && (pill_button(ui, &p, cancel, t("Cancel"), false, "busy-cancel") || ui.input(|i| i.key_pressed(egui::Key::Escape))) {
                sfx::play(Sfx::Cancel);
                PROGRESS.cancel.store(true, Relaxed);
            }
        });
    }

    fn draw_toast(&mut self, ctx: &egui::Context, panel: Rect) {
        let Some(tst) = &self.toast else { return };
        let left = tst.until.saturating_duration_since(Instant::now()).as_secs_f32();
        if left <= 0.0 || panel == Rect::NOTHING {
            self.toast = None;
            return;
        }
        let a = (left / 0.4).min(1.0);
        let folder = tst.folder.clone();
        let msg = tst.text.clone();
        let mut dismiss = false;
        egui::Area::new(egui::Id::new("toast")).order(egui::Order::Tooltip).fixed_pos(panel.min).show(ctx, |ui| {
            let p = ui.painter().clone();
            let font = FontId::new(13.0, family("ui-light"));
            let link = folder.as_ref().map(|_| t("Show"));
            let max_w = panel.width() - 200.0;
            let g = elide(&p, &msg, font.clone(), max_w - 120.0);
            let lw = link.map(|l| lay(&p, l, crate::gui::link_font()).size().x + 24.0).unwrap_or(0.0);
            let w = g.size().x + 54.0 + lw;
            let y = if self.browser.is_some() { panel.bottom() - FOOT_H - 34.0 } else { panel.bottom() - 150.0 };
            let r = Rect::from_center_size(Pos2::new(panel.center().x, y), Vec2::new(w, 40.0));
            radial_glow(&p, r.center(), w * 0.7, alpha(Color32::BLACK, (100.0 * a) as u8));
            p.rect_filled(r, rr(20), alpha(crate::skin::c().dialog_top, (245.0 * a) as u8));
            p.rect_stroke(r, rr(20), Stroke::new(1.0_f32, alpha(OK_GREEN(), (160.0 * a) as u8)), egui::StrokeKind::Inside);
            let c = Pos2::new(r.left() + 22.0, r.center().y);
            p.add(PathShape::line(vec![c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.5, 3.5), c + Vec2::new(5.0, -4.0)], Stroke::new(2.0_f32, alpha(OK_GREEN(), (255.0 * a) as u8))));
            g.paint(&p, Pos2::new(r.left() + 38.0, r.center().y - g.size().y / 2.0), alpha(TEXT(), (255.0 * a) as u8));
            if let (Some(l), Some(f)) = (link, &folder) {
                let pos = Pos2::new(r.right() - lw + 4.0, r.center().y - 8.0);
                if crate::gui::text_link(ui, &p, pos, l, "toast-show") {
                    open_folder(f);
                    dismiss = true;
                }
            }
        });
        if dismiss {
            self.toast = None;
        }
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------------
// The viewer
// ---------------------------------------------------------------------------

/// Returns Some(usize::MAX) when the viewer is closed.
#[allow(clippy::too_many_arguments)]
fn draw_viewer(ui: &mut egui::Ui, ctx: &egui::Context, p: &egui::Painter, panel: Rect, v: &mut Viewer, find_focus: bool, busy: bool, acts: &mut Vec<Act>) -> Option<usize> {
    let mut result = None;
    let c = panel.shrink2(Vec2::new(26.0, 22.0));
    radial_glow(p, c.center(), c.width() * 0.6, alpha(ACCENT(), 26));
    gradient_panel(p, c, 18.0, crate::skin::c().dialog_top, crate::skin::c().dialog_bottom);
    p.rect_stroke(c, rr(18), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 90)), egui::StrokeKind::Inside);

    // ---- header: icon, name, size; buttons on the right
    let ext = ext_of(&v.name);
    file_icon(p, Rect::from_center_size(Pos2::new(c.left() + 36.0, c.top() + 34.0), Vec2::splat(34.0)), false, &ext);
    // close
    let xr = Rect::from_center_size(Pos2::new(c.right() - 30.0, c.top() + 34.0), Vec2::splat(28.0));
    let (cr, col) = icon_button(ui, xr, "view-close");
    p.line_segment([xr.center() + Vec2::new(-5.5, -5.5), xr.center() + Vec2::new(5.5, 5.5)], Stroke::new(1.5_f32, col));
    p.line_segment([xr.center() + Vec2::new(-5.5, 5.5), xr.center() + Vec2::new(5.5, -5.5)], Stroke::new(1.5_f32, col));
    if cr.clicked() {
        result = Some(usize::MAX);
    }
    let mut x = xr.left() - 10.0;
    let mut btn = |ic: Ic, label: Option<&str>, on: bool, enabled: bool, id: &str, hint: &'static str| -> bool {
        let w = match label {
            Some(l) => button_width(p, l),
            None => 36.0,
        };
        let r = Rect::from_min_size(Pos2::new(x - w, c.top() + 16.0), Vec2::new(w, 36.0));
        x -= w + 8.0;
        tool_button(ui, p, r, ic, label, enabled, on, id).on_hover_text(tip(hint)).clicked()
    };
    if btn(Ic::Notepad, Some(t("Open in Notepad")), false, !busy, "v-notepad", "Open the whole file in Notepad") {
        acts.push(Act::Notepad(v.node));
    }
    let has_text = v.text.is_some();
    if btn(Ic::Copy, None, false, has_text && !v.hex, "v-copy", "Copy all the text") {
        if let Some(tx) = &v.text {
            ctx.copy_text(tx.clone());
        }
    }
    if btn(Ic::Wrap, None, v.wrap, has_text && !v.hex, "v-wrap", "Wrap long lines") {
        v.wrap = !v.wrap;
        v.rows.clear();
    }
    if btn(Ic::Hex, None, v.hex, has_text, "v-hex", "Text or hexadecimal view") {
        v.hex = !v.hex;
        v.found_for = "\u{0}".into();
    }
    drop(btn);
    let name_right = x - 10.0;
    let ng = elide(p, &v.name, FontId::new(16.0, family("ui")), name_right - (c.left() + 64.0));
    ng.paint(p, Pos2::new(c.left() + 64.0, c.top() + 26.0 - ng.size().y / 2.0), TEXT());
    let mut sub = human(v.size);
    if v.more {
        sub.push_str(&format!("  ·  {}", tf("showing the first {}", &[&human(v.data.len() as u64)])));
    }
    if v.hex && v.text.is_none() {
        sub.push_str(&format!("  ·  {}", t("binary file")));
    }
    text_fit(p, Pos2::new(c.left() + 64.0, c.top() + 46.0), Align2::LEFT_CENTER, sub, FontId::new(11.5, family("ui-light")), TEXT_DIM(), name_right - c.left() - 64.0);

    // ---- find bar
    let fy = c.top() + 66.0;
    let fr = Rect::from_min_size(Pos2::new(c.left() + 20.0, fy), Vec2::new(300.0f32.min(c.width() * 0.4), 34.0));
    let resp = field(ui, p, fr, &mut v.find, "Find (Ctrl+F)", "v-find", false);
    glyph(p, Pos2::new(fr.right() - 18.0, fr.center().y), Ic::Search, TEXT_DIM(), 14.0);
    if find_focus {
        resp.request_focus();
    }
    v.search();
    let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let f3 = ui.input(|i| i.key_pressed(egui::Key::F3));
    let nfound = v.found.len();
    let prev_r = Rect::from_min_size(Pos2::new(fr.right() + 8.0, fy), Vec2::splat(34.0));
    let next_r = prev_r.translate(Vec2::new(40.0, 0.0));
    if tool_button(ui, p, prev_r, Ic::Prev, None, nfound > 0, false, "v-prev").clicked() && nfound > 0 {
        v.current = (v.current + nfound - 1) % nfound;
        v.jump = true;
    }
    if (tool_button(ui, p, next_r, Ic::Next, None, nfound > 0, false, "v-next").clicked() || enter || f3) && nfound > 0 {
        v.current = (v.current + 1) % nfound;
        v.jump = true;
        if enter {
            resp.request_focus();
        }
    }
    if !v.find.is_empty() {
        let s = if nfound == 0 { t("not found").to_string() } else { format!("{} / {}", v.current + 1, nfound) };
        text(p, Pos2::new(next_r.right() + 12.0, fy + 17.0), Align2::LEFT_CENTER, s, FontId::new(12.0, family("ui-light")), if nfound == 0 { ERR_RED() } else { TEXT_DIM() });
    }

    // ---- content
    let area = Rect::from_min_max(Pos2::new(c.left() + 16.0, fy + 46.0), Pos2::new(c.right() - 16.0, c.bottom() - 16.0));
    p.rect_filled(area, rr(12), alpha(crate::skin::c().field, 220));
    p.rect_stroke(area, rr(12), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 40)), egui::StrokeKind::Inside);
    let font = mono(13.0);
    let cw = lay(p, "0", font.clone()).size().x.max(1.0);
    let lh = 19.0;
    let inner = area.shrink2(Vec2::new(4.0, 8.0));
    let nrows = if v.hex { v.data.len().div_ceil(16) } else { v.rows.len() };
    let digits = if v.hex { 8 } else { (v.text.as_ref().map(|t| t.lines().count()).unwrap_or(1).max(1) as f32).log10() as usize + 1 };
    let gutter = digits as f32 * cw + 24.0;
    let text_w = inner.width() - gutter - 16.0;
    if !v.hex {
        v.layout(((text_w / cw).floor() as usize).max(20));
    }
    let nrows = if v.hex { nrows } else { v.rows.len() };
    if v.jump {
        v.jump = false;
        if let Some(&pos) = v.found.get(v.current) {
            let row = v.row_of(pos);
            v.scroll = (row as f32 * lh - inner.height() / 3.0).max(0.0);
        }
    }
    let mut hscroll = ui.data(|d| d.get_temp::<f32>(egui::Id::new("v-hscroll")).unwrap_or(0.0));
    if ui.rect_contains_pointer(inner) {
        let (dx, shift) = ui.input(|i| (i.smooth_scroll_delta.x, i.modifiers.shift));
        if !v.wrap && !v.hex {
            hscroll -= dx;
            if shift {
                hscroll -= ui.input(|i| i.smooth_scroll_delta.y);
            }
        }
    }
    if v.wrap || v.hex {
        hscroll = 0.0;
    }
    hscroll = hscroll.max(0.0);
    scroller(ui, p, inner, nrows as f32 * lh, &mut v.scroll, "v-scroll", !ui.input(|i| i.modifiers.shift));
    let tp = clipped(p, inner);
    let first = (v.scroll / lh) as usize;
    let count = (inner.height() / lh) as usize + 2;
    p.line_segment([Pos2::new(inner.left() + gutter - 10.0, inner.top()), Pos2::new(inner.left() + gutter - 10.0, inner.bottom())], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 30)));
    let cur_pos = v.found.get(v.current).copied();
    let q_len = v.find.len();
    let tx0 = inner.left() + gutter - hscroll;
    let textp = clipped(p, Rect::from_min_max(Pos2::new(inner.left() + gutter - 4.0, inner.top()), inner.max));
    let mut max_line_w = 0.0f32;
    for r in first..(first + count).min(nrows) {
        let y = inner.top() + r as f32 * lh - v.scroll;
        if v.hex {
            let off = r * 16;
            let chunk = &v.data[off..(off + 16).min(v.data.len())];
            text(&tp, Pos2::new(inner.left() + 8.0, y), Align2::LEFT_TOP, format!("{off:08X}"), font.clone(), alpha(TEXT_DIM(), 160));
            let mut hex = String::with_capacity(52);
            for (i, b) in chunk.iter().enumerate() {
                if i == 8 {
                    hex.push(' ');
                }
                hex.push_str(&format!("{b:02X} "));
            }
            text(&textp, Pos2::new(inner.left() + gutter, y), Align2::LEFT_TOP, &hex, font.clone(), TEXT());
            let ascii: String = chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '·' }).collect();
            text(&textp, Pos2::new(inner.left() + gutter + 50.0 * cw, y), Align2::LEFT_TOP, &ascii, font.clone(), ACCENT_SOFT());
            if let Some(pos) = cur_pos {
                if pos / 16 == r {
                    let col = (pos % 16) as f32;
                    let hx = inner.left() + gutter + (col * 3.0 + if col >= 8.0 { 1.0 } else { 0.0 }) * cw;
                    textp.rect_filled(Rect::from_min_size(Pos2::new(hx - 1.0, y), Vec2::new(2.0 * cw + 2.0, lh - 2.0)), rr(3), alpha(WARN(), 90));
                }
            }
            continue;
        }
        let Some(txt) = &v.text else { break };
        let (ln, s, e) = v.rows[r];
        let starts_line = r == 0 || v.rows[r - 1].0 != ln;
        if starts_line {
            text(&tp, Pos2::new(inner.left() + gutter - 16.0, y), Align2::RIGHT_TOP, format!("{}", ln + 1), font.clone(), alpha(TEXT_DIM(), 140));
        }
        let line = &txt[s..e];
        // highlights of the found text in this row
        if q_len > 0 {
            let lo = v.found.partition_point(|&m| m + q_len <= s);
            for &m in v.found[lo..].iter().take_while(|&&m| m < e) {
                let a = m.max(s);
                let b = (m + q_len).min(e);
                let xa = tx0 + txt[s..a].chars().count() as f32 * cw;
                let xb = tx0 + txt[s..b].chars().count() as f32 * cw;
                let strong = Some(m) == cur_pos;
                textp.rect_filled(Rect::from_min_max(Pos2::new(xa - 1.0, y), Pos2::new(xb + 1.0, y + lh - 2.0)), rr(3), alpha(WARN(), if strong { 150 } else { 55 }));
            }
        }
        if !line.is_empty() {
            let g = textp.layout_no_wrap(line.to_string(), font.clone(), TEXT());
            max_line_w = max_line_w.max(g.size().x);
            textp.galley(Pos2::new(tx0, y), g, alpha(TEXT(), 235));
        }
    }
    if !v.wrap && !v.hex {
        hscroll = hscroll.min((max_line_w - text_w + 40.0).max(0.0));
    }
    ui.data_mut(|d| d.insert_temp(egui::Id::new("v-hscroll"), hscroll));
    if nrows == 0 {
        text(p, area.center(), Align2::CENTER_CENTER, t("This file is empty."), FontId::new(14.0, family("ui-light")), TEXT_DIM());
    }
    result
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

/// Windows: if unNik was started as administrator (drag and drop blocked),
/// start it again as the normal user (see gui.rs, same method).
#[cfg(windows)]
fn relaunch_unelevated() -> bool {
    let marker = std::env::temp_dir().join("unnik-unelevated.marker");
    if !crate::gui::is_elevated() {
        let _ = std::fs::remove_file(&marker);
        return false;
    }
    if let Ok(age) = std::fs::metadata(&marker).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default()) {
        let _ = std::fs::remove_file(&marker);
        if age.as_secs() < 20 {
            return false;
        }
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    if std::fs::write(&marker, b"1").is_err() {
        return false;
    }
    std::process::Command::new("explorer.exe").arg(exe).spawn().is_ok()
}

/// Open the window; `archive` is opened at once (unNik archive.nik).
pub fn run(archive: Option<PathBuf>) -> Result<(), String> {
    #[cfg(windows)]
    if archive.is_none() && relaunch_unelevated() {
        return Ok(());
    }
    run_window(archive, None)
}

/// `unNik --extract / --extract-here / --extract-to <archives>` (right-click menu).
pub fn run_quick(mode: Quick, archives: Vec<PathBuf>) -> Result<(), String> {
    let verb = match mode {
        Quick::Dialog => "extract",
        Quick::Here => "extract-here",
        Quick::ToFolder => "extract-to",
    };
    let Some(mut archives) = crate::shell::collect(verb, archives) else {
        return Ok(()); // another unNik window takes care of this selection
    };
    archives.retain(|p| p.is_file());
    if archives.is_empty() {
        return Ok(());
    }
    if mode == Quick::Dialog {
        // one archive per window (the browser)
        for extra in archives.split_off(1) {
            if let Ok(exe) = std::env::current_exe() {
                let _ = std::process::Command::new(exe).arg("--extract").arg(extra).spawn();
            }
        }
    }
    run_window(None, Some((mode, archives)))
}

/// `unNik --add / --add-here <items>`: hand the items to nikpak, which is a console
/// program on Windows; started from here it opens no console window.
pub fn forward_to_nikpak(args: &[std::ffi::OsString]) -> Result<(), String> {
    let exe = crate::gui::sibling_exe("nikpak").ok_or_else(|| tf("Cannot find {} next to this program.", &["nikpak"]))?;
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn run_window(archive: Option<PathBuf>, quick: Option<(Quick, Vec<PathBuf>)>) -> Result<(), String> {
    i18n::set(crate::gui::load_language());
    crate::gui::start_sounds();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("unNik")
            .with_app_id("unNik")
            .with_inner_size(SMALL)
            .with_min_inner_size(SMALL)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true)
            .with_icon(Arc::new(icon())),
        centered: true,
        ..Default::default()
    };
    eframe::run_native("unNik", options, Box::new(move |cc| Ok(Box::new(App::new(cc, archive, quick))))).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_and_text() {
        let e = |name: &str, is_dir: bool, size: u64| Entry { name: name.into(), is_dir, size, mtime: 1, mode: 0o644, crc: 0, filter: String::new() };
        // "a/b/c.txt" without folder entries: the folders are made up
        let idx = Index { entries: vec![e("a/b/c.txt", false, 10), e("a", true, 0), e("z.bin", false, 5), e("a/d.txt", false, 1)], ..Default::default() };
        let nodes = build_tree(&idx);
        let root = &nodes[0];
        assert_eq!(root.files, 3);
        assert_eq!(root.size, 16);
        assert_eq!(root.dirs, 2);
        let names: Vec<&str> = root.children.iter().map(|&c| nodes[c].name.as_str()).collect();
        assert_eq!(names, ["a", "z.bin"]);
        let a = root.children[0];
        assert!(nodes[a].entry.is_some());
        assert_eq!(nodes[a].files, 2);

        assert!(looks_like_text(b"hello\r\nworld"));
        assert!(!looks_like_text(b"\x00\x01\x02"));
        assert_eq!(decode_text(b"\xEF\xBB\xBFh\xC3\xA9", false), "hé");
        assert_eq!(decode_text(&[0xFF, 0xFE, b'h', 0, b'i', 0], false), "hi");
        assert_eq!(decode_text(b"caf\xE9", false), "café");
        assert_eq!(ext_of("dir.x/file.TXT"), "txt");
        assert_eq!(ext_of(".bashrc"), "");

        let mut v = Viewer::new(0, "t.txt".into(), 11, b"abcdef\nxyz".to_vec(), false);
        v.wrap = true;
        v.layout(4);
        assert_eq!(v.rows, vec![(0, 0, 4), (0, 4, 6), (1, 7, 10)]);
        v.find = "Z".into();
        v.search();
        assert_eq!(v.found, vec![9]);
        assert_eq!(v.row_of(9), 2);
    }
}
