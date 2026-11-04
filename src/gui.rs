//! Graphical interface, shown when nikpak is started without arguments
//! (for example by double-clicking nikpak.exe).
//!
//! One minimalist, frameless window: drop files or folders onto it and they
//! are compressed into a .nik archive in the destination folder shown at the
//! bottom (remembered between sessions). The compression level (Fast / Medium /
//! Best, shown at the centre top) is Medium by default; while compressing, the
//! window also estimates the final size and time. Progress is shown as a large
//! percentage inside a circular meter.

use crate::archive::{self, PROGRESS, PHASE_FINISH, PHASE_SCAN};
use crate::cli::{Command, EpMode, Level, MediaMode, Options, Overwrite};
use crate::error::NikError;
use crate::i18n::{self, t, tf, Lang};
use crate::textshape::{self, Weight};
use crate::license::{self, License};
use crate::sfx::{self, Sfx};
use crate::skin;
use crate::util::PwdSpec;
use eframe::egui::{
    self, epaint::PathShape, text::LayoutJob, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily,
    FontId, Mesh, Pos2, Rect, Sense, Shape, Stroke, TextFormat, Vec2,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Look
// ---------------------------------------------------------------------------

pub(crate) const W: f32 = 500.0;
// 500 x 560 (about 9:10): the whole window fits an X (Twitter) picture post uncropped
pub(crate) const H: f32 = 560.0;
pub(crate) const RADIUS: f32 = 22.0;
/// Extra room to the right of the program, made while the skins box is open:
/// the box stands there, outside the program, so the interface stays visible.
/// That part of the window is transparent (clear_color is fully transparent).
pub(crate) const SKIN_SLOT: f32 = 560.0;

// The colours come from the skin (skin.rs: the same skins as Nikkomander).
// Nikkoid Blue, the default skin, is NikPak's original look.
#[allow(non_snake_case)]
pub(crate) fn ACCENT() -> Color32 {
    skin::c().accent
}
#[allow(non_snake_case)]
pub(crate) fn ACCENT_SOFT() -> Color32 {
    skin::c().accent_soft
}
/// Text on the accent colour (filled buttons).
#[allow(non_snake_case)]
pub(crate) fn ACCENT_FG() -> Color32 {
    skin::c().accent_fg
}
#[allow(non_snake_case)]
pub(crate) fn TEXT() -> Color32 {
    skin::c().text
}
#[allow(non_snake_case)]
pub(crate) fn TEXT_DIM() -> Color32 {
    skin::c().text_dim
}
#[allow(non_snake_case)]
pub(crate) fn TRACK() -> Color32 {
    skin::c().track
}
#[allow(non_snake_case)]
pub(crate) fn OK_GREEN() -> Color32 {
    skin::c().ok
}
#[allow(non_snake_case)]
pub(crate) fn ERR_RED() -> Color32 {
    skin::c().err
}
#[allow(non_snake_case)]
pub(crate) fn WARN() -> Color32 {
    skin::c().warn
}

/// Corner radius `n` (Nikkoid's), scaled for the skin (0 = square corners).
pub(crate) fn rr(n: u8) -> CornerRadius {
    CornerRadius::same(rad(n as f32).round() as u8)
}

pub(crate) fn rad(n: f32) -> f32 {
    n * skin::c().radius
}

/// A light highlight on dark skins, a dark one on bright skins.
pub(crate) fn hi(a: u8) -> Color32 {
    if skin::c().dark { alpha(Color32::WHITE, a) } else { alpha(Color32::from_rgb(0x10, 0x20, 0x40), a) }
}

/// Community links shown at the top right of the window (and in the guide).
pub const SOCIAL: [(&str, &str); 3] = [
    ("Telegram", "https://t.me/cryptonikko"),
    ("X", "https://x.com/cryptonikkoid"),
    ("Discord", "https://discord.com/users/1537826350759415811"),
];

/// The compression levels offered by the window, shown at the centre top and
/// clicked to change. Medium (zstd 6) is the default: a good size/speed balance.
const LEVELS: [(&str, Level); 3] = [
    ("Fast", Level::Zstd(1)),
    ("Medium", Level::Zstd(6)),
    ("Best", Level::Max),
];

pub(crate) fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub(crate) fn family(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub(crate) fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

/// The embedded fonts: Inter (Latin, Cyrillic) with a Chinese fallback.
pub(crate) fn font_definitions() -> FontDefinitions {
    let mut f = FontDefinitions::empty();
    for (name, bytes) in [
        ("display-light", &include_bytes!("../assets/fonts/InterDisplay-ExtraLight.otf")[..]),
        ("display", &include_bytes!("../assets/fonts/InterDisplay-Medium.otf")[..]),
        ("ui-light", &include_bytes!("../assets/fonts/Inter-Light.otf")[..]),
        ("ui", &include_bytes!("../assets/fonts/Inter-Medium.otf")[..]),
    ] {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
        f.families.insert(family(name), vec![name.into()]);
    }
    // Chinese: a subset of Noto Sans SC as fallback in every family
    f.font_data.insert("sc".into(), Arc::new(FontData::from_static(include_bytes!("../assets/fonts/NotoSansSC-subset.ttf"))));
    for name in ["display-light", "display", "ui-light", "ui"] {
        f.families.get_mut(&family(name)).unwrap().push("sc".into());
    }
    f.families.insert(FontFamily::Proportional, vec!["ui-light".into(), "sc".into()]);
    f.families.insert(FontFamily::Monospace, vec!["ui-light".into(), "sc".into()]);
    f
}

/// Window icon: a blue ring with a down arrow (drawn, no image file needed).
pub(crate) fn icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5 - 32.0, y as f32 + 0.5 - 32.0);
            let r = (fx * fx + fy * fy).sqrt();
            let mut a = 0.0f32;
            let mut col = (0x0Eu8, 0x22u8, 0x50u8);
            if r < 31.0 {
                a = 1.0;
            }
            if (r - 25.0).abs() < 3.2 {
                col = (0x1E, 0x9B, 0xFF);
            }
            // arrow: shaft and head
            let shaft = fx.abs() < 2.6 && fy > -14.0 && fy < 6.0;
            let head = fy >= 0.0 && fy < 12.0 && fx.abs() < 12.0 - fy;
            if shaft || head {
                col = (0xEA, 0xF4, 0xFF);
            }
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[col.0, col.1, col.2, (a * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

// ---------------------------------------------------------------------------
// Settings: the last destination folder
// ---------------------------------------------------------------------------

pub(crate) fn settings_file() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("NIKPAK_GUI_SETTINGS") {
        return Some(PathBuf::from(p));
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("nikpak").join("settings.txt"))
}

pub(crate) fn default_destination() -> PathBuf {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    match home {
        Some(h) if h.join("Desktop").is_dir() => h.join("Desktop"),
        Some(h) => h,
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

pub(crate) fn read_setting(key: &str) -> Option<String> {
    let text = std::fs::read_to_string(settings_file()?).ok()?;
    let prefix = format!("{key}=");
    text.lines().find_map(|l| l.strip_prefix(prefix.as_str()).map(|v| v.to_string()))
}

/// Settings file: one `key=value` per line (destination, language).
pub(crate) fn write_setting(key: &str, value: &str) {
    let Some(f) = settings_file() else { return };
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let prefix = format!("{key}=");
    let mut lines: Vec<String> = std::fs::read_to_string(&f)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with(prefix.as_str()) && !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();
    lines.push(format!("{key}={value}"));
    let _ = std::fs::write(f, lines.join("\n") + "\n");
}

pub(crate) fn load_destination() -> String {
    read_setting("destination")
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| default_destination().display().to_string())
}

pub(crate) fn save_destination(dest: &str) {
    write_setting("destination", dest.trim());
}

/// The legal statement lives in its own file (it has several lines), beside the settings.
fn legal_file() -> Option<PathBuf> {
    settings_file().and_then(|f| f.parent().map(|d| d.join("legal.txt")))
}

fn load_legal_text() -> String {
    legal_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default()
}

fn cover_file() -> Option<PathBuf> {
    settings_file().and_then(|f| f.parent().map(|d| d.join("cover.jpg")))
}

fn load_cover() -> Option<Vec<u8>> {
    cover_file().and_then(|f| std::fs::read(f).ok()).filter(|b| crate::cover::kind(b).is_some())
}

fn save_cover(on: bool, pic: Option<&[u8]>) {
    write_setting("cover", if on { "1" } else { "0" });
    let Some(f) = cover_file() else { return };
    match pic {
        Some(b) => {
            if let Some(dir) = f.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(f, b);
        }
        None => {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// A cover's texture for the window (decode once, keep the handle).
pub(crate) fn cover_texture(ctx: &egui::Context, id: &str, pic: &[u8]) -> Option<egui::TextureHandle> {
    let (w, h, px) = crate::cover::rgba(pic)?;
    let img = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &px);
    Some(ctx.load_texture(id.to_string(), img, egui::TextureOptions::LINEAR))
}

/// A square cover frame: the picture, or a hint when there is none.
pub(crate) fn paint_cover(p: &egui::Painter, r: Rect, tex: Option<&egui::TextureHandle>, hint: &str) {
    p.rect_filled(r, rr(10), alpha(skin::c().field, 190));
    match tex {
        Some(t) => {
            p.image(t.id(), r.shrink(1.0), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        None => {
            wrapped(p, Pos2::new(r.left() + 12.0, r.center().y - 16.0), r.width() - 24.0, hint, 12.0, TEXT_DIM());
        }
    }
    p.rect_stroke(r, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 90)), egui::StrokeKind::Inside);
}

/// The picture files a cover can be made from.
pub(crate) fn is_picture(p: &Path) -> bool {
    p.extension()
        .and_then(|x| x.to_str())
        .map(|x| matches!(x.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"))
        .unwrap_or(false)
}

fn save_legal(on: bool, text: &str) {
    write_setting("legal", if on { "1" } else { "0" });
    if let Some(f) = legal_file() {
        if let Some(dir) = f.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(f, text);
    }
}

/// Saved choice, else the language of the operating system, else English.
/// NIKPAK_GUI_LANG overrides both (screenshots).
pub(crate) fn load_language() -> Lang {
    std::env::var("NIKPAK_GUI_LANG")
        .ok()
        .and_then(|c| Lang::from_code(&c))
        .or_else(|| read_setting("language").and_then(|c| Lang::from_code(&c)))
        .or_else(i18n::system_language)
        .unwrap_or(Lang::En)
}

/// The other program of the package, next to this one: unNik.exe for NikPak,
/// nikpak.exe for unNik (no extension outside Windows).
pub(crate) fn sibling_exe(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    let p = dir.join(&file);
    if p.is_file() {
        return Some(p);
    }
    // Windows does not care about case, other systems might
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|q| {
        q.is_file() && q.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(&file))
    })
}

/// Start the other program (`name` = "unNik" or "nikpak") with `args`, then
/// close this window. Err(message) when it cannot be found or started.
pub(crate) fn switch_to(ctx: &egui::Context, name: &str, args: &[String]) -> Result<(), String> {
    let exe = sibling_exe(name).ok_or_else(|| tf("Cannot find {} next to this program.", &[&format!("{name}{}", if cfg!(windows) { ".exe" } else { "" })]))?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.args(args);
    // the new program plays the "switch" sound (this one closes at once)
    cmd.env("NIKPAK_SFX_SWITCHED", "1");
    if let Some(dir) = exe.parent() {
        cmd.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // nikpak.exe is a console program: no console window when started from a window
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd.spawn().map_err(|e| tf("Cannot start {}: {}", &[&exe.display().to_string(), &e.to_string()]))?;
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    Ok(())
}

/// Sounds on or off from the settings; the "switch" sound when this program
/// was started by the other one.
pub(crate) fn start_sounds() {
    sfx::set_enabled(read_setting("sounds").as_deref() != Some("off"));
    sfx::warm_up();
    if std::env::var_os("NIKPAK_SFX_SWITCHED").is_some() {
        // at start, before other threads read the environment
        std::env::remove_var("NIKPAK_SFX_SWITCHED");
        sfx::play(Sfx::Switch);
    }
}

/// Open a folder in Explorer / Finder / the file manager.
pub(crate) fn open_folder(p: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
}

/// The speaker button: sounds on / off (shared by both programs).
pub(crate) fn sound_button(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect) {
    let on = sfx::enabled();
    let (resp, col) = icon_button(ui, rect, "sounds");
    let resp = resp.on_hover_text(if on { tip("Sounds: on (click to switch them off)") } else { tip("Sounds: off (click to switch them on)") });
    let c = rect.center() + Vec2::new(-2.0, 0.0);
    let s = Stroke::new(1.3_f32, col);
    p.add(Shape::convex_polygon(
        vec![c + Vec2::new(-5.5, -2.5), c + Vec2::new(-3.0, -2.5), c + Vec2::new(0.5, -5.5), c + Vec2::new(0.5, 5.5), c + Vec2::new(-3.0, 2.5), c + Vec2::new(-5.5, 2.5)],
        col,
        Stroke::NONE,
    ));
    if on {
        p.add(PathShape::line(arc_points(c + Vec2::new(1.0, 0.0), 3.6, -0.9, 0.9), s));
        p.add(PathShape::line(arc_points(c + Vec2::new(1.0, 0.0), 6.6, -0.9, 0.9), s));
    } else {
        p.line_segment([c + Vec2::new(3.0, -3.0), c + Vec2::new(8.0, 3.0)], s);
        p.line_segment([c + Vec2::new(3.0, 3.0), c + Vec2::new(8.0, -3.0)], s);
    }
    if resp.clicked() {
        if on {
            sfx::play(Sfx::Off);
            sfx::set_enabled(false);
        } else {
            sfx::set_enabled(true);
            sfx::play(Sfx::On);
        }
        write_setting("sounds", if on { "off" } else { "on" });
    }
}

/// A folder with an arrow going out of it ("open the destination folder").
pub(crate) fn folder_open_glyph(p: &egui::Painter, c: Pos2, col: Color32) {
    let v = |x: f32, y: f32| c + Vec2::new(x, y);
    let s = Stroke::new(1.4_f32, col);
    p.add(PathShape::line(vec![v(1.0, 6.5), v(-8.0, 6.5), v(-8.0, -6.0), v(-3.5, -6.0), v(-1.5, -4.0), v(5.0, -4.0), v(5.0, -1.5)], s));
    p.line_segment([v(-1.0, 3.5), v(7.5, -5.0)], s);
    p.add(PathShape::line(vec![v(3.0, -5.5), v(7.5, -5.5), v(7.5, -1.0)], s));
}

/// Round button with the other program's icon: a small ring with an arrow
/// going up (unNik) or down (NikPak). Returns the response.
pub(crate) fn switch_button(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, to_unnik: bool, enabled: bool) -> egui::Response {
    let id = if to_unnik { "switch-unnik" } else { "switch-nikpak" };
    let (resp, col) = icon_button(ui, rect, id);
    let col = if enabled { col } else { alpha(TEXT_DIM(), 90) };
    let hint = if to_unnik { tip("Switch to unNik (browse and extract archives)") } else { tip("Switch to NikPak (create archives)") };
    let resp = resp.on_hover_text(hint).on_hover_cursor(if enabled { egui::CursorIcon::PointingHand } else { egui::CursorIcon::NotAllowed });
    let c = rect.center();
    p.circle_stroke(c, 7.5, Stroke::new(1.2_f32, col));
    let s = Stroke::new(1.4_f32, col);
    if to_unnik {
        p.line_segment([c + Vec2::new(0.0, 3.8), c + Vec2::new(0.0, -3.8)], s);
        p.add(PathShape::line(vec![c + Vec2::new(-2.6, -1.3), c + Vec2::new(0.0, -3.8), c + Vec2::new(2.6, -1.3)], s));
    } else {
        p.line_segment([c + Vec2::new(0.0, -3.8), c + Vec2::new(0.0, 3.8)], s);
        p.add(PathShape::line(vec![c + Vec2::new(-2.6, 1.3), c + Vec2::new(0.0, 3.8), c + Vec2::new(2.6, 1.3)], s));
    }
    // a small "swap" mark at the bottom right
    p.circle_filled(c + Vec2::new(6.0, 6.0), 3.6, if resp.hovered() && enabled { ACCENT() } else { alpha(ACCENT(), 170) });
    p.add(PathShape::line(vec![c + Vec2::new(4.6, 6.0), c + Vec2::new(7.4, 6.0)], Stroke::new(1.0_f32, ACCENT_FG())));
    p.add(PathShape::line(vec![c + Vec2::new(6.4, 5.0), c + Vec2::new(7.4, 6.0), c + Vec2::new(6.4, 7.0)], Stroke::new(1.0_f32, ACCENT_FG())));
    resp
}

/// NikPak and unNik share one settings file, so a language chosen in one is
/// used by the other. A running window also follows a change made by the
/// other program (checked every two seconds; NIKPAK_GUI_LANG wins).
pub(crate) fn follow_language(ctx: &egui::Context) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    ctx.request_repaint_after(std::time::Duration::from_secs(2));
    let now = ctx.input(|i| i.time);
    let tick = (now / 2.0) as u64 + 1;
    if LAST.swap(tick, Ordering::Relaxed) == tick {
        return;
    }
    sfx::set_enabled(read_setting("sounds").as_deref() != Some("off"));
    skin::follow();
    if std::env::var_os("NIKPAK_GUI_LANG").is_some() {
        return;
    }
    if let Some(l) = read_setting("language").and_then(|c| Lang::from_code(&c)) {
        if l != i18n::current() {
            i18n::set(l);
            ctx.request_repaint();
        }
    }
}

/// Start a new NikPak and close this one (after a language change).
pub(crate) fn restart(ctx: &egui::Context, args: &[String]) {
    if let Ok(exe) = std::env::current_exe() {
        let mut cmd = std::process::Command::new(exe);
        cmd.args(args);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
        }
        let _ = cmd.spawn();
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
}

// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

/// Name of the archive for the dropped items: the file name without its
/// extension, the folder name, or (for several items) the name of the folder
/// they were dropped from.
pub(crate) fn archive_stem(paths: &[PathBuf]) -> String {
    let name_of = |p: &Path| -> Option<String> {
        if p.is_dir() {
            p.file_name()
        } else {
            p.file_stem().or_else(|| p.file_name())
        }
        .map(|s| s.to_string_lossy().into_owned())
    };
    let stem = if paths.len() == 1 {
        name_of(&paths[0])
    } else {
        paths[0].parent().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned())
    };
    stem.filter(|s| !s.is_empty()).unwrap_or_else(|| "archive".into())
}

pub(crate) fn unique_archive_path(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.nik"));
    if !first.exists() {
        return first;
    }
    (2..10_000)
        .map(|i| dir.join(format!("{stem} ({i}).nik")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

struct Job {
    handle: JoinHandle<Result<u8, NikError>>,
    out: PathBuf,
    label: String,
    started: Instant,
}

enum Stage {
    Idle,
    Working(Job),
    Done { out: PathBuf, input: u64, output: u64, secs: f64, warnings: bool },
    Failed(String),
}

pub(crate) fn human(n: u64) -> String {
    let n = n as f64;
    if n >= 1e9 {
        format!("{:.2} GB", n / 1e9)
    } else if n >= 1e6 {
        format!("{:.1} MB", n / 1e6)
    } else if n >= 1e3 {
        format!("{:.0} KB", n / 1e3)
    } else {
        format!("{n:.0} B")
    }
}

/// A short, single-line duration (for the "time to compress" estimate).
pub(crate) fn human_secs(s: f64) -> String {
    if s < 90.0 {
        format!("{:.0} s", s)
    } else if s < 5400.0 {
        format!("{:.1} min", s / 60.0)
    } else {
        format!("{:.1} h", s / 3600.0)
    }
}

/// Open a web link in the default browser.
pub(crate) fn open_link(url: &str) {
    #[cfg(windows)]
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub(crate) fn show_in_folder(p: &Path) {
    #[cfg(windows)]
    {
        // explorer needs exactly /select,"path" - Rust's normal quoting of the
        // whole argument breaks paths with spaces, so pass it raw.
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{}\"", p.display())).spawn();
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(p.parent().unwrap_or(Path::new("."))).spawn();
}

// ---------------------------------------------------------------------------
// The application
// ---------------------------------------------------------------------------

/// Overlay boxes shown above the main window.
pub const COPYRIGHT: &str = "© 2026 Nicolas Choukroun";
pub const DISTRIBUTOR: &str = "Distributed by Nikkoid LLC";

/// "Jane Doe (personal licence)" in the interface language.
pub(crate) fn describe(l: &License) -> String {
    match l.kind {
        license::Kind::Personal => tf("{} (personal licence)", &[&l.name]),
        license::Kind::Company if l.seats == 1 => tf("{} (company licence, 1 seat)", &[&l.name]),
        license::Kind::Company => tf("{} (company licence, {} seats)", &[&l.name, &l.seats.to_string()]),
    }
}

enum Modal {
    None,
    /// shareware reminder; the two buttons swap sides at random
    Nag { register_left: bool },
    /// registration form
    Register { name: String, email: String, key: String, error: Option<String>, done: Option<String> },
    /// version, copyright and licence status (the small "i" button)
    About,
    /// a newer release on the web site: shown once, at launch
    Update(crate::update::UpdateInfo),
    /// the legal statement attached to new archives (the "Legal" pill)
    Legal { text: String, on: bool, error: Option<String> },
    /// the cover picture attached to new archives (the "Cover" pill)
    Cover { pic: Option<Vec<u8>>, on: bool, error: Option<String>, tex: Option<egui::TextureHandle> },
}

struct App {
    elevated: bool,
    dest: String,
    /// compression level for the next archive (shown centre top)
    level: Level,
    stage: Stage,
    start: Instant,
    shown: f32,
    license: Option<License>,
    modal: Modal,
    /// "choose files / choose a folder" buttons shown after a click on the drop icon
    chooser: bool,
    /// language menu open
    lang_open: bool,
    /// skins menu
    skins: SkinMenu,
    /// files were hovering over the window in the last frame (sound)
    was_hover: bool,
    /// the size the window was asked to take (it widens while the skins box
    /// is open, to give the box room beside the program)
    win_size: Vec2,
    /// the answer of the launch update check, when it arrives
    update_rx: Option<crate::update::Answer>,
    /// the update screen was shown once: this launch is done asking
    update_shown: bool,
    /// attach the legal statement below to every new archive
    legal_on: bool,
    /// the legal statement (kept in legal.txt beside the settings)
    legal_text: String,
    /// attach the cover below to every new archive
    cover_on: bool,
    /// the cover (a 1024 x 1024 JPEG, kept in cover.jpg beside the settings)
    cover: Option<Vec<u8>>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, shell_job: Option<(Vec<PathBuf>, PathBuf)>) -> Self {
        install_fonts(&cc.egui_ctx);
        skin::sync_style(&cc.egui_ctx);
        let lic = license::current();
        let modal = if lic.is_some() || std::env::var_os("NIKPAK_GUI_NO_NAG").is_some() {
            Modal::None
        } else {
            Modal::Nag { register_left: rand::random::<bool>() }
        };
        let mut app = App {
            // NIKPAK_GUI_CAPTURE: the window is being photographed (buildskins.ps1).
            // It then stays elevated instead of restarting through Explorer, so the
            // shell that started it - often an administrator one - owns the window
            // and can copy it off the screen. The warning about drag and drop is
            // left out too: the picture must show the program as users see it.
            elevated: is_elevated() && std::env::var_os("NIKPAK_GUI_CAPTURE").is_none(),
            dest: load_destination(),
            level: LEVELS[1].1,
            stage: Stage::Idle,
            start: Instant::now(),
            shown: 0.0,
            license: lic,
            modal,
            chooser: false,
            lang_open: std::env::var_os("NIKPAK_GUI_LANGMENU").is_some(),
            skins: SkinMenu { open: std::env::var_os("NIKPAK_GUI_SKINMENU").is_some(), beside: true, ..Default::default() },
            was_hover: false,
            win_size: Vec2::new(W, H),
            // the check runs in the background; the window opens immediately
            update_rx: if std::env::var_os("NIKPAK_GUI_NO_UPDATE").is_some() { None } else { Some(crate::update::check_in_background()) },
            update_shown: false,
            legal_on: read_setting("legal").as_deref() == Some("1"),
            legal_text: load_legal_text(),
            cover_on: read_setting("cover").as_deref() == Some("1"),
            cover: load_cover(),
        };
        // Test hooks (screenshots)
        match std::env::var("NIKPAK_GUI_MODAL").as_deref() {
            Ok("register") => app.modal = Modal::Register { name: String::new(), email: String::new(), key: String::new(), error: None, done: None },
            Ok("nag-left") => app.modal = Modal::Nag { register_left: true },
            Ok("nag-right") => app.modal = Modal::Nag { register_left: false },
            Ok("about") => app.modal = Modal::About,
            Ok("legal") => app.modal = Modal::Legal { text: app.legal_text.clone(), on: true, error: None },
            _ => {}
        }
        if std::env::var_os("NIKPAK_GUI_CHOOSER").is_some() {
            app.chooser = true;
        }
        // Test hook (screenshots): NIKPAK_GUI_DEMO="path1;path2" starts compressing at once.
        if let Ok(list) = std::env::var("NIKPAK_GUI_DEMO") {
            let paths: Vec<PathBuf> = list.split(';').filter(|s| !s.is_empty()).map(PathBuf::from).collect();
            app.start_job(paths);
        }
        // started from the right-click menu: compress at once to the chosen archive
        if let Some((items, out)) = shell_job {
            app.start_job_to(items, out);
        }
        app
    }

    /// Widen the window while the skins box is open, so the box can stand
    /// beside the program, and put it back when the box closes. The extra
    /// part of the window stays transparent (clear_color has alpha 0); the
    /// program keeps drawing its 500 x 560 box at the top left.
    fn fit_skins_window(&mut self, ctx: &egui::Context) {
        let want = if self.skins.open { Vec2::new(W + SKIN_SLOT, H) } else { Vec2::new(W, H) };
        if self.win_size == want {
            return;
        }
        self.win_size = want;
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        // never wider than the screen: on a small screen the box then stands
        // a little over the program, as it used to
        let mut size = want;
        if let Some(m) = monitor {
            size.x = size.x.min(m.x - 24.0).max(W);
        }
        // a window that is not "resizable" may ignore a new size on some
        // systems; unNik does the same when it grows for an archive
        ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(size.x > W + 1.0));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        // slide the window left when the wider window would run off the right
        // of the screen (the monitor is taken to start at 0, as in unNik)
        if let (Some(o), Some(m)) = (outer, monitor) {
            if o.min.x + size.x > m.x - 8.0 {
                let x = (m.x - 8.0 - size.x).max(8.0);
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(x, o.min.y)));
            }
        }
    }

    fn start_job(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() || matches!(self.stage, Stage::Working(_)) {
            return;
        }
        let dest = PathBuf::from(self.dest.trim());
        if self.dest.trim().is_empty() {
            self.stage = Stage::Failed(t("Choose a destination folder first.").into());
            return;
        }
        if !dest.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dest) {
                self.stage = Stage::Failed(tf("Cannot use the destination folder: {}", &[&e.to_string()]));
                return;
            }
        }
        save_destination(&self.dest);
        let out = unique_archive_path(&dest, &archive_stem(&paths));
        self.start_job_to(paths, out);
    }

    /// Compress `paths` into the archive `out` (the drop window, or the right-click menu).
    fn start_job_to(&mut self, paths: Vec<PathBuf>, out: PathBuf) {
        if paths.is_empty() || matches!(self.stage, Stage::Working(_)) {
            return;
        }
        let label = if paths.len() == 1 {
            paths[0].file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
        } else {
            tf("{} items", &[&paths.len().to_string()])
        };
        let opts = Options {
            command: Command::Add,
            archive: out.display().to_string(),
            files: paths.iter().map(|p| p.display().to_string()).collect(),
            password: PwdSpec::None,
            level: self.level,
            ep: EpMode::Names,
            overwrite: Overwrite::All,
            quiet: true,
            dest: None,
            excludes: Vec::new(),
            threads: 0,
            media: MediaMode::Auto,
            // the terms are attached when the "Legal" pill is on
            legal: if self.legal_on && !self.legal_text.trim().is_empty() { Some(self.legal_text.clone()) } else { None },
            accept_legal: false,
            // and the cover when the "Cover" pill is on
            cover: if self.cover_on { self.cover.clone() } else { None },
        };
        PROGRESS.cancel.store(false, Relaxed);
        PROGRESS.done.store(0, Relaxed);
        PROGRESS.total.store(0, Relaxed);
        PROGRESS.phase.store(PHASE_SCAN, Relaxed);
        self.shown = 0.0;
        let handle = std::thread::spawn(move || archive::create(&opts));
        self.stage = Stage::Working(Job { handle, out, label, started: Instant::now() });
        sfx::play(Sfx::Start);
    }

    fn poll_job(&mut self) {
        let finished = matches!(&self.stage, Stage::Working(j) if j.handle.is_finished());
        if !finished {
            return;
        }
        let Stage::Working(job) = std::mem::replace(&mut self.stage, Stage::Idle) else { return };
        let secs = job.started.elapsed().as_secs_f64();
        self.stage = match job.handle.join() {
            Ok(Ok(code)) => Stage::Done {
                input: PROGRESS.total.load(Relaxed),
                output: std::fs::metadata(&job.out).map(|m| m.len()).unwrap_or(0),
                out: job.out,
                secs,
                warnings: code != 0,
            },
            Ok(Err(e)) if PROGRESS.cancel.load(Relaxed) => {
                let _ = e;
                Stage::Failed(t("Cancelled. Nothing was written.").into())
            }
            Ok(Err(e)) => Stage::Failed(e.to_string()),
            Err(_) => Stage::Failed(t("Internal error while compressing.").into()),
        };
        sfx::play(match &self.stage {
            Stage::Done { warnings: false, .. } => Sfx::Success,
            Stage::Failed(_) if PROGRESS.cancel.load(Relaxed) => Sfx::Cancel,
            _ => Sfx::Error,
        });
    }
}

// --- drawing helpers --------------------------------------------------------

/// Rounded rectangle filled with a vertical gradient.
pub(crate) fn gradient_panel(p: &egui::Painter, r: Rect, radius: f32, top: Color32, bottom: Color32) {
    gradient_panel_px(p, r, rad(radius), top, bottom);
}

/// The same with a radius that is not scaled for the skin.
pub(crate) fn gradient_panel_px(p: &egui::Painter, r: Rect, radius: f32, top: Color32, bottom: Color32) {
    let pts = skin::rounded_points(r, radius);
    let lerp = |y: f32| {
        let t = ((y - r.top()) / r.height()).clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        // premultiplied colours: interpolating them keeps translucent panels right
        Color32::from_rgba_premultiplied(m(top.r(), bottom.r()), m(top.g(), bottom.g()), m(top.b(), bottom.b()), m(top.a(), bottom.a()))
    };
    let mut mesh = Mesh::default();
    let center = r.center();
    mesh.colored_vertex(center, lerp(center.y));
    for q in &pts {
        mesh.colored_vertex(*q, lerp(q.y));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

/// A glow colour weakened (or removed) for the skin: flat skins have no glow.
pub(crate) fn glow(c: Color32) -> Color32 {
    let g = skin::c().glow;
    if g >= 1.0 || (c.r() == 0 && c.g() == 0 && c.b() == 0) {
        return c; // shadows stay
    }
    c.gamma_multiply(g)
}

/// Soft radial glow (transparent at the edge).
pub(crate) fn radial_glow(p: &egui::Painter, c: Pos2, radius: f32, col: Color32) {
    let col = glow(col);
    if col.a() == 0 {
        return;
    }
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, col);
    let n = 72u32;
    for i in 0..n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(c + Vec2::new(a.cos(), a.sin()) * radius, Color32::TRANSPARENT);
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(Shape::mesh(mesh));
}

pub(crate) fn arc_points(c: Pos2, r: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let steps = (((a1 - a0).abs() / std::f32::consts::TAU) * 180.0).ceil().max(2.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / steps as f32;
            c + Vec2::new(a.cos(), a.sin()) * r
        })
        .collect()
}

/// Progress arc from 12 o'clock, clockwise, with a glow and round caps.
pub(crate) fn progress_arc(p: &egui::Painter, c: Pos2, r: f32, frac: f32, col: Color32) {
    if frac <= 0.0 {
        return;
    }
    let a0 = -std::f32::consts::FRAC_PI_2;
    let a1 = a0 + frac.min(1.0) * std::f32::consts::TAU;
    let pts = arc_points(c, r, a0, a1);
    for (w, a) in [(30.0_f32, 10u8), (20.0, 22), (13.0, 45)] {
        p.add(PathShape::line(pts.clone(), Stroke::new(w, glow(alpha(col, a)))));
    }
    p.add(PathShape::line(pts.clone(), Stroke::new(9.0_f32, col)));
    if frac < 1.0 {
        for q in [pts[0], *pts.last().unwrap()] {
            p.circle_filled(q, 4.5, col);
        }
    }
}

// ---------------------------------------------------------------------------
// Text: egui lays out Latin, Cyrillic and Chinese itself; Hindi, Bengali and
// Arabic go through the shaper (textshape.rs) and are painted as images.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub(crate) enum Txt {
    G(Arc<egui::Galley>),
    S(Arc<textshape::Shaped>),
}

impl Txt {
    pub(crate) fn size(&self) -> Vec2 {
        match self {
            Txt::G(g) => g.size(),
            Txt::S(s) => s.size,
        }
    }
    pub(crate) fn paint(&self, p: &egui::Painter, pos: Pos2, color: Color32) {
        match self {
            Txt::G(g) => p.galley_with_override_text_color(pos, g.clone(), color),
            Txt::S(s) => {
                p.image(s.texture.id(), Rect::from_min_size(pos, s.size), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), color);
            }
        }
    }
}

pub(crate) fn weight_of(font: &FontId) -> Weight {
    match &font.family {
        FontFamily::Name(n) if &**n == "ui" => Weight::Medium,
        FontFamily::Name(n) if &**n == "display" => Weight::DisplayMedium,
        FontFamily::Name(n) if &**n == "display-light" => Weight::DisplayLight,
        _ => Weight::Light,
    }
}

pub(crate) fn lay(p: &egui::Painter, s: &str, font: FontId) -> Txt {
    if textshape::needs_shaping(s) {
        Txt::S(textshape::shaped(p.ctx(), s, weight_of(&font), font.size))
    } else {
        Txt::G(p.layout_no_wrap(s.to_string(), font, TEXT()))
    }
}

/// Like `lay`, but a smaller font when the text is wider than `max_w` (long translations).
pub(crate) fn lay_fit(p: &egui::Painter, s: &str, font: FontId, max_w: f32) -> Txt {
    let t = lay(p, s, font.clone());
    let w = t.size().x;
    if max_w > 0.0 && w > max_w {
        lay(p, s, FontId::new((font.size * max_w / w).max(8.0), font.family))
    } else {
        t
    }
}

pub(crate) fn text(p: &egui::Painter, pos: Pos2, align: Align2, s: impl AsRef<str>, font: FontId, color: Color32) -> Rect {
    let t = lay(p, s.as_ref(), font);
    let r = align.anchor_size(pos, t.size());
    t.paint(p, r.min, color);
    r
}

pub(crate) fn text_fit(p: &egui::Painter, pos: Pos2, align: Align2, s: impl AsRef<str>, font: FontId, color: Color32, max_w: f32) -> Rect {
    let t = lay_fit(p, s.as_ref(), font, max_w);
    let r = align.anchor_size(pos, t.size());
    t.paint(p, r.min, color);
    r
}

/// Paint text with a small, dark drop shadow so it stays readable on bright
/// wallpapers; on dark skins the shadow is invisible. One soft copy, not a glow.
pub(crate) fn paint_shadowed(p: &egui::Painter, t: &Txt, pos: Pos2, color: Color32) {
    t.paint(p, pos + Vec2::new(0.0, 1.6), Color32::from_black_alpha(96));
    t.paint(p, pos, color);
}

/// Tooltips are drawn by egui itself: keep English for scripts it cannot shape.
pub(crate) fn tip(s: &'static str) -> &'static str {
    let tr = t(s);
    if textshape::needs_shaping(tr) { s } else { tr }
}

/// Letter-spaced capitals (labels). Shaped scripts are not letter-spaced.
pub(crate) fn spaced(p: &egui::Painter, text: &str, font: FontId, color: Color32, spacing: f32) -> Txt {
    if textshape::needs_shaping(text) {
        // no capitals in these scripts: a slightly larger size instead of letter spacing
        return lay(p, text, FontId::new(font.size * 1.25, font.family));
    }
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() });
    Txt::G(p.layout_job(job))
}

pub(crate) fn spaced_fit(p: &egui::Painter, text: &str, font: FontId, color: Color32, spacing: f32, max_w: f32) -> Txt {
    let g = spaced(p, text, font.clone(), color, spacing);
    let w = g.size().x;
    if w > max_w {
        spaced(p, text, FontId::new((font.size * max_w / w).max(7.0), font.family), color, spacing * max_w / w)
    } else {
        g
    }
}

/// Text centred on `c`, with a soft glow made of offset translucent copies.
pub(crate) fn glow_text(p: &egui::Painter, c: Pos2, text: &str, font: FontId, color: Color32, glow_col: Color32, spacing: f32) -> Rect {
    let g = spaced_fit(p, text, font, color, spacing, W - 80.0);
    let glow_col = glow(glow_col);
    // letter spacing is also added after the last glyph: compensate
    let size = match g {
        Txt::G(_) => g.size() - Vec2::new(spacing, 0.0),
        Txt::S(_) => g.size(),
    };
    let pos = c - size / 2.0;
    if glow_col.a() > 0 {
        for r in [2.0f32, 4.0] {
            for k in 0..12 {
                let a = k as f32 / 12.0 * std::f32::consts::TAU;
                g.paint(p, pos + Vec2::new(a.cos(), a.sin()) * r, glow_col);
            }
        }
    }
    g.paint(p, pos, color);
    Rect::from_min_size(pos, size)
}

/// Round, brand-coloured buttons for the community links.
pub(crate) fn brand_icon(p: &egui::Painter, name: &str, c: Pos2, radius: f32, hovered: bool) {
    let k = radius / 19.0;
    let v = |x: f32, y: f32| c + Vec2::new(x * k, y * k);
    let white = Color32::WHITE;
    if hovered {
        radial_glow(p, c, radius * 1.9, alpha(ACCENT_SOFT(), 70));
    }
    match name {
        "Telegram" => {
            p.circle_filled(c, radius, Color32::from_rgb(0x22, 0x9E, 0xD9));
            // paper plane
            p.add(PathShape::convex_polygon(vec![v(-9.5, -0.6), v(8.8, -8.0), v(5.6, 8.6)], white, Stroke::NONE));
            p.add(PathShape::convex_polygon(vec![v(-2.4, 2.6), v(8.8, -8.0), v(-0.4, 8.4)], Color32::from_rgb(0xC8, 0xDA, 0xEA), Stroke::NONE));
            p.add(PathShape::convex_polygon(vec![v(-2.4, 2.6), v(-0.4, 8.4), v(2.4, 5.0)], white, Stroke::NONE));
        }
        "X" => {
            p.circle_filled(c, radius, Color32::from_rgb(0x0F, 0x14, 0x19));
            p.circle_stroke(c, radius - 0.5, Stroke::new(1.0_f32, alpha(white, 70)));
            p.line_segment([v(-6.5, -7.0), v(6.5, 7.0)], Stroke::new(3.0 * k, white));
            p.line_segment([v(6.5, -7.0), v(-6.5, 7.0)], Stroke::new(1.6 * k, white));
        }
        _ => {
            p.circle_filled(c, radius, Color32::from_rgb(0x58, 0x65, 0xF2));
            // controller-shaped face with two eyes
            p.add(PathShape::convex_polygon(
                vec![v(-4.5, 7.0), v(-9.0, 4.0), v(-7.2, -4.6), v(-3.6, -6.6), v(3.6, -6.6), v(7.2, -4.6), v(9.0, 4.0), v(4.5, 7.0)],
                white,
                Stroke::NONE,
            ));
            let blurple = Color32::from_rgb(0x58, 0x65, 0xF2);
            p.circle_filled(v(-3.1, 0.6), 1.9 * k, blurple);
            p.circle_filled(v(3.1, 0.6), 1.9 * k, blurple);
        }
    }
}

/// Clickable text; returns true when clicked.
pub(crate) fn link_font() -> FontId {
    FontId::new(13.0, family("ui"))
}

pub(crate) fn text_link(ui: &mut egui::Ui, p: &egui::Painter, pos: Pos2, text: &str, id: &str) -> bool {
    let g = lay(p, text, link_font());
    let rect = Rect::from_min_size(pos, g.size()).expand(3.0);
    let resp = ui.interact(rect, egui::Id::new(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let col = if resp.hovered() { TEXT() } else { ACCENT_SOFT() };
    paint_shadowed(p, &g, pos, col);
    let uy = pos.y + g.size().y - if matches!(g, Txt::S(_)) { 5.0 } else { 0.0 };
    p.line_segment([Pos2::new(pos.x, uy), Pos2::new(pos.x + g.size().x, uy)], Stroke::new(1.0_f32, alpha(col, if resp.hovered() { 200 } else { 70 })));
    if resp.clicked() {
        sfx::play(Sfx::Click);
    }
    resp.clicked()
}

// Dialogs are attached to the NikPak window (set_parent) so that Windows shows
// them in front of it, not behind it (seen on Windows 1.4.1).
pub(crate) fn pick_files(parent: &eframe::Frame) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().set_parent(parent).set_title(t("Choose the files to compress")).pick_files()
}

pub(crate) fn pick_folder(parent: &eframe::Frame) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().set_parent(parent).set_title(t("Choose the folder to compress")).pick_folder().map(|f| vec![f])
}

/// Windows does not let Explorer drag files onto a program that runs as
/// administrator (UIPI); the window then says so.
#[cfg(windows)]
pub(crate) fn is_elevated() -> bool {
    #[link(name = "shell32")]
    extern "system" {
        fn IsUserAnAdmin() -> i32;
    }
    // SAFETY: argument-less Win32 query.
    unsafe { IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub(crate) fn is_elevated() -> bool {
    std::env::var_os("NIKPAK_GUI_ELEVATED").is_some()
}

pub(crate) fn icon_button(ui: &mut egui::Ui, rect: Rect, id: &str) -> (egui::Response, Color32) {
    let resp = ui.interact(rect, egui::Id::new(id), Sense::click());
    if resp.clicked() {
        sfx::play(Sfx::Click);
    }
    let col = if resp.hovered() { TEXT() } else { TEXT_DIM() };
    if resp.hovered() {
        ui.painter().circle_filled(rect.center(), rect.width() / 2.0, alpha(ACCENT(), 40));
    }
    (resp, col)
}

/// Filled (primary) or outlined button; returns true when clicked.
pub(crate) fn pill_button(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, label: &str, primary: bool, id: &str) -> bool {
    let resp = ui.interact(rect, egui::Id::new(id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        sfx::play(Sfx::Click);
    }
    let h = resp.hovered();
    if primary {
        if h {
            radial_glow(p, rect.center(), rect.width() * 0.75, alpha(ACCENT(), 60));
        }
        p.rect_filled(rect, rr(12), if h { skin::c().accent_btn_hover() } else { skin::c().accent_btn() });
        text_fit(p, rect.center(), Align2::CENTER_CENTER, label, FontId::new(14.0, family("ui")), ACCENT_FG(), rect.width() - 18.0);
    } else {
        p.rect_filled(rect, rr(12), alpha(ACCENT(), if h { 50 } else { 18 }));
        p.rect_stroke(rect, rr(12), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if h { 200 } else { 110 })), egui::StrokeKind::Inside);
        text_fit(p, rect.center(), Align2::CENTER_CENTER, label, FontId::new(14.0, family("ui")), if h { TEXT() } else { alpha(TEXT(), 220) }, rect.width() - 18.0);
    }
    resp.clicked()
}

pub(crate) fn lang_button_rect(panel: Rect) -> Rect {
    Rect::from_min_size(Pos2::new(panel.left() + 20.0, panel.bottom() - 32.0 - 15.0), Vec2::new(170.0, 30.0))
}

/// Small flags for the language menu, drawn with shapes (no images, no emoji font).
pub(crate) fn draw_flag(p: &egui::Painter, r: Rect, lang: Lang) {
    let rgb = Color32::from_rgb;
    let fp = p.with_clip_rect(r.intersect(p.clip_rect()));
    let (w, h) = (r.width(), r.height());
    let at = |x: f32, y: f32| Pos2::new(r.left() + x * w, r.top() + y * h);
    let hbands = |cols: &[Color32], weights: &[f32]| {
        let sum: f32 = weights.iter().sum();
        let mut y = r.top();
        for (c, wt) in cols.iter().zip(weights) {
            let hh = h * wt / sum;
            fp.rect_filled(Rect::from_min_size(Pos2::new(r.left(), y), Vec2::new(w, hh + 0.5)), 0.0, *c);
            y += hh;
        }
    };
    let vbands = |cols: &[Color32]| {
        for (k, c) in cols.iter().enumerate() {
            let ww = w / cols.len() as f32;
            fp.rect_filled(Rect::from_min_size(Pos2::new(r.left() + k as f32 * ww, r.top()), Vec2::new(ww + 0.5, h)), 0.0, *c);
        }
    };
    let star = |c: Pos2, ro: f32, col: Color32| {
        let mut mesh = Mesh::default();
        mesh.colored_vertex(c, col);
        for k in 0..=10 {
            let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
            let rr = if k % 2 == 0 { ro } else { ro * 0.42 };
            mesh.colored_vertex(c + Vec2::new(a.cos(), a.sin()) * rr, col);
        }
        for k in 1..=10u32 {
            mesh.add_triangle(0, k, k + 1);
        }
        fp.add(Shape::mesh(mesh));
    };
    let white = Color32::WHITE;
    match lang {
        Lang::En => {
            fp.rect_filled(r, 0.0, rgb(0x01, 0x21, 0x69));
            for (a, b) in [(at(0.0, 0.0), at(1.0, 1.0)), (at(0.0, 1.0), at(1.0, 0.0))] {
                fp.line_segment([a, b], Stroke::new(h * 0.3, white));
                fp.line_segment([a, b], Stroke::new(h * 0.1, rgb(0xC8, 0x10, 0x2E)));
            }
            fp.rect_filled(Rect::from_center_size(r.center(), Vec2::new(w, h * 0.34)), 0.0, white);
            fp.rect_filled(Rect::from_center_size(r.center(), Vec2::new(h * 0.34, h)), 0.0, white);
            fp.rect_filled(Rect::from_center_size(r.center(), Vec2::new(w, h * 0.2)), 0.0, rgb(0xC8, 0x10, 0x2E));
            fp.rect_filled(Rect::from_center_size(r.center(), Vec2::new(h * 0.2, h)), 0.0, rgb(0xC8, 0x10, 0x2E));
        }
        Lang::Zh => {
            let y = rgb(0xFF, 0xDE, 0x00);
            fp.rect_filled(r, 0.0, rgb(0xDE, 0x29, 0x10));
            star(at(0.17, 0.3), h * 0.2, y);
            for (x, yy) in [(0.33, 0.1), (0.4, 0.22), (0.4, 0.38), (0.33, 0.5)] {
                star(at(x, yy), h * 0.07, y);
            }
        }
        Lang::Hi => {
            hbands(&[rgb(0xFF, 0x99, 0x33), white, rgb(0x13, 0x88, 0x08)], &[1.0, 1.0, 1.0]);
            fp.circle_stroke(r.center(), h * 0.13, Stroke::new(0.9_f32, rgb(0x00, 0x00, 0x80)));
        }
        Lang::Es => hbands(&[rgb(0xAA, 0x15, 0x1B), rgb(0xF1, 0xBF, 0x00), rgb(0xAA, 0x15, 0x1B)], &[1.0, 2.0, 1.0]),
        Lang::Ar => {
            fp.rect_filled(r, 0.0, rgb(0x00, 0x6C, 0x35));
            fp.line_segment([at(0.24, 0.36), at(0.76, 0.36)], Stroke::new(h * 0.1, white));
            fp.line_segment([at(0.3, 0.66), at(0.72, 0.66)], Stroke::new(h * 0.07, white));
        }
        Lang::Fr => vbands(&[rgb(0x00, 0x55, 0xA4), white, rgb(0xEF, 0x41, 0x35)]),
        Lang::Bn => {
            fp.rect_filled(r, 0.0, rgb(0x00, 0x6A, 0x4E));
            fp.circle_filled(at(0.45, 0.5), h * 0.3, rgb(0xF4, 0x2A, 0x41));
        }
        Lang::Pt => {
            fp.rect_filled(r, 0.0, rgb(0x00, 0x9C, 0x3B));
            fp.add(Shape::convex_polygon(vec![at(0.08, 0.5), at(0.5, 0.1), at(0.92, 0.5), at(0.5, 0.9)], rgb(0xFF, 0xDF, 0x00), Stroke::NONE));
            fp.circle_filled(r.center(), h * 0.22, rgb(0x00, 0x27, 0x76));
        }
        Lang::Ru => hbands(&[white, rgb(0x00, 0x39, 0xA6), rgb(0xD5, 0x2B, 0x1E)], &[1.0, 1.0, 1.0]),
        Lang::Id => hbands(&[rgb(0xCE, 0x11, 0x26), white], &[1.0, 1.0]),
    }
    p.rect_stroke(r, rr(2), Stroke::new(0.8_f32, hi(50)), egui::StrokeKind::Outside);
}

pub(crate) fn card(p: &egui::Painter, rect: Rect) {
    radial_glow(p, rect.center(), rect.width() * 0.85, alpha(ACCENT(), 40));
    gradient_panel(p, rect, 18.0, skin::c().dialog_top, skin::c().dialog_bottom);
    p.rect_stroke(rect, rr(18), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 90)), egui::StrokeKind::Inside);
}

/// Height that `wrapped` will use, without painting.
pub(crate) fn wrapped_height(p: &egui::Painter, width: f32, text: &str, size: f32) -> f32 {
    if textshape::needs_shaping(text) {
        return textshape::wrap(text, Weight::Light, size, width).len() as f32 * size * 1.6;
    }
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.append(text, 0.0, TextFormat { font_id: FontId::new(size, family("ui-light")), line_height: Some(size * 1.45), ..Default::default() });
    p.layout_job(job).size().y
}

/// Top-left corner for a small label meant to start at `pos` (shaped scripts are taller).
pub(crate) fn label_pos(lab: &Txt, pos: Pos2) -> Pos2 {
    match lab {
        Txt::G(_) => pos,
        Txt::S(_) => Pos2::new(pos.x, pos.y + 6.0 - lab.size().y / 2.0),
    }
}

/// A small label: vertically centred on `top + 6` (shaped scripts are taller),
/// left-aligned at `left`, or right-aligned at `right` for right-to-left languages.
pub(crate) fn paint_label(p: &egui::Painter, lab: &Txt, left: f32, right: f32, top: f32, color: Color32) {
    let x = if i18n::current().rtl() { right - lab.size().x } else { left };
    let y = match lab {
        Txt::G(_) => top,
        Txt::S(_) => top + 6.0 - lab.size().y / 2.0,
    };
    lab.paint(p, Pos2::new(x, y), color);
}

pub(crate) fn wrapped(p: &egui::Painter, pos: Pos2, width: f32, text: &str, size: f32, color: Color32) -> f32 {
    if textshape::needs_shaping(text) {
        let lh = size * 1.6;
        let mut y = pos.y;
        for line in textshape::wrap(text, Weight::Light, size, width) {
            let s = textshape::shaped(p.ctx(), &line, Weight::Light, size);
            let top = y + (lh - s.size.y) / 2.0;
            p.image(s.texture.id(), Rect::from_min_size(Pos2::new(pos.x + (width - s.size.x) / 2.0, top), s.size), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), color);
            y += lh;
        }
        return y - pos.y;
    }
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.halign = egui::Align::Center;
    job.append(text, 0.0, TextFormat { font_id: FontId::new(size, family("ui-light")), color, line_height: Some(size * 1.45), ..Default::default() });
    let g = p.layout_job(job);
    let h = g.size().y;
    // centred layout: rows are centred around x = 0 of the galley
    p.galley(Pos2::new(pos.x + width / 2.0, pos.y), g, color);
    h
}

/// The language menu above the language button. A choice is saved and the
/// program restarts in the new language (with `restart_args`).
pub(crate) fn lang_menu(ctx: &egui::Context, panel: Rect, lb: Rect, open: &mut bool, restart_args: &[String]) {
    if !*open || panel == Rect::NOTHING {
        return;
    }
    let row_h = 30.0;
    let head = 34.0;
    let h = head + i18n::ALL.len() as f32 * row_h + 10.0;
    let menu = Rect::from_min_size(Pos2::new(lb.left() - 4.0, lb.top() - 8.0 - h), Vec2::new(216.0, h));
    let cur = i18n::current();
    egui::Area::new(egui::Id::new("langmenu"))
        .order(egui::Order::Foreground)
        .fixed_pos(panel.min)
        .show(ctx, |ui| {
            ui.set_min_size(panel.size());
            let p = ui.painter().clone();
            // a click outside the menu closes it
            let outside = ui.interact(panel, egui::Id::new("lang-blocker"), Sense::click());
            if outside.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                *open = false;
            }
            radial_glow(&p, menu.center(), menu.width(), alpha(ACCENT(), 30));
            gradient_panel(&p, menu, 14.0, skin::c().dialog_top, skin::c().dialog_bottom);
            p.rect_stroke(menu, rr(14), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 100)), egui::StrokeKind::Inside);
            let lab = spaced(&p, t("LANGUAGE"), FontId::new(10.0, family("ui")), TEXT_DIM(), 2.0);
            paint_label(&p, &lab, menu.left() + 16.0, menu.right() - 16.0, menu.top() + 13.0, TEXT_DIM());
            for (i, l) in i18n::ALL.iter().enumerate() {
                let row = Rect::from_min_size(Pos2::new(menu.left() + 6.0, menu.top() + head + i as f32 * row_h), Vec2::new(menu.width() - 12.0, row_h));
                let r = ui.interact(row, egui::Id::new(("lang-row", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                if r.hovered() || *l == cur {
                    p.rect_filled(row, rr(8), alpha(ACCENT(), if r.hovered() { 60 } else { 26 }));
                }
                let fr = Rect::from_min_size(Pos2::new(row.left() + 10.0, row.center().y - 7.5), Vec2::new(22.0, 15.0));
                draw_flag(&p, fr, *l);
                let nt = lay(&p, l.native_name(), FontId::new(14.0, family("ui-light")));
                nt.paint(&p, Pos2::new(fr.right() + 12.0, row.center().y - nt.size().y / 2.0), if *l == cur { TEXT() } else { alpha(TEXT(), 210) });
                if *l == cur {
                    let c = Pos2::new(row.right() - 16.0, row.center().y);
                    p.add(PathShape::line(
                        vec![c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.5, 3.5), c + Vec2::new(5.0, -4.0)],
                        Stroke::new(1.6_f32, ACCENT_SOFT()),
                    ));
                }
                if r.clicked() {
                    sfx::play(Sfx::Click);
                    *open = false;
                    if *l != cur {
                        // the new language is used from the next start: restart now
                        write_setting("language", l.code());
                        restart(ctx, restart_args);
                    }
                }
            }
        });
}

// ---------------------------------------------------------------------------
// Skins: the palette button, the sun / moon button and the skins menu
// ---------------------------------------------------------------------------

/// The palette button (opens the skins menu). Returns true when clicked.
pub(crate) fn skin_button(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect, open: bool) -> bool {
    let (resp, col) = icon_button(ui, rect, "skins");
    let resp = resp.on_hover_text(tip("Skins: the colours of the window"));
    let col = if open { TEXT() } else { col };
    let c = rect.center();
    // a painter's palette: a ring with a thumb hole and three colour dots
    let k = skin::c();
    p.add(PathShape::line(arc_points(c, 7.0, 0.95, std::f32::consts::TAU + 0.35), Stroke::new(1.3_f32, col)));
    p.circle_stroke(c + Vec2::new(3.2, 3.0), 1.6, Stroke::new(1.1_f32, col));
    for (k2, dot) in [(-2.6f32, k.accent_soft), (-0.9, k.icon_img), (0.8, k.icon_arc)].iter().enumerate() {
        let _ = k2;
        let a = dot.0 - 0.6;
        p.circle_filled(c + Vec2::new(a.cos(), a.sin()) * 4.0, 1.7, dot.1);
    }
    resp.clicked()
}

/// The sun / moon button: Nikkoid Blue (dark) <-> Nikkoid Bright, like Nikkomander.
pub(crate) fn theme_button(ui: &mut egui::Ui, p: &egui::Painter, rect: Rect) {
    let dark = skin::chosen().dark;
    let (resp, col) = icon_button(ui, rect, "theme");
    let resp = resp.on_hover_text(if dark { tip("Bright colours") } else { tip("Dark colours") });
    let c = rect.center();
    if dark {
        // sun
        p.circle_filled(c, 3.4, alpha(col, 70));
        p.circle_stroke(c, 3.4, Stroke::new(1.3_f32, col));
        for i in 0..8 {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            let d = Vec2::new(a.cos(), a.sin());
            p.line_segment([c + d * 5.6, c + d * 7.6], Stroke::new(1.3_f32, col));
        }
    } else {
        // moon: the crescent of Nikkomander's icon (two circles of radius 8 on a 24 grid)
        let s = 0.62;
        let o = c - Vec2::splat(12.0 * s);
        let at = |x: f32, y: f32| o + Vec2::new(x, y) * s;
        let (a, b) = (Vec2::new(20.0, 14.5), Vec2::new(9.5, 4.0));
        let m = (a + b) / 2.0;
        let half = (a - b).length() / 2.0;
        let h = (64.0 - half * half).max(0.0).sqrt();
        let perp = Vec2::new((a - b).y, -(a - b).x).normalized();
        let (body, bite) = (m - perp * h, m + perp * h);
        let (body, bite) = if (body - Vec2::splat(12.0)).length() < (bite - Vec2::splat(12.0)).length() { (body, bite) } else { (bite, body) };
        let ang = |cc: Vec2, q: Vec2| (q.y - cc.y).atan2(q.x - cc.x);
        let arc = |cc: Vec2, from: f32, to: f32, toward: Vec2| -> Vec<Pos2> {
            // the arc from `from` to `to` that passes nearest `toward`
            let mut to2 = to;
            while to2 < from {
                to2 += std::f32::consts::TAU;
            }
            let mid = (from + to2) / 2.0;
            let probe = cc + Vec2::new(mid.cos(), mid.sin()) * 8.0;
            let other = mid + std::f32::consts::PI;
            let probe2 = cc + Vec2::new(other.cos(), other.sin()) * 8.0;
            if (probe - toward).length() > (probe2 - toward).length() {
                to2 -= std::f32::consts::TAU;
            }
            (0..=16).map(|i| {
                let t = from + (to2 - from) * i as f32 / 16.0;
                at(cc.x + 8.0 * t.cos(), cc.y + 8.0 * t.sin())
            }).collect()
        };
        let away = body + (body - bite) * 2.0;
        let outer = arc(body, ang(body, a), ang(body, b), away);
        let inner = arc(bite, ang(bite, a), ang(bite, b), body);
        let mut mesh = Mesh::default();
        let fill = alpha(col, 55);
        for i in 0..outer.len() {
            mesh.colored_vertex(outer[i], fill);
            mesh.colored_vertex(inner[i], fill);
        }
        for i in 0..outer.len() as u32 - 1 {
            mesh.add_triangle(2 * i, 2 * i + 1, 2 * i + 2);
            mesh.add_triangle(2 * i + 1, 2 * i + 3, 2 * i + 2);
        }
        p.add(Shape::mesh(mesh));
        let mut line = outer;
        line.extend(inner.into_iter().rev());
        p.add(PathShape::closed_line(line, Stroke::new(1.3_f32, col)));
    }
    if resp.clicked() {
        skin::toggle_bright();
    }
}

/// State of the skins menu.
pub(crate) struct SkinMenu {
    pub open: bool,
    pub error: Option<String>,
    /// where the box has been dragged to, from its usual corner
    pub offset: Vec2,
    /// the skin being tried out, until Apply or Cancel
    pub selected: Option<String>,
    /// the box still has to be put beside the window, once the window is wide
    /// enough (NikPak widens itself while the box is open)
    pub beside: bool,
}

impl Default for SkinMenu {
    fn default() -> SkinMenu {
        SkinMenu { open: false, error: None, offset: Vec2::ZERO, selected: None, beside: false }
    }
}

impl SkinMenu {
    pub(crate) fn toggle(&mut self) {
        self.open = !self.open;
        self.error = None;
        self.beside = self.open;
        if self.open {
            self.offset = Vec2::ZERO;
            self.selected = None;
        } else {
            skin::preview(None);
        }
    }
}

/// The skins menu: every skin in its own colours. A click tries the skin on
/// the whole window, Apply keeps it (saved for NikPak and unNik) while Cancel
/// goes back to the previous one. The box is put beside the program's own
/// box, on the empty, transparent part of the window that NikPak makes room
/// for while it is open, and can be dragged anywhere in the window.
pub(crate) fn skin_menu(ctx: &egui::Context, parent: &eframe::Frame, panel: Rect, win: Rect, menu_state: &mut SkinMenu) {
    if !menu_state.open || panel == Rect::NOTHING {
        return;
    }
    let win = if win == Rect::NOTHING { panel } else { win };
    let w = (panel.width() - 32.0).min(560.0);
    let h = (panel.height() - 72.0).min(600.0);
    let base = Pos2::new(panel.right() - 16.0 - w, panel.top() + 56.0);
    // once the window has been widened, put the box beside the program: the
    // whole interface stays visible, and the box can still be dragged back
    // over it, or anywhere else in the window
    if menu_state.beside && win.width() > panel.width() + w + 24.0 {
        menu_state.offset = Vec2::new(panel.right() + 8.0 - base.x, 0.0);
        menu_state.beside = false;
    }
    // let the box be dragged anywhere in the window, but not out of it
    let min_x = win.left() + 8.0 - base.x;
    let max_x = (win.right() - 8.0 - w - base.x).max(min_x);
    let min_y = win.top() + 8.0 - base.y;
    let max_y = (win.bottom() - 8.0 - h - base.y).max(min_y);
    menu_state.offset.x = menu_state.offset.x.clamp(min_x, max_x);
    menu_state.offset.y = menu_state.offset.y.clamp(min_y, max_y);
    let menu = Rect::from_min_size(base + menu_state.offset, Vec2::new(w, h));
    let skins = skin::all();
    let chosen = skin::chosen().id.clone();
    let mut close = false;
    let mut applied = false;
    egui::Area::new(egui::Id::new("skinmenu"))
        .order(egui::Order::Foreground)
        // the area covers the whole window, not just the program's box: the
        // skins box may stand beside the program, on the transparent part
        .fixed_pos(win.min)
        .show(ctx, |ui| {
            ui.set_min_size(win.size());
            ui.set_clip_rect(win);
            let p = ui.painter().clone();
            // the program behind keeps still while a skin is being tried, but
            // clicking it no longer throws the box away: it stands beside the
            // window exactly so that the interface can be looked at
            let _blocker = ui.interact(panel, egui::Id::new("skin-blocker"), Sense::click());
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                close = true;
            }
            radial_glow(&p, menu.center(), menu.width(), alpha(ACCENT(), 30));
            radial_glow(&p, menu.center(), menu.width() * 0.8, alpha(Color32::BLACK, 70));
            gradient_panel(&p, menu, 14.0, skin::c().dialog_top, skin::c().dialog_bottom);
            p.rect_stroke(menu, rr(14), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 100)), egui::StrokeKind::Inside);
            // drag handle: the title bar moves the box aside, so the window
            // behind it shows the skin being tried
            let grip = Rect::from_min_size(menu.min, Vec2::new((menu.width() - 52.0).max(40.0), 40.0));
            let drag = ui.interact(grip, egui::Id::new("skin-drag"), Sense::drag());
            if drag.dragged() {
                menu_state.offset += drag.drag_delta();
            }
            if drag.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if drag.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
            for k in 0..3 {
                let gy = menu.top() + 15.0 + k as f32 * 5.0;
                p.line_segment([Pos2::new(menu.left() + 9.0, gy), Pos2::new(menu.left() + 17.0, gy)], Stroke::new(1.2_f32, alpha(TEXT_DIM(), 170)));
            }
            let lab = spaced(&p, t("SKINS"), FontId::new(10.0, family("ui")), TEXT_DIM(), 2.0);
            paint_label(&p, &lab, menu.left() + 24.0, menu.right() - 18.0, menu.top() + 15.0, TEXT_DIM());
            let x = Rect::from_center_size(Pos2::new(menu.right() - 22.0, menu.top() + 21.0), Vec2::splat(24.0));
            let (xr, col) = icon_button(ui, x, "skin-close");
            p.line_segment([x.center() + Vec2::new(-4.5, -4.5), x.center() + Vec2::new(4.5, 4.5)], Stroke::new(1.4_f32, col));
            p.line_segment([x.center() + Vec2::new(-4.5, 4.5), x.center() + Vec2::new(4.5, -4.5)], Stroke::new(1.4_f32, col));
            if xr.clicked() {
                close = true;
            }

            // ---- the skins, two or three per row ---------------------------------
            let grid = Rect::from_min_max(menu.min + Vec2::new(10.0, 40.0), menu.max - Vec2::new(10.0, 56.0));
            let cols = if grid.width() > 520.0 { 3 } else { 2 };
            let gap = 8.0;
            let tile_w = ((grid.width() - 14.0 - gap * (cols as f32 - 1.0)) / cols as f32).floor();
            let tile_h = 58.0;
            let rows = skins.len().div_ceil(cols);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(grid).layout(egui::Layout::top_down(egui::Align::Min)));
            child.set_clip_rect(grid);
            let mut remove: Option<String> = None;
            let mut pick: Option<String> = None;
            egui::ScrollArea::vertical().id_salt("skin-scroll").max_height(grid.height()).auto_shrink([false, false]).show(&mut child, |ui| {
                let (area, _) = ui.allocate_exact_size(Vec2::new(grid.width() - 14.0, rows as f32 * (tile_h + gap)), Sense::hover());
                let p = ui.painter().clone();
                for (i, s) in skins.iter().enumerate() {
                    let tile = Rect::from_min_size(
                        area.min + Vec2::new((i % cols) as f32 * (tile_w + gap), (i / cols) as f32 * (tile_h + gap)),
                        Vec2::new(tile_w, tile_h),
                    );
                    if !ui.is_rect_visible(tile) {
                        continue;
                    }
                    let resp = ui.interact(tile, egui::Id::new(("skin", &s.id)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let k = &s.c;
                    let rad_t = (10.0 * k.radius).min(14.0);
                    skin::paint_skin_background(ui.ctx(), &p, s, tile, rad_t);
                    // a small window of the skin: panel, accent button, text
                    let card = Rect::from_min_size(tile.min + Vec2::new(tile_w - 62.0, 12.0), Vec2::new(50.0, tile_h - 24.0));
                    gradient_panel_px(&p, card, 6.0 * k.radius, k.panel_top, k.panel_bottom);
                    p.rect_stroke(card, CornerRadius::same((6.0 * k.radius) as u8), Stroke::new(1.0_f32, alpha(k.border, 200)), egui::StrokeKind::Inside);
                    let btn = Rect::from_min_size(card.min + Vec2::new(8.0, card.height() - 14.0), Vec2::new(card.width() - 16.0, 7.0));
                    p.rect_filled(btn, CornerRadius::same((3.5 * k.radius) as u8), k.accent);
                    p.rect_filled(Rect::from_min_size(card.min + Vec2::new(8.0, 7.0), Vec2::new(26.0, 3.0)), 1.0, k.text);
                    p.rect_filled(Rect::from_min_size(card.min + Vec2::new(8.0, 13.0), Vec2::new(18.0, 3.0)), 1.0, k.text_dim);
                    for (j, dot) in [k.accent_soft, k.ok, k.err, k.warn].iter().enumerate() {
                        p.circle_filled(Pos2::new(tile.left() + 16.0 + j as f32 * 11.0, tile.bottom() - 14.0), 3.6, *dot);
                    }
                    // the name, on a veil of the skin's own background for contrast
                    let name_w = tile_w - 86.0;
                    let nt = lay_fit(&p, &s.name, FontId::new(12.5, family("ui")), name_w);
                    let np = Pos2::new(tile.left() + 11.0, tile.top() + 10.0);
                    p.rect_filled(Rect::from_min_size(np - Vec2::new(4.0, 2.0), nt.size() + Vec2::new(8.0, 4.0)), CornerRadius::same((5.0 * k.radius) as u8), alpha(k.bg, 150));
                    nt.paint(&p, np, k.text);
                    let sel = match menu_state.selected.as_deref() {
                        Some(id) => id == s.id,
                        None => s.id == chosen,
                    };
                    if sel || resp.hovered() {
                        p.rect_stroke(tile, CornerRadius::same(rad_t as u8), Stroke::new(if sel { 2.2_f32 } else { 1.4 }, if sel { ACCENT() } else { ACCENT_SOFT() }), egui::StrokeKind::Outside);
                    }
                    if sel {
                        let c = Pos2::new(card.left() - 13.0, tile.bottom() - 14.0);
                        p.circle_filled(c, 7.5, ACCENT());
                        p.add(PathShape::line(vec![c + Vec2::new(-3.2, 0.2), c + Vec2::new(-0.9, 2.5), c + Vec2::new(3.4, -2.4)], Stroke::new(1.5_f32, ACCENT_FG())));
                    }
                    if s.custom && resp.hovered() {
                        // remove an imported skin
                        let xb = Rect::from_center_size(Pos2::new(tile.right() - 9.0, tile.top() + 9.0), Vec2::splat(16.0));
                        let xr = ui.interact(xb, egui::Id::new(("skin-x", &s.id)), Sense::click()).on_hover_text(tip("Remove this skin"));
                        p.circle_filled(xb.center(), 7.5, if xr.hovered() { ERR_RED() } else { alpha(Color32::BLACK, 140) });
                        p.line_segment([xb.center() + Vec2::new(-3.0, -3.0), xb.center() + Vec2::new(3.0, 3.0)], Stroke::new(1.3_f32, Color32::WHITE));
                        p.line_segment([xb.center() + Vec2::new(-3.0, 3.0), xb.center() + Vec2::new(3.0, -3.0)], Stroke::new(1.3_f32, Color32::WHITE));
                        if xr.clicked() {
                            remove = Some(s.id.clone());
                            continue;
                        }
                    }
                    if resp.clicked() {
                        sfx::play(Sfx::Click);
                        pick = Some(s.id.clone());
                    }
                }
            });
            if let Some(id) = remove {
                skin::remove(&id);
                if menu_state.selected.as_deref() == Some(id.as_str()) {
                    menu_state.selected = None;
                }
            } else if let Some(id) = pick {
                skin::preview(Some(&id));
                menu_state.selected = Some(id);
            }

            // ---- import a Nikkomander skin file -----------------------------------
            let ib = Rect::from_min_size(Pos2::new(menu.left() + 16.0, menu.bottom() - 44.0), Vec2::new(160.0, 32.0));
            if pill_button(ui, &p, ib, t("Import a skin…"), false, "skin-import") {
                let f = rfd::FileDialog::new().set_parent(parent).set_title(t("Import a skin…")).add_filter(t("Skin files"), &["json"]).pick_file();
                if let Some(f) = f {
                    match skin::import(&f) {
                        Ok(_) => close = true,
                        Err(e) => menu_state.error = Some(tf("Could not import the skin: {}", &[&e])),
                    }
                }
            }
            // ---- Apply / Cancel ---------------------------------------------------
            let apply_r = Rect::from_min_size(Pos2::new(menu.right() - 16.0 - 118.0, menu.bottom() - 44.0), Vec2::new(118.0, 32.0));
            let cancel_r = Rect::from_min_size(Pos2::new(apply_r.left() - 8.0 - 82.0, apply_r.top()), Vec2::new(82.0, 32.0));
            if pill_button(ui, &p, cancel_r, t("Cancel"), false, "skin-cancel") {
                close = true;
            }
            if pill_button(ui, &p, apply_r, t("Apply Skin"), true, "skin-apply") {
                if let Some(id) = menu_state.selected.clone() {
                    skin::apply(&id);
                }
                applied = true;
                close = true;
            }
            let note_x = ib.right() + 12.0;
            let note_w = (cancel_r.left() - 10.0 - note_x).max(40.0);
            match &menu_state.error {
                Some(e) => {
                    text_fit(&p, Pos2::new(note_x, ib.center().y), Align2::LEFT_CENTER, e, FontId::new(11.5, family("ui-light")), ERR_RED(), note_w);
                }
                None => {
                    text_fit(&p, Pos2::new(note_x, ib.center().y), Align2::LEFT_CENTER, tf("{} skins", &[&skins.len().to_string()]), FontId::new(11.5, family("ui-light")), TEXT_DIM(), note_w);
                }
            }
        });
    if close {
        menu_state.open = false;
        menu_state.error = None;
        menu_state.selected = None;
        if !applied {
            // Cancel, the × or Esc: back to the skin we had
            skin::preview(None);
        }
    }
}

/// The language button: flag + native name + a small triangle. Returns true when clicked.
pub(crate) fn lang_button(ui: &mut egui::Ui, p: &egui::Painter, lb: Rect, open: bool, busy: bool) -> bool {
    let cur = i18n::current();
    let lresp = ui.interact(lb, egui::Id::new("lang"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let hov = (lresp.hovered() || open) && !busy;
    p.rect_filled(lb, rr(15), alpha(ACCENT(), if hov { 46 } else { 16 }));
    p.rect_stroke(lb, rr(15), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if hov { 150 } else { 60 })), egui::StrokeKind::Inside);
    let fr = Rect::from_min_size(Pos2::new(lb.left() + 11.0, lb.center().y - 7.0), Vec2::new(21.0, 14.0));
    draw_flag(p, fr, cur);
    let name_w = lb.right() - 22.0 - (fr.right() + 8.0);
    let nt = lay_fit(p, cur.native_name(), FontId::new(12.5, family("ui-light")), name_w);
    nt.paint(p, Pos2::new(fr.right() + 8.0, lb.center().y - nt.size().y / 2.0), if busy { TEXT_DIM() } else { TEXT() });
    let cc = Pos2::new(lb.right() - 13.0, lb.center().y + 1.0);
    p.add(Shape::convex_polygon(
        vec![cc + Vec2::new(-4.0, -2.5), cc + Vec2::new(4.0, -2.5), cc + Vec2::new(0.0, 2.5)],
        if busy { TEXT_DIM() } else { ACCENT_SOFT() },
        Stroke::NONE,
    ));
    if lresp.clicked() && !busy {
        sfx::play(if open { Sfx::Click } else { Sfx::Pop });
    }
    lresp.clicked() && !busy
}

/// The three round community buttons; `centers` gives their positions.
pub(crate) fn social_buttons(ui: &mut egui::Ui, p: &egui::Painter, centers: [Pos2; 3], radius: f32) {
    for (k, (name, url)) in SOCIAL.iter().enumerate() {
        let c = centers[k];
        let hit = Rect::from_center_size(c, Vec2::splat(radius * 2.3));
        let resp = ui
            .interact(hit, egui::Id::new(("social", k)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text(format!("{name}: {url}"));
        let hov = resp.hovered() || std::env::var("NIKPAK_GUI_ICON_HOVER").map_or(false, |v| v == *name);
        brand_icon(p, name, c, if hov { radius * 1.1 } else { radius }, hov);
        if resp.clicked() {
            sfx::play(Sfx::Click);
            open_link(url);
        }
    }
}

/// Close (×) / minimise (−) / about (i) buttons at the top right, right to left
/// from `close`. Returns true when the "i" was clicked.
pub(crate) fn window_buttons(ui: &mut egui::Ui, p: &egui::Painter, ctx: &egui::Context, close: Rect, about_tip: &'static str) -> bool {
    let (cr, col) = icon_button(ui, close, "close");
    let (c, d) = (close.center(), 5.0);
    p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], Stroke::new(1.4_f32, col));
    p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], Stroke::new(1.4_f32, col));
    if cr.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let mini = close.translate(Vec2::new(-32.0, 0.0));
    let (mr, col) = icon_button(ui, mini, "minimise");
    p.line_segment([mini.center() + Vec2::new(-d, 0.0), mini.center() + Vec2::new(d, 0.0)], Stroke::new(1.4_f32, col));
    if mr.clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
    let info = mini.translate(Vec2::new(-32.0, 0.0));
    let (ir, col) = icon_button(ui, info, "about");
    let ir = ir.on_hover_text(tip(about_tip));
    p.circle_stroke(info.center(), 7.5, Stroke::new(1.2_f32, col));
    text(p, info.center() + Vec2::new(0.0, 0.5), Align2::CENTER_CENTER, "i", FontId::new(11.0, family("ui")), col);
    ir.clicked()
}

/// The glowing logo with its fading line underneath.
pub(crate) fn logo(p: &egui::Painter, c: Pos2, name: &str, size: f32) -> Rect {
    let lr = glow_text(p, c, name, FontId::new(size, family("display")), TEXT(), alpha(ACCENT(), 6), size / 7.5);
    let ly = lr.bottom() + 8.0 * size / 30.0;
    let half = 70.0 * size / 30.0;
    for k in 0..40 {
        let x0 = -half + 2.0 * half * k as f32 / 40.0;
        let x1 = -half + 2.0 * half * (k + 1) as f32 / 40.0;
        let a = 1.0 - ((x0 + x1) / 2.0 / half).abs();
        let pa = Pos2::new(c.x + x0, ly);
        let pb = Pos2::new(c.x + x1, ly);
        p.line_segment([pa, pb], Stroke::new(3.0_f32, alpha(ACCENT(), (a * 30.0) as u8)));
        p.line_segment([pa, pb], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), (a * 200.0) as u8)));
    }
    lr
}

/// The × at the top right of a card; true when clicked (or Esc).
pub(crate) fn card_close(ui: &mut egui::Ui, p: &egui::Painter, c: Rect, id: &str) -> bool {
    let x = Rect::from_center_size(Pos2::new(c.right() - 26.0, c.top() + 26.0), Vec2::splat(26.0));
    let (xr, col) = icon_button(ui, x, id);
    p.line_segment([x.center() + Vec2::new(-5.0, -5.0), x.center() + Vec2::new(5.0, 5.0)], Stroke::new(1.4_f32, col));
    p.line_segment([x.center() + Vec2::new(-5.0, 5.0), x.center() + Vec2::new(5.0, -5.0)], Stroke::new(1.4_f32, col));
    xr.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape))
}

impl App {
    fn draw_modal(&mut self, ctx: &egui::Context, panel: Rect) {
        if matches!(self.modal, Modal::None) || panel == Rect::NOTHING {
            return;
        }
        egui::Area::new(egui::Id::new("modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(panel.min)
            .show(ctx, |ui| {
                ui.set_min_size(panel.size());
                let p = ui.painter().clone();
                // dim and block everything underneath
                let _ = ui.interact(panel, egui::Id::new("modal-blocker"), Sense::click_and_drag());
                p.rect_filled(panel, rr(RADIUS as u8), alpha(skin::c().overlay, 205));
                let cx = panel.center().x;
                let mut next: Option<Modal> = None;
                match &mut self.modal {
                    Modal::None => {}
                    Modal::Nag { register_left } => {
                        let tw = 384.0 - 60.0;
                        let need = 104.0
                            + wrapped_height(&p, tw, t("NikPak is shareware. You may test it for free, without time limit, for personal use."), 14.0)
                            + 12.0
                            + wrapped_height(&p, tw, t("Companies and organisations need one licence per seat."), 14.0)
                            + 16.0 + 74.0
                            + wrapped_height(&p, tw, t("Registering removes this reminder."), 12.5)
                            + 16.0 + 70.0;
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(384.0, need.clamp(404.0, H - 20.0)));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("SHAREWARE"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 70.0), t("Evaluation copy"), FontId::new(24.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.0);
                        let mut y = c.top() + 104.0;
                        y += wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0,
                            t("NikPak is shareware. You may test it for free, without time limit, for personal use."), 14.0, TEXT());
                        y += 12.0;
                        y += wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0,
                            t("Companies and organisations need one licence per seat."), 14.0, TEXT());
                        y += 16.0;
                        // prices
                        for (k, (label, price)) in [(t("Personal"), license::PRICE_PERSONAL), (t("Company"), t("from $5 / seat"))].iter().enumerate() {
                            let bx = Rect::from_min_size(Pos2::new(c.left() + 30.0 + k as f32 * 168.0, y), Vec2::new(156.0, 58.0));
                            p.rect_filled(bx, rr(12), alpha(skin::c().field, 160));
                            p.rect_stroke(bx, rr(12), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 50)), egui::StrokeKind::Inside);
                            text_fit(&p, Pos2::new(bx.center().x, bx.top() + 18.0), Align2::CENTER_CENTER, *label, FontId::new(11.5, family("ui")), TEXT_DIM(), bx.width() - 14.0);
                            text_fit(&p, Pos2::new(bx.center().x, bx.top() + 39.0), Align2::CENTER_CENTER, *price, FontId::new(19.0, family("display")), TEXT(), bx.width() - 14.0);
                        }
                        y += 74.0;
                        wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0, t("Registering removes this reminder."), 12.5, TEXT_DIM());
                        // the two buttons swap sides at random
                        let left = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(156.0, 44.0));
                        let right = left.translate(Vec2::new(168.0, 0.0));
                        let (reg_r, test_r) = if *register_left { (left, right) } else { (right, left) };
                        if pill_button(ui, &p, reg_r, t("Register"), true, "nag-register") {
                            next = Some(Modal::Register { name: String::new(), email: String::new(), key: String::new(), error: None, done: None });
                        }
                        if pill_button(ui, &p, test_r, t("Test for free"), false, "nag-test") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(Modal::None);
                        }
                    }
                    Modal::About => {
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(372.0, 380.0));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("ABOUT"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 76.0), "NikPak", FontId::new(32.0, family("display")), TEXT(), alpha(ACCENT(), 8), 1.5);
                        text(&p, Pos2::new(cx, c.top() + 110.0), Align2::CENTER_CENTER, tf("Version {}", &[crate::cli::VERSION]), FontId::new(13.0, family("ui-light")), TEXT_DIM());
                        p.line_segment([Pos2::new(cx - 60.0, c.top() + 132.0), Pos2::new(cx + 60.0, c.top() + 132.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 60)));
                        text(&p, Pos2::new(cx, c.top() + 158.0), Align2::CENTER_CENTER, COPYRIGHT, FontId::new(15.0, family("ui")), TEXT());
                        text(&p, Pos2::new(cx, c.top() + 182.0), Align2::CENTER_CENTER, t(DISTRIBUTOR), FontId::new(13.0, family("ui-light")), TEXT_DIM());
                        let status = match &self.license {
                            Some(l) => tf("Registered to {}.", &[&describe(l)]),
                            None => t("Evaluation copy: free for personal use. Companies need one licence per seat.").to_string(),
                        };
                        let col = if self.license.is_some() { OK_GREEN() } else { TEXT() };
                        let sh = wrapped(&p, Pos2::new(c.left() + 30.0, c.top() + 214.0), c.width() - 60.0, &status, 13.5, col);
                        // web site link
                        let site = "nikpak.nikkoid.com";
                        let link_c = Pos2::new(cx, c.top() + 232.0 + sh);
                        let link_r = Rect::from_center_size(link_c, Vec2::new(150.0, 22.0));
                        let lr = ui.interact(link_r, egui::Id::new("about-site"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        let lt = text(&p, link_c, Align2::CENTER_CENTER, site, FontId::new(13.0, family("ui")), if lr.hovered() { TEXT() } else { ACCENT_SOFT() });
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
                        if pill_button(ui, &p, ok, t("Close"), true, "about-close") {
                            next = Some(Modal::None);
                        }
                        let x = Rect::from_center_size(Pos2::new(c.right() - 26.0, c.top() + 26.0), Vec2::splat(26.0));
                        let (xr, col) = icon_button(ui, x, "about-x");
                        p.line_segment([x.center() + Vec2::new(-5.0, -5.0), x.center() + Vec2::new(5.0, 5.0)], Stroke::new(1.4_f32, col));
                        p.line_segment([x.center() + Vec2::new(-5.0, 5.0), x.center() + Vec2::new(5.0, -5.0)], Stroke::new(1.4_f32, col));
                        if xr.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(Modal::None);
                        }
                    }
                    Modal::Update(info) => {
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(372.0, 300.0));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 32.0), t("UPDATE"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 70.0), &tf("NikPak {} is available", &[info.latest.as_str()]), FontId::new(21.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.0);
                        p.line_segment([Pos2::new(cx - 60.0, c.top() + 92.0), Pos2::new(cx + 60.0, c.top() + 92.0)], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 60)));
                        let mut y = c.top() + 108.0;
                        y += wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0,
                            &tf("You have {}. The download page has the new version.", &[info.current.as_str()]), 13.5, TEXT());
                        if !info.notes.is_empty() {
                            y += 8.0;
                            wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0, &info.notes, 12.0, TEXT_DIM());
                        }
                        // Cancel (or Esc, or the cross) closes it; Download opens the site
                        let no = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 66.0), Vec2::new(140.0, 44.0));
                        let yes = no.translate(Vec2::new(160.0, 0.0));
                        if pill_button(ui, &p, no, t("Cancel"), false, "upd-cancel") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(Modal::None);
                        }
                        if pill_button(ui, &p, yes, t("Download"), true, "upd-download") {
                            open_link(&info.url);
                            next = Some(Modal::None);
                        }
                        let x = Rect::from_center_size(Pos2::new(c.right() - 26.0, c.top() + 26.0), Vec2::splat(26.0));
                        let (xr, col) = icon_button(ui, x, "upd-x");
                        p.line_segment([x.center() + Vec2::new(-5.0, -5.0), x.center() + Vec2::new(5.0, 5.0)], Stroke::new(1.4_f32, col));
                        p.line_segment([x.center() + Vec2::new(-5.0, 5.0), x.center() + Vec2::new(5.0, -5.0)], Stroke::new(1.4_f32, col));
                        if xr.clicked() {
                            next = Some(Modal::None);
                        }
                    }
                    Modal::Cover { pic, on, error, tex } => {
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(420.0, (H - 24.0).min(500.0)));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 32.0), t("COVER"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 64.0), t("A picture for your archives"), FontId::new(21.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.0);
                        let mut y = c.top() + 90.0;
                        y += wrapped(&p, Pos2::new(c.left() + 28.0, y), c.width() - 56.0,
                            t("Any picture is cropped square and saved at 1024 × 1024. Click the frame to choose one, or drop it on the window."), 12.5, TEXT_DIM());
                        y += 10.0;
                        if pic.is_some() && tex.is_none() {
                            *tex = pic.as_deref().and_then(|b| cover_texture(ctx, "cover-edit", b));
                        }
                        let side = (c.bottom() - 138.0 - y).clamp(120.0, 220.0);
                        let frame = Rect::from_min_size(Pos2::new(cx - side / 2.0, y), Vec2::splat(side));
                        paint_cover(&p, frame, tex.as_ref(), t("Click to choose a picture"));
                        let fr = ui.interact(frame, egui::Id::new("cover-frame"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        if fr.clicked() {
                            let chosen = rfd::FileDialog::new()
                                .set_title(t("Choose a cover picture"))
                                .add_filter(t("Pictures"), &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
                                .pick_file();
                            if let Some(f) = chosen {
                                match crate::cover::from_file(&f) {
                                    Ok(b) => {
                                        *pic = Some(b);
                                        *tex = None;
                                        *error = None;
                                        *on = true;
                                    }
                                    Err(e) => *error = Some(e),
                                }
                            }
                        }
                        let bottom = frame.bottom();
                        let sw = Rect::from_min_size(Pos2::new(c.left() + 28.0, bottom + 14.0), Vec2::new(34.0, 18.0));
                        let swr = ui.interact(sw.union(Rect::from_min_size(sw.right_top(), Vec2::new(c.width() - 190.0, 18.0))), egui::Id::new("cover-on"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if swr.clicked() {
                            *on = !*on;
                        }
                        p.rect_filled(sw, rr(9), if *on { skin::c().accent_btn() } else { alpha(skin::c().field, 220) });
                        let knob = if *on { Pos2::new(sw.right() - 9.0, sw.center().y) } else { Pos2::new(sw.left() + 9.0, sw.center().y) };
                        p.circle_filled(knob, 6.5, if *on { ACCENT_FG() } else { TEXT_DIM() });
                        text_fit(&p, Pos2::new(sw.right() + 10.0, sw.center().y), Align2::LEFT_CENTER, t("Attach to new archives"), FontId::new(13.0, family("ui")), TEXT(), c.width() - 200.0);
                        if pic.is_some() {
                            let rm = Rect::from_min_size(Pos2::new(c.right() - 28.0 - 96.0, bottom + 8.0), Vec2::new(96.0, 30.0));
                            if pill_button(ui, &p, rm, t("Remove"), false, "cover-remove") {
                                *pic = None;
                                *tex = None;
                            }
                        }
                        if let Some(e) = error.as_ref() {
                            text_fit(&p, Pos2::new(cx, bottom + 52.0), Align2::CENTER_CENTER, e.as_str(), FontId::new(12.5, family("ui")), ERR_RED(), c.width() - 56.0);
                        }
                        let no = Rect::from_min_size(Pos2::new(c.left() + 28.0, c.bottom() - 66.0), Vec2::new(170.0, 44.0));
                        let yes = Rect::from_min_size(Pos2::new(c.right() - 28.0 - 170.0, c.bottom() - 66.0), Vec2::new(170.0, 44.0));
                        if pill_button(ui, &p, no, t("Cancel"), false, "cover-cancel") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(Modal::None);
                        }
                        if pill_button(ui, &p, yes, t("Save"), true, "cover-save") {
                            if *on && pic.is_none() {
                                *error = Some(t("Choose a picture first.").to_string());
                            } else {
                                save_cover(*on, pic.as_deref());
                                self.cover_on = *on && pic.is_some();
                                self.cover = pic.clone();
                                sfx::play(Sfx::Click);
                                next = Some(Modal::None);
                            }
                        }
                    }
                    Modal::Legal { text: terms, on, error } => {
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(420.0, (H - 24.0).min(500.0)));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 32.0), t("LEGAL STATEMENT"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 64.0), t("Terms for your archives"), FontId::new(21.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.0);
                        let mut y = c.top() + 90.0;
                        y += wrapped(&p, Pos2::new(c.left() + 28.0, y), c.width() - 56.0,
                            t("Whoever opens an archive made with these terms must read them and click I approve before anything is listed or extracted."), 12.5, TEXT_DIM());
                        y += 10.0;
                        // the terms: a scrolling multi-line field
                        let bottom = c.bottom() - 128.0;
                        let f = Rect::from_min_max(Pos2::new(c.left() + 28.0, y), Pos2::new(c.right() - 28.0, bottom));
                        p.rect_filled(f, rr(10), alpha(skin::c().field, 190));
                        p.rect_stroke(f, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 70)), egui::StrokeKind::Inside);
                        ui.scope_builder(egui::UiBuilder::new().max_rect(f.shrink(4.0)), |ui| {
                            egui::ScrollArea::vertical().id_salt("legal-scroll").auto_shrink([false, false]).show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(terms)
                                        .id(egui::Id::new("legal-text"))
                                        .font(FontId::new(12.5, family("ui-light")))
                                        .text_color(TEXT())
                                        .frame(false)
                                        .desired_width(f.width() - 24.0)
                                        .desired_rows(8)
                                        .margin(egui::Margin::symmetric(8, 6)),
                                );
                            });
                        });
                        // "Attach to new archives": a round switch with its label
                        let sw = Rect::from_min_size(Pos2::new(c.left() + 28.0, bottom + 14.0), Vec2::new(34.0, 18.0));
                        let swr = ui.interact(sw.union(Rect::from_min_size(sw.right_top(), Vec2::new(c.width() - 100.0, 18.0))), egui::Id::new("legal-on"), Sense::click())
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if swr.clicked() {
                            *on = !*on;
                        }
                        p.rect_filled(sw, rr(9), if *on { skin::c().accent_btn() } else { alpha(skin::c().field, 220) });
                        let knob = if *on { Pos2::new(sw.right() - 9.0, sw.center().y) } else { Pos2::new(sw.left() + 9.0, sw.center().y) };
                        p.circle_filled(knob, 6.5, if *on { ACCENT_FG() } else { TEXT_DIM() });
                        text_fit(&p, Pos2::new(sw.right() + 10.0, sw.center().y), Align2::LEFT_CENTER, t("Attach to new archives"), FontId::new(13.0, family("ui")), TEXT(), c.width() - 110.0);
                        if let Some(e) = error.as_ref() {
                            text_fit(&p, Pos2::new(cx, bottom + 48.0), Align2::CENTER_CENTER, e.as_str(), FontId::new(12.5, family("ui")), ERR_RED(), c.width() - 56.0);
                        }
                        let no = Rect::from_min_size(Pos2::new(c.left() + 28.0, c.bottom() - 66.0), Vec2::new(170.0, 44.0));
                        let yes = Rect::from_min_size(Pos2::new(c.right() - 28.0 - 170.0, c.bottom() - 66.0), Vec2::new(170.0, 44.0));
                        if pill_button(ui, &p, no, t("Cancel"), false, "legal-cancel") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            next = Some(Modal::None);
                        }
                        if pill_button(ui, &p, yes, t("Save"), true, "legal-save") {
                            if *on && terms.trim().is_empty() {
                                *error = Some(t("Write the terms first.").to_string());
                            } else {
                                save_legal(*on, terms);
                                self.legal_on = *on;
                                self.legal_text = terms.clone();
                                sfx::play(Sfx::Click);
                                next = Some(Modal::None);
                            }
                        }
                    }
                    Modal::Register { name, email, key, error, done } => {
                        let c = Rect::from_center_size(Pos2::new(cx, panel.center().y), Vec2::new(392.0, 516.0));
                        card(&p, c);
                        glow_text(&p, Pos2::new(cx, c.top() + 34.0), t("REGISTRATION"), FontId::new(11.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                        glow_text(&p, Pos2::new(cx, c.top() + 68.0), t("Register NikPak"), FontId::new(24.0, family("display")), TEXT(), alpha(ACCENT(), 6), 1.0);
                        if let Some(msg) = done.clone() {
                            p.circle_stroke(Pos2::new(cx, c.top() + 170.0), 34.0, Stroke::new(3.0_f32, OK_GREEN()));
                            p.add(PathShape::line(
                                vec![Pos2::new(cx - 15.0, c.top() + 170.0), Pos2::new(cx - 4.0, c.top() + 181.0), Pos2::new(cx + 16.0, c.top() + 158.0)],
                                Stroke::new(3.0_f32, OK_GREEN()),
                            ));
                            wrapped(&p, Pos2::new(c.left() + 30.0, c.top() + 230.0), c.width() - 60.0, &msg, 15.0, TEXT());
                            let b = Rect::from_center_size(Pos2::new(cx, c.bottom() - 48.0), Vec2::new(180.0, 44.0));
                            if pill_button(ui, &p, b, t("Continue"), true, "reg-continue") {
                                next = Some(Modal::None);
                            }
                        } else {
                            let mut y = c.top() + 98.0;
                            y += wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0,
                                t("Enter your name and e-mail address exactly as on your licence, and paste the key."), 13.0, TEXT_DIM());
                            y += 12.0;
                            let field_w = c.width() - 60.0;
                            for (label, value, rows, id) in [(t("NAME"), &mut *name, 1usize, "f-name"), (t("E-MAIL"), &mut *email, 1, "f-email"), (t("LICENCE KEY"), &mut *key, 4, "f-key")] {
                                let lab = spaced(&p, label, FontId::new(10.0, family("ui")), TEXT_DIM(), 2.0);
                                paint_label(&p, &lab, c.left() + 32.0, c.right() - 32.0, y, TEXT_DIM());
                                y += 17.0;
                                let h = if rows == 1 { 34.0 } else { 74.0 };
                                let f = Rect::from_min_size(Pos2::new(c.left() + 30.0, y), Vec2::new(field_w, h));
                                p.rect_filled(f, rr(10), alpha(skin::c().field, 190));
                                p.rect_stroke(f, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 70)), egui::StrokeKind::Inside);
                                let edit = if rows == 1 {
                                    egui::TextEdit::singleline(value).vertical_align(egui::Align::Center)
                                } else {
                                    egui::TextEdit::multiline(value).desired_rows(3)
                                }
                                .id(egui::Id::new(id))
                                .font(FontId::new(13.0, family(if rows == 1 { "ui-light" } else { "ui" })))
                                .text_color(TEXT())
                                .frame(false)
                                .margin(egui::Margin::symmetric(12, if rows == 1 { 0 } else { 8 }));
                                ui.put(f, edit);
                                y += h + 12.0;
                            }
                            if let Some(e) = error.as_ref() {
                                wrapped(&p, Pos2::new(c.left() + 30.0, y), c.width() - 60.0, e, 12.5, ERR_RED());
                            }
                            let buy = Rect::from_min_size(Pos2::new(c.left() + 30.0, c.bottom() - 70.0), Vec2::new(160.0, 44.0));
                            let reg = buy.translate(Vec2::new(172.0, 0.0));
                            if pill_button(ui, &p, buy, t("Buy a licence"), false, "reg-buy") {
                                open_link(license::BUY_URL);
                            }
                            if pill_button(ui, &p, reg, t("Register"), true, "reg-ok") {
                                match license::register(name, email, key) {
                                    Ok(l) => {
                                        sfx::play(Sfx::Success);
                                        *done = Some(tf("Thank you! NikPak is registered to {}.", &[&describe(&l)]));
                                        self.license = Some(l);
                                    }
                                    Err(e) => {
                                        sfx::play(Sfx::Error);
                                        *error = Some(i18n::t_dyn(&e));
                                    }
                                }
                            }
                            // close (×) at the top right of the card
                            let x = Rect::from_center_size(Pos2::new(c.right() - 26.0, c.top() + 26.0), Vec2::splat(26.0));
                            let (xr, col) = icon_button(ui, x, "reg-close");
                            p.line_segment([x.center() + Vec2::new(-5.0, -5.0), x.center() + Vec2::new(5.0, 5.0)], Stroke::new(1.4_f32, col));
                            p.line_segment([x.center() + Vec2::new(-5.0, 5.0), x.center() + Vec2::new(5.0, -5.0)], Stroke::new(1.4_f32, col));
                            if xr.clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                next = Some(Modal::None);
                            }
                        }
                    }
                }
                if let Some(m) = next {
                    self.modal = m;
                }
            });
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

impl eframe::App for App {
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Stage::Working(job) = std::mem::replace(&mut self.stage, Stage::Idle) {
            PROGRESS.cancel.store(true, Relaxed);
            let _ = job.handle.join(); // removes the unfinished temporary archive
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let parent: &eframe::Frame = frame;
        follow_language(ctx);
        skin::sync_style(ctx);
        self.poll_job();

        // ---- the launch update check -------------------------------------
        // The answer arrives from a thread (src/update.rs); it is shown once,
        // and only when nothing else is open, so it never cuts off the
        // shareware reminder or the drop animation.
        if !self.update_shown {
            if let Some(rx) = self.update_rx.take() {
                match rx.try_recv() {
                    Ok(info) => {
                        if info.newer && matches!(self.modal, Modal::None) {
                            self.modal = Modal::Update(info);
                            self.update_shown = true;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        self.update_rx = Some(rx);
                        ctx.request_repaint_after(std::time::Duration::from_millis(250));
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {}
                }
            }
        }

        // ---- drag and drop -----------------------------------------------
        let (dropped, hovering) = ctx.input(|i| {
            (
                i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<_>>(),
                !i.raw.hovered_files.is_empty() || std::env::var_os("NIKPAK_GUI_HOVER").is_some(),
            )
        });
        let files_over = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if files_over && !self.was_hover {
            sfx::play(Sfx::Hover);
        }
        self.was_hover = files_over;
        if let Modal::Cover { pic, tex, error, on } = &mut self.modal {
            // the cover box is open: a dropped picture becomes the cover
            if let Some(f) = dropped.iter().find(|f| is_picture(f)) {
                match crate::cover::from_file(f) {
                    Ok(b) => {
                        *pic = Some(b);
                        *tex = None;
                        *error = None;
                        *on = true;
                        sfx::play(Sfx::Click);
                    }
                    Err(e) => *error = Some(e),
                }
            }
        } else if !dropped.is_empty() && matches!(self.modal, Modal::None) {
            self.chooser = false;
            self.start_job(dropped);
        }

        // ---- room for the skins box beside the program --------------------
        // The box is drawn inside the window, so to stand outside the program
        // the window itself grows by SKIN_SLOT while the box is open; that
        // extra part is left transparent and the program keeps its size.
        self.fit_skins_window(ctx);

        let now = self.start.elapsed().as_secs_f32();
        let mut panel_rect = Rect::NOTHING;
        let mut window_rect = Rect::NOTHING;
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| {
            let full = ui.max_rect();
            window_rect = full;
            let p = ui.painter().clone();
            // the program keeps its 500 x 560 box at the top left of the
            // window, even when the window is wider for the skins box
            let panel = Rect::from_min_size(full.min, Vec2::new(W, H)).shrink(1.0);

            // ---- background ----------------------------------------------------
            skin::paint_background(ui.ctx(), &p, panel, rad(RADIUS));
            let ring_c = Pos2::new(panel.center().x, panel.top() + 208.0);
            radial_glow(&p, ring_c, 200.0, alpha(ACCENT(), 34));
            radial_glow(&p, Pos2::new(panel.center().x, panel.top() - 40.0), 260.0, alpha(ACCENT_SOFT(), 18));
            p.rect_stroke(panel, rr(RADIUS as u8), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 46)), egui::StrokeKind::Inside);

            // ---- title bar: drag to move, minimise and close ---------------------
            // while the skins box is open the window is wider than the program:
            // the empty part beside it moves the window too (the box has its
            // own title bar, and it is drawn in a layer above this one)
            let bar_w = if self.skins.open { full.width() } else { panel.width() };
            let bar = Rect::from_min_size(panel.min, Vec2::new(bar_w, 110.0));
            let drag = ui.interact(bar, egui::Id::new("titlebar"), Sense::click_and_drag());
            if drag.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
            let close = Rect::from_center_size(Pos2::new(panel.right() - 26.0, panel.top() + 26.0), Vec2::splat(26.0));
            let (cr, col) = icon_button(ui, close, "close");
            let (c, d) = (close.center(), 5.0);
            p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], Stroke::new(1.4_f32, col));
            p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], Stroke::new(1.4_f32, col));
            if cr.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            let mini = close.translate(Vec2::new(-32.0, 0.0));
            let (mr, col) = icon_button(ui, mini, "minimise");
            p.line_segment([mini.center() + Vec2::new(-d, 0.0), mini.center() + Vec2::new(d, 0.0)], Stroke::new(1.4_f32, col));
            if mr.clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            // small "i" button: About (version, copyright, licence)
            let info = mini.translate(Vec2::new(-32.0, 0.0));
            let (ir, col) = icon_button(ui, info, "about");
            let ir = ir.on_hover_text(tip("About NikPak"));
            p.circle_stroke(info.center(), 7.5, Stroke::new(1.2_f32, col));
            text(&p, info.center() + Vec2::new(0.0, 0.5), Align2::CENTER_CENTER, "i", FontId::new(11.0, family("ui")), col);
            if ir.clicked() && matches!(self.modal, Modal::None) {
                self.modal = Modal::About;
            }
            // switch to unNik (not while compressing), and the sounds on / off
            let sw = info.translate(Vec2::new(-32.0, 0.0));
            sound_button(ui, &p, sw.translate(Vec2::new(-32.0, 0.0)));
            theme_button(ui, &p, sw.translate(Vec2::new(-64.0, 0.0)));
            if skin_button(ui, &p, sw.translate(Vec2::new(-96.0, 0.0)), self.skins.open) && matches!(self.modal, Modal::None) {
                self.skins.toggle();
                self.lang_open = false;
            }
            let busy_now = matches!(self.stage, Stage::Working(_));
            if switch_button(ui, &p, sw, true, !busy_now).clicked() && !busy_now && matches!(self.modal, Modal::None) {
                // after a compression, unNik opens the archive just made
                let args = match &self.stage {
                    Stage::Done { out, .. } if out.is_file() => vec![out.display().to_string()],
                    _ => Vec::new(),
                };
                if let Err(e) = switch_to(ctx, "unNik", &args) {
                    self.stage = Stage::Failed(e);
                }
            }
            // ---- licence status, top left -------------------------------------------
            let st_pos = Pos2::new(panel.left() + 22.0, panel.top() + 19.0);
            match &self.license {
                Some(l) => {
                    let g = spaced_fit(&p, &tf("REGISTERED TO {}", &[&l.name.to_uppercase()]), FontId::new(9.5, family("ui")), TEXT_DIM(), 1.2, 270.0);
                    g.paint(&p, label_pos(&g, st_pos), TEXT_DIM());
                }
                None => {
                    let g = spaced_fit(&p, t("EVALUATION COPY  ·  REGISTER"), FontId::new(9.5, family("ui")), ACCENT_SOFT(), 1.2, 232.0);
                    let rect = Rect::from_min_size(label_pos(&g, st_pos), g.size()).expand(4.0);
                    let resp = ui.interact(rect, egui::Id::new("evalreg"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    g.paint(&p, label_pos(&g, st_pos), if resp.hovered() { TEXT() } else { ACCENT_SOFT() });
                    if resp.clicked() {
                        sfx::play(Sfx::Pop);
                        self.modal = Modal::Register { name: String::new(), email: String::new(), key: String::new(), error: None, done: None };
                    }
                }
            }

            // ---- logo ------------------------------------------------------------
            let logo_c = Pos2::new(panel.center().x, panel.top() + 56.0);
            let lr = glow_text(&p, logo_c, "NikPak", FontId::new(30.0, family("display")), TEXT(), alpha(ACCENT(), 6), 4.0);
            // thin glowing line under the logo, fading at both ends
            let ly = lr.bottom() + 8.0;
            let half = 70.0;
            for k in 0..40 {
                let x0 = -half + 2.0 * half * k as f32 / 40.0;
                let x1 = -half + 2.0 * half * (k + 1) as f32 / 40.0;
                let a = 1.0 - ((x0 + x1) / 2.0 / half).abs();
                let pa = Pos2::new(logo_c.x + x0, ly);
                let pb = Pos2::new(logo_c.x + x1, ly);
                p.line_segment([pa, pb], Stroke::new(3.0_f32, alpha(ACCENT(), (a * 30.0) as u8)));
                p.line_segment([pa, pb], Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), (a * 200.0) as u8)));
            }

            // ---- compression level, centre top: click to change -------------------
            let lvl_font = FontId::new(11.5, family("ui"));
            let lvl_txts: Vec<Txt> = LEVELS.iter().map(|(k, _)| lay(&p, t(*k), lvl_font.clone())).collect();
            let lvl_sep = 6.0;
            let legal_w = lay(&p, &format!("§ {}", t("Legal")), lvl_font.clone()).size().x + 20.0;
            let cover_w = lay(&p, &format!("▣ {}", t("Cover")), lvl_font.clone()).size().x + 20.0;
            let extra_w = legal_w + lvl_sep + cover_w;
            let lvl_w: f32 = lvl_txts.iter().map(|g| g.size().x + 20.0).sum::<f32>() + lvl_sep * (LEVELS.len() - 1) as f32;
            let lvl_y = lr.bottom() + 20.0;
            // the row (levels + the Legal pill) stays centred as a whole; right-to-left
            // languages put the pill first
            let row_x = panel.center().x - (lvl_w + lvl_sep * 2.0 + extra_w) / 2.0;
            let mut lvl_x = if i18n::current().rtl() { row_x + extra_w + lvl_sep * 2.0 } else { row_x };
            // right-to-left languages read the pills right-to-left
            let lvl_order: Vec<usize> = if i18n::current().rtl() {
                (0..LEVELS.len()).rev().collect()
            } else {
                (0..LEVELS.len()).collect()
            };
            for k in lvl_order {
                let (key, lv) = LEVELS[k];
                let w = lvl_txts[k].size().x + 20.0;
                let rect = Rect::from_min_size(Pos2::new(lvl_x, lvl_y - 9.0), Vec2::new(w, 18.0));
                let sel = self.level == lv;
                let resp = ui.interact(rect, egui::Id::new(("level", k)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let hover = resp.hovered() && !busy_now;
                if sel {
                    p.rect_filled(rect, rr(9), skin::c().accent_btn());
                } else if hover {
                    p.rect_filled(rect, rr(9), alpha(ACCENT(), 34));
                }
                let col = if sel { ACCENT_FG() } else if hover { TEXT() } else { TEXT_DIM() };
                text_fit(&p, rect.center(), Align2::CENTER_CENTER, t(key), lvl_font.clone(), col, w - 10.0);
                if resp.clicked() && !busy_now {
                    sfx::play(Sfx::Click);
                    self.level = lv;
                }
                lvl_x += w + lvl_sep;
            }
            // the "Legal" pill closes the row (opposite end for right-to-left): lit
            // while new archives carry the terms; a click opens the terms box
            let rtl = i18n::current().rtl();
            let extras_x = if rtl { row_x } else { lvl_x + lvl_sep };
            {
                let w = legal_w;
                let x = if rtl { extras_x + cover_w + lvl_sep } else { extras_x };
                let rect = Rect::from_min_size(Pos2::new(x, lvl_y - 9.0), Vec2::new(w, 18.0));
                let resp = ui.interact(rect, egui::Id::new("legal-pill"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let hover = resp.hovered() && !busy_now;
                if hover {
                    p.rect_filled(rect, rr(9), alpha(ACCENT(), 34));
                }
                let col = if self.legal_on { ACCENT_SOFT() } else if hover { TEXT() } else { TEXT_DIM() };
                p.rect_stroke(rect, rr(9), Stroke::new(1.0_f32, alpha(col, if self.legal_on { 220 } else { 90 })), egui::StrokeKind::Inside);
                text_fit(&p, rect.center(), Align2::CENTER_CENTER, &format!("§ {}", t("Legal")), lvl_font.clone(), col, w - 10.0);
                if resp.clicked() && !busy_now && matches!(self.modal, Modal::None) {
                    sfx::play(Sfx::Click);
                    self.modal = Modal::Legal { text: self.legal_text.clone(), on: self.legal_on || self.legal_text.trim().is_empty(), error: None };
                }
            }
            // the "Cover" pill: lit while new archives carry the cover
            {
                let w = cover_w;
                let x = if rtl { extras_x } else { extras_x + legal_w + lvl_sep };
                let rect = Rect::from_min_size(Pos2::new(x, lvl_y - 9.0), Vec2::new(w, 18.0));
                let resp = ui.interact(rect, egui::Id::new("cover-pill"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                let hover = resp.hovered() && !busy_now;
                if hover {
                    p.rect_filled(rect, rr(9), alpha(ACCENT(), 34));
                }
                let lit = self.cover_on && self.cover.is_some();
                let col = if lit { ACCENT_SOFT() } else if hover { TEXT() } else { TEXT_DIM() };
                p.rect_stroke(rect, rr(9), Stroke::new(1.0_f32, alpha(col, if lit { 220 } else { 90 })), egui::StrokeKind::Inside);
                text_fit(&p, rect.center(), Align2::CENTER_CENTER, &format!("▣ {}", t("Cover")), lvl_font.clone(), col, w - 10.0);
                if resp.clicked() && !busy_now && matches!(self.modal, Modal::None) {
                    sfx::play(Sfx::Click);
                    self.modal = Modal::Cover { pic: self.cover.clone(), on: self.cover_on || self.cover.is_none(), error: None, tex: None };
                }
            }

            // ---- centre ------------------------------------------------------------
            let r = 92.0;
            let status_y = ring_c.y + r + 34.0;
            match &self.stage {
                Stage::Working(job) => {
                    let total = PROGRESS.total.load(Relaxed).max(1);
                    let done = PROGRESS.done.load(Relaxed).min(total);
                    let phase = PROGRESS.phase.load(Relaxed);
                    // 0-99 % while reading input, 100 % only when the archive is complete
                    let target = (done as f32 / total as f32 * 99.0).min(99.0);
                    self.shown += (target - self.shown) * 0.18;
                    p.circle_stroke(ring_c, r, Stroke::new(9.0_f32, TRACK()));
                    progress_arc(&p, ring_c, r, self.shown / 100.0, ACCENT());
                    let pct = format!("{}", self.shown.round() as u32);
                    let num = p.layout_no_wrap(pct, FontId::new(84.0, family("display-light")), TEXT());
                    let sign = p.layout_no_wrap("%".into(), FontId::new(26.0, family("display")), TEXT());
                    let wsum = num.size().x + 4.0 + sign.size().x;
                    let npos = Pos2::new(ring_c.x - wsum / 2.0, ring_c.y - num.size().y / 2.0 - 2.0);
                    let spos = Pos2::new(npos.x + num.size().x + 4.0, npos.y + num.size().y * 0.80 - sign.size().y);
                    p.galley(npos, num, TEXT());
                    p.galley(spos, sign, TEXT());
                    let phase_txt = match phase {
                        PHASE_SCAN => t("ANALYSING"),
                        PHASE_FINISH => t("FINISHING"),
                        _ => t("COMPRESSING"),
                    };
                    glow_text(&p, Pos2::new(ring_c.x, status_y), phase_txt, FontId::new(12.0, family("ui")), ACCENT_SOFT(), alpha(ACCENT(), 5), 3.0);
                    text(&p, Pos2::new(ring_c.x, status_y + 21.0), Align2::CENTER_CENTER, &job.label, FontId::new(14.0, family("ui-light")), TEXT());
                    let out_now = PROGRESS.out.load(Relaxed);
                    let (ratio_txt, eta_txt, size_txt) = if done > 0 && out_now > 0 {
                        let ratio = out_now as f64 * 100.0 / done as f64;
                        let eta = if total > done {
                            job.started.elapsed().as_secs_f64() * (total - done) as f64 / done as f64
                        } else {
                            0.0
                        };
                        let est = out_now as f64 * total as f64 / done as f64;
                        (format!("{:.0}%", ratio), human_secs(eta), human(est as u64))
                    } else {
                        ("…".to_string(), "…".to_string(), "…".to_string())
                    };
                    text_fit(&p,
                        Pos2::new(ring_c.x, status_y + 39.0),
                        Align2::CENTER_CENTER,
                        format!("{} {}   ·   {} {}   ·   {} {}", t("Ratio"), ratio_txt, t("Time"), eta_txt, t("Size"), size_txt),
                        FontId::new(12.0, family("ui-light")),
                        TEXT_DIM(),
                        W - 40.0,
                    );
                    // Cancel: stops at once, removes the unfinished archive, keeps the window open
                    let cancel = Rect::from_center_size(Pos2::new(ring_c.x, status_y + 68.0), Vec2::new(140.0, 32.0));
                    if PROGRESS.cancel.load(Relaxed) {
                        p.rect_stroke(cancel, rr(12), Stroke::new(1.0_f32, alpha(TEXT_DIM(), 80)), egui::StrokeKind::Inside);
                        text_fit(&p, cancel.center(), Align2::CENTER_CENTER, t("Cancel"), FontId::new(14.0, family("ui")), TEXT_DIM(), cancel.width() - 18.0);
                    } else if pill_button(ui, &p, cancel, t("Cancel"), false, "cancel") || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        sfx::play(Sfx::Cancel);
                        PROGRESS.cancel.store(true, Relaxed);
                    }
                    ctx.request_repaint_after(Duration::from_millis(16));
                }
                Stage::Done { out, input, output, secs, warnings } => {
                    p.circle_stroke(ring_c, r, Stroke::new(9.0_f32, TRACK()));
                    progress_arc(&p, ring_c, r, 1.0, OK_GREEN());
                    let num = p.layout_no_wrap("100".into(), FontId::new(84.0, family("display-light")), TEXT());
                    let sign = p.layout_no_wrap("%".into(), FontId::new(26.0, family("display")), TEXT());
                    let wsum = num.size().x + 4.0 + sign.size().x;
                    let npos = Pos2::new(ring_c.x - wsum / 2.0, ring_c.y - num.size().y / 2.0 - 2.0);
                    let spos = Pos2::new(npos.x + num.size().x + 4.0, npos.y + num.size().y * 0.80 - sign.size().y);
                    p.galley(npos, num, TEXT());
                    p.galley(spos, sign, TEXT());
                    glow_text(&p, Pos2::new(ring_c.x, status_y), if *warnings { t("DONE · SOME FILES SKIPPED") } else { t("DONE") }, FontId::new(12.0, family("ui")), OK_GREEN(), alpha(OK_GREEN(), 5), 3.0);
                    let name = out.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    text(&p, Pos2::new(ring_c.x, status_y + 21.0), Align2::CENTER_CENTER, &name, FontId::new(14.0, family("ui-light")), TEXT());
                    let pct = if *input > 0 { *output as f64 * 100.0 / *input as f64 } else { 100.0 };
                    text(&p, 
                        Pos2::new(ring_c.x, status_y + 39.0),
                        Align2::CENTER_CENTER,
                        format!("{} → {}  ({:.0} %)  ·  {:.1} s", human(*input), human(*output), pct, secs),
                        FontId::new(12.0, family("ui-light")),
                        TEXT_DIM(),
                    );
                    let out = out.clone();
                    // "Show in folder · Open in unNik · Compress more", centred whatever the language
                    let lf = FontId::new(12.0, family("ui"));
                    let mut links: Vec<(&str, &str)> = vec![("show", t("Show in folder")), ("unnik", t("Open in unNik")), ("more", t("Compress more"))];
                    if i18n::current().rtl() {
                        links.reverse();
                    }
                    let laid: Vec<Txt> = links.iter().map(|(_, l)| lay(&p, l, lf.clone())).collect();
                    let gap = 20.0;
                    let total = laid.iter().map(|g| g.size().x).sum::<f32>() + gap * (laid.len() - 1) as f32;
                    let row_y = status_y + 60.0;
                    let mut x = ring_c.x - total / 2.0;
                    let mut clicked: Option<&str> = None;
                    for (k, ((id, _), g)) in links.iter().zip(&laid).enumerate() {
                        let r = Rect::from_min_size(Pos2::new(x, row_y - 9.0), Vec2::new(g.size().x, 18.0));
                        let resp = ui.interact(r.expand(3.0), egui::Id::new(("done-link", *id)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                        g.paint(&p, Pos2::new(r.left(), row_y - g.size().y / 2.0), if resp.hovered() { TEXT() } else { ACCENT_SOFT() });
                        if resp.clicked() {
                            sfx::play(Sfx::Click);
                            clicked = Some(id);
                        }
                        if k + 1 < laid.len() {
                            text(&p, Pos2::new(r.right() + gap / 2.0, row_y), Align2::CENTER_CENTER, "·", lf.clone(), TEXT_DIM());
                        }
                        x = r.right() + gap;
                    }
                    let mr_clicked = clicked == Some("more");
                    match clicked {
                        Some("show") => show_in_folder(&out),
                        Some("unnik") => {
                            if let Err(e) = switch_to(ctx, "unNik", &[out.display().to_string()]) {
                                self.stage = Stage::Failed(e);
                            }
                        }
                        _ => {}
                    }
                    // the green circle also picks new files
                    let zone = Rect::from_center_size(ring_c, Vec2::splat(2.0 * r));
                    let zr = ui.interact(zone, egui::Id::new("donezone"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    if mr_clicked {
                        self.stage = Stage::Idle;
                    } else if zr.clicked() && matches!(self.stage, Stage::Done { .. }) {
                        self.stage = Stage::Idle;
                        self.chooser = true;
                    }
                    if hovering {
                        p.circle_stroke(ring_c, r + 16.0, Stroke::new(1.5_f32, alpha(ACCENT_SOFT(), 160)));
                    }
                }
                Stage::Idle | Stage::Failed(_) => {
                    // the whole circle is also a button: click to choose files
                    let zone = Rect::from_center_size(ring_c, Vec2::splat(2.0 * r));
                    let zr = ui.interact(zone, egui::Id::new("dropzone"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                    let hovering = hovering || zr.hovered();
                    // dashed ring, slowly turning; brighter while files hover over the window
                    let (ring_a, glow_a) = if hovering { (230u8, 60u8) } else { (110u8, 0u8) };
                    if hovering {
                        radial_glow(&p, ring_c, r + 40.0, alpha(ACCENT(), 50));
                    }
                    let segs = 48;
                    for k in 0..segs {
                        let a0 = now * 0.25 + k as f32 / segs as f32 * std::f32::consts::TAU;
                        let a1 = a0 + std::f32::consts::TAU / segs as f32 * 0.55;
                        let pts = arc_points(ring_c, r, a0, a1);
                        if glow_a > 0 {
                            p.add(PathShape::line(pts.clone(), Stroke::new(8.0_f32, alpha(ACCENT(), glow_a))));
                        }
                        // a small dark shadow under each dash keeps the ring readable on bright skins
                        p.add(PathShape::line(arc_points(ring_c + Vec2::new(0.0, 2.0), r, a0, a1), Stroke::new(2.0_f32, Color32::from_black_alpha(90))));
                        p.add(PathShape::line(pts, Stroke::new(2.0_f32, alpha(ACCENT_SOFT(), ring_a))));
                    }
                    p.circle_stroke(ring_c, r + 14.0, Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 30)));
                    // drop icon: arrow into a tray
                    let ic = ring_c + Vec2::new(0.0, (now * 2.2).sin() * if hovering { 4.0 } else { 1.5 });
                    let s = Stroke::new(3.0_f32, if hovering { TEXT() } else { ACCENT_SOFT() });
                    let halo = Stroke::new(9.0_f32, alpha(ACCENT(), 28));
                    let arrow = [
                        vec![ic + Vec2::new(0.0, -40.0), ic + Vec2::new(0.0, 8.0)],
                        vec![ic + Vec2::new(-17.0, -9.0), ic + Vec2::new(0.0, 8.0), ic + Vec2::new(17.0, -9.0)],
                        vec![ring_c + Vec2::new(-36.0, 14.0), ring_c + Vec2::new(-36.0, 36.0), ring_c + Vec2::new(36.0, 36.0), ring_c + Vec2::new(36.0, 14.0)],
                    ];
                    if !self.chooser {
                        // a small dark shadow keeps the arrow readable on bright wallpapers
                        let sh = Stroke::new(3.0_f32, Color32::from_black_alpha(96));
                        for line in &arrow {
                            p.add(PathShape::line(line.clone(), halo));
                        }
                        for line in &arrow {
                            p.add(PathShape::line(line.iter().map(|pt| *pt + Vec2::new(0.0, 2.0)).collect(), sh));
                        }
                        for line in arrow {
                            p.add(PathShape::line(line, s));
                        }
                    }
                    let files_hover = ctx.input(|i| !i.raw.hovered_files.is_empty());
                    let msg = if files_hover { t("Release to compress") } else { t("Drop a file or a folder to compress it") };
                    let msg_t = lay_fit(&p, &msg, FontId::new(16.0, family("ui-light")), W - 70.0);
                    let msg_r = Align2::CENTER_CENTER.anchor_size(Pos2::new(ring_c.x, status_y), msg_t.size());
                    paint_shadowed(&p, &msg_t, msg_r.min, TEXT());
                    // "or choose files · choose a folder" (mirrored for right-to-left languages)
                    let small = FontId::new(13.0, family("ui-light"));
                    let mut parts: Vec<(usize, &str)> = vec![(0, t("or")), (1, t("choose files")), (2, "·"), (3, t("choose a folder"))];
                    if i18n::current().rtl() {
                        parts.reverse();
                    }
                    let laid: Vec<Txt> = parts.iter().map(|(k, s)| lay(&p, s, if k % 2 == 1 { link_font() } else { small.clone() })).collect();
                    let gap = 7.0;
                    let total: f32 = laid.iter().map(|l| l.size().x).sum::<f32>() + gap * 3.0;
                    let mut x = ring_c.x - total / 2.0;
                    let row_c = status_y + 26.0;
                    let mut chosen: Option<Vec<PathBuf>> = None;
                    for ((k, s), l) in parts.iter().zip(&laid) {
                        let pos = Pos2::new(x, row_c - l.size().y / 2.0);
                        match k {
                            1 => {
                                if text_link(ui, &p, pos, s, "pick-files") {
                                    chosen = pick_files(parent);
                                }
                            }
                            3 => {
                                if text_link(ui, &p, pos, s, "pick-folder") {
                                    chosen = pick_folder(parent);
                                }
                            }
                            _ => paint_shadowed(&p, l, pos, TEXT_DIM()),
                        }
                        x += l.size().x + gap;
                    }
                    if zr.clicked() {
                        sfx::play(Sfx::Pop);
                        self.chooser = !self.chooser;
                    }
                    if self.chooser {
                        // two buttons inside the circle; a click elsewhere closes them
                        p.circle_filled(ring_c, r - 6.0, alpha(skin::c().field, 235));
                        for (k, (label, id)) in [(t("Choose files"), "ch-files"), (t("Choose a folder"), "ch-folder")].iter().enumerate() {
                            let b = Rect::from_center_size(ring_c + Vec2::new(0.0, if k == 0 { -26.0 } else { 26.0 }), Vec2::new(176.0, 40.0));
                            let resp = ui.interact(b, egui::Id::new(*id), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
                            p.rect_filled(b, rr(20), alpha(ACCENT(), if resp.hovered() { 120 } else { 60 }));
                            p.rect_stroke(b, rr(20), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 160)), egui::StrokeKind::Inside);
                            text_fit(&p, b.center(), Align2::CENTER_CENTER, *label, FontId::new(14.0, family("ui")), TEXT(), b.width() - 20.0);
                            if resp.clicked() {
                                sfx::play(Sfx::Click);
                                self.chooser = false;
                                chosen = if k == 0 { pick_files(parent) } else { pick_folder(parent) };
                            }
                        }
                        if ui.input(|i| i.pointer.any_click() || i.key_pressed(egui::Key::Escape))
                            && !ui.rect_contains_pointer(Rect::from_center_size(ring_c, Vec2::splat(2.0 * r)))
                        {
                            self.chooser = false;
                        }
                    }
                    let note_y = status_y + 48.0;
                    match &self.stage {
                        Stage::Failed(e) => {
                            let short: String = e.lines().next().unwrap_or("").chars().take(70).collect();
                            text_fit(&p, Pos2::new(ring_c.x, note_y), Align2::CENTER_CENTER, short, FontId::new(12.0, family("ui-light")), ERR_RED(), W - 40.0);
                        }
                        _ if self.elevated => {
                            text_fit(&p, Pos2::new(ring_c.x, note_y), Align2::CENTER_CENTER, t("Running as administrator: Windows blocks drag and drop from Explorer."), FontId::new(11.5, family("ui-light")), WARN(), W - 40.0);
                            text_fit(&p, Pos2::new(ring_c.x, note_y + 17.0), Align2::CENTER_CENTER, t("Use the links above, or start nikpak normally (not as administrator)."), FontId::new(11.5, family("ui-light")), TEXT_DIM(), W - 40.0);
                        }
                        _ => {}
                    }
                    if let Some(paths) = chosen {
                        self.start_job(paths);
                    }
                    ctx.request_repaint_after(Duration::from_millis(33));
                }
            }

            // ---- destination ---------------------------------------------------------
            let busy = matches!(self.stage, Stage::Working(_));
            let label_y = panel.bottom() - 120.0;
            let lab = spaced(&p, t("SAVE TO"), FontId::new(10.5, family("ui")), TEXT_DIM(), 2.0);
            paint_label(&p, &lab, panel.left() + 28.0, panel.right() - 116.0, label_y, TEXT_DIM());
            let field = Rect::from_min_max(Pos2::new(panel.left() + 24.0, label_y + 20.0), Pos2::new(panel.right() - 160.0, label_y + 56.0));
            let open_dest = Rect::from_min_max(Pos2::new(panel.right() - 152.0, label_y + 20.0), Pos2::new(panel.right() - 116.0, label_y + 56.0));
            let browse = Rect::from_min_max(Pos2::new(panel.right() - 106.0, label_y + 20.0), Pos2::new(panel.right() - 24.0, label_y + 56.0));
            p.rect_filled(field, rr(10), alpha(skin::c().field, 170));
            p.rect_stroke(field, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 60)), egui::StrokeKind::Inside);
            let edit = egui::TextEdit::singleline(&mut self.dest)
                .font(FontId::new(13.0, family("ui-light")))
                .text_color(TEXT())
                .frame(false)
                .margin(egui::Margin::symmetric(12, 0))
                .vertical_align(egui::Align::Center)
                .hint_text(tip("Destination folder"));
            let resp = ui.add_enabled_ui(!busy, |ui| ui.put(field, edit)).inner;
            if resp.lost_focus() {
                save_destination(&self.dest);
            }
            let b = ui.interact(browse, egui::Id::new("browse"), Sense::click());
            let fill = if b.hovered() && !busy { alpha(ACCENT(), 70) } else { alpha(ACCENT(), 34) };
            p.rect_filled(browse, rr(10), fill);
            p.rect_stroke(browse, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 120)), egui::StrokeKind::Inside);
            text_fit(&p, browse.center(), Align2::CENTER_CENTER, t("Browse"), FontId::new(13.0, family("ui")), if busy { TEXT_DIM() } else { TEXT() }, browse.width() - 10.0);
            // open the destination folder in Explorer / Finder
            let od = ui.interact(open_dest, egui::Id::new("open-dest"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(tip("Open the destination folder"));
            p.rect_filled(open_dest, rr(10), if od.hovered() { alpha(ACCENT(), 70) } else { alpha(ACCENT(), 34) });
            p.rect_stroke(open_dest, rr(10), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), 120)), egui::StrokeKind::Inside);
            folder_open_glyph(&p, open_dest.center(), if od.hovered() { hi(255) } else { TEXT() });
            if od.clicked() {
                sfx::play(Sfx::Pop);
                let d = PathBuf::from(self.dest.trim());
                if self.dest.trim().is_empty() {
                    self.stage = Stage::Failed(t("Choose a destination folder first.").into());
                } else if std::fs::create_dir_all(&d).is_ok() {
                    save_destination(&self.dest);
                    open_folder(&d);
                } else {
                    self.stage = Stage::Failed(tf("Cannot use the destination folder: {}", &[&self.dest]));
                }
            }
            if b.clicked() && !busy {
                sfx::play(Sfx::Click);
                let mut dlg = rfd::FileDialog::new().set_parent(parent).set_title(t("Where should the .nik archives be saved?"));
                if Path::new(self.dest.trim()).is_dir() {
                    dlg = dlg.set_directory(self.dest.trim());
                }
                if let Some(dir) = dlg.pick_folder() {
                    self.dest = dir.display().to_string();
                    save_destination(&self.dest);
                }
            }

            // ---- community links, centred under the destination ------------------------
            let icon_y = panel.bottom() - 32.0;
            let step = 58.0;
            for (k, (name, url)) in SOCIAL.iter().enumerate() {
                let c = Pos2::new(panel.right() - 44.0 - (2 - k) as f32 * step, icon_y);
                let hit = Rect::from_center_size(c, Vec2::splat(44.0));
                let resp = ui
                    .interact(hit, egui::Id::new(("social", k)), Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text(format!("{name}: {url}"));
                let hov = resp.hovered() || std::env::var("NIKPAK_GUI_ICON_HOVER").map_or(false, |v| v == *name);
                brand_icon(&p, name, c, if hov { 21.0 } else { 19.0 }, hov);
                if resp.clicked() {
                    open_link(url);
                }
            }

            // ---- language, bottom left: flag + name; the menu opens above it ----------
            let cur = i18n::current();
            let lb = lang_button_rect(panel);
            let lresp = ui.interact(lb, egui::Id::new("lang"), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
            let hov = (lresp.hovered() || self.lang_open) && !busy;
            p.rect_filled(lb, rr(15), alpha(ACCENT(), if hov { 46 } else { 16 }));
            p.rect_stroke(lb, rr(15), Stroke::new(1.0_f32, alpha(ACCENT_SOFT(), if hov { 150 } else { 60 })), egui::StrokeKind::Inside);
            let fr = Rect::from_min_size(Pos2::new(lb.left() + 11.0, lb.center().y - 7.0), Vec2::new(21.0, 14.0));
            draw_flag(&p, fr, cur);
            let name_w = lb.right() - 22.0 - (fr.right() + 8.0);
            let nt = lay_fit(&p, cur.native_name(), FontId::new(12.5, family("ui-light")), name_w);
            nt.paint(&p, Pos2::new(fr.right() + 8.0, lb.center().y - nt.size().y / 2.0), if busy { TEXT_DIM() } else { TEXT() });
            let cc = Pos2::new(lb.right() - 13.0, lb.center().y + 1.0);
            p.add(Shape::convex_polygon(
                vec![cc + Vec2::new(-4.0, -2.5), cc + Vec2::new(4.0, -2.5), cc + Vec2::new(0.0, 2.5)],
                if busy { TEXT_DIM() } else { ACCENT_SOFT() },
                Stroke::NONE,
            ));
            if lresp.clicked() && !busy {
                self.lang_open = !self.lang_open;
            }
            panel_rect = panel;
        });
        if matches!(self.modal, Modal::None) {
            lang_menu(ctx, panel_rect, lang_button_rect(panel_rect), &mut self.lang_open, &[]);
            skin_menu(ctx, parent, panel_rect, window_rect, &mut self.skins);
        }
        self.draw_modal(ctx, panel_rect);
    }
}

/// Hide the console window that Windows opens for a console program started
/// from Explorer (only when no other process shares that console, so a
/// command prompt the user started nikpak from is never closed).
#[cfg(windows)]
pub(crate) fn detach_own_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    let mut ids = [0u32; 4];
    // SAFETY: plain Win32 calls with a valid buffer of the given length.
    unsafe {
        if GetConsoleProcessList(ids.as_mut_ptr(), ids.len() as u32) == 1 {
            FreeConsole();
        }
    }
}

/// Start the window. Err when no graphical display is available.
/// Windows blocks drag and drop from Explorer into programs running as
/// administrator. If nikpak's window was started elevated (e.g. from an
/// administrator PowerShell), start it again through Explorer, which runs it
/// as the normal user, and let this instance exit. A marker file prevents a
/// loop when the relaunched copy is elevated too (UAC switched off).
#[cfg(windows)]
pub(crate) fn relaunch_unelevated() -> bool {
    let marker = std::env::temp_dir().join("nikpak-unelevated.marker");
    if !is_elevated() {
        let _ = std::fs::remove_file(&marker);
        return false;
    }
    if let Ok(age) = std::fs::metadata(&marker).and_then(|m| m.modified()).map(|t| t.elapsed().unwrap_or_default()) {
        let _ = std::fs::remove_file(&marker);
        if age.as_secs() < 20 {
            return false; // we are the relaunched copy and still elevated: carry on
        }
    }
    let Ok(exe) = std::env::current_exe() else { return false };
    if std::fs::write(&marker, b"1").is_err() {
        return false;
    }
    std::process::Command::new("explorer.exe").arg(exe).spawn().is_ok()
}

pub fn run() -> Result<(), String> {
    // a capture keeps the window it was given: the shell that started it has to
    // own it to copy it, and that shell is often elevated (see App::new)
    #[cfg(windows)]
    if std::env::var_os("NIKPAK_GUI_CAPTURE").is_none() && relaunch_unelevated() {
        return Ok(());
    }
    #[cfg(windows)]
    detach_own_console();
    i18n::set(load_language());
    start_sounds();
    let title = if license::current().is_some() { "NikPak".to_string() } else { t("NikPak (evaluation copy)").to_string() };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([W, H])
            .with_min_inner_size([W, H])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true)
            .with_icon(Arc::new(icon())),
        centered: true,
        ..Default::default()
    };
    eframe::run_native("NikPak", options, Box::new(|cc| Ok(Box::new(App::new(cc, None))))).map_err(|e| e.to_string())
}

/// What the right-click menu asked NikPak to do.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShellAdd {
    /// "Add to archive…": ask for the name and the folder of the archive
    Dialog,
    /// "Add to archive here": the archive goes next to the items
    Here,
}

/// `nikpak --add <items>` / `nikpak --add-here <items>`: open the window and
/// compress the items (all the items of a multiple selection go into one archive).
pub fn run_shell(mode: ShellAdd, items: Vec<PathBuf>) -> Result<(), String> {
    #[cfg(windows)]
    detach_own_console();
    let verb = if mode == ShellAdd::Here { "add-here" } else { "add" };
    let Some(items) = crate::shell::collect(verb, items) else {
        return Ok(()); // another NikPak window takes care of this selection
    };
    let items: Vec<PathBuf> = items.into_iter().filter(|p| p.exists()).collect();
    if items.is_empty() {
        return Ok(());
    }
    i18n::set(load_language());
    let folder = items[0].parent().map(Path::to_path_buf).unwrap_or_else(default_destination);
    let stem = archive_stem(&items);
    let out = match mode {
        ShellAdd::Here => unique_archive_path(&folder, &stem),
        ShellAdd::Dialog => {
            let suggested = unique_archive_path(&folder, &stem);
            let picked = rfd::FileDialog::new()
                .set_title(t("Save the archive as"))
                .set_directory(&folder)
                .set_file_name(suggested.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| format!("{stem}.nik")))
                .add_filter("NikPak", &["nik"])
                .save_file();
            let Some(mut p) = picked else { return Ok(()) };
            if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nik")) {
                let mut name = p.file_name().map(|s| s.to_os_string()).unwrap_or_default();
                name.push(".nik");
                p.set_file_name(name);
            }
            p
        }
    };
    start_sounds();
    let title = if license::current().is_some() { "NikPak".to_string() } else { t("NikPak (evaluation copy)").to_string() };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([W, H])
            .with_min_inner_size([W, H])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_drag_and_drop(true)
            .with_icon(Arc::new(icon())),
        centered: true,
        ..Default::default()
    };
    let job = Some((items, out));
    eframe::run_native("NikPak", options, Box::new(move |cc| Ok(Box::new(App::new(cc, job))))).map_err(|e| e.to_string())
}
