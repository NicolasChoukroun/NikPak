//! The launch check for a newer release: one small HTTPS request to the web
//! site, once, when the window opens. Nothing is downloaded or installed here -
//! the window only tells the user, and opens the download page if they ask.
//!
//! The same logic as src-tauri/src/updatecheck.rs in NikEdit, NikSQL and
//! Nikkomander: the same manifest, the same version comparison, the same
//! decision. Only the HTTP call differs, because this program has no HTTP crate
//! of its own and asks the one the system already ships (curl, present on
//! Windows 10 and later, on macOS and on every Linux desktop).

use std::process::{Command, Stdio};
use std::time::Duration;

/// The manifest published with the web site.
pub const VERSION_URL: &str = "https://nikpak.nikkoid.com/version.json";
/// Where the "Download" button of the update screen sends the user.
pub const DOWNLOAD_URL: &str = "https://nikpak.nikkoid.com/#download";

/// What the window shows: the installed version, the published one, and the
/// notes of the release. `newer` is the decision, taken here.
#[derive(Clone, Debug, Default)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub newer: bool,
    pub url: String,
    pub notes: String,
}

/// "1.2.10" > "1.2.9": compare the numbers, not the text.
fn parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches('v')
        .split('.')
        .map(|x| x.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let a = parts(latest);
    if a.iter().all(|n| *n == 0) {
        return false; // an empty or unreadable version is never "newer"
    }
    a > parts(current)
}

/// The value of `"key": "text"` in the manifest. Enough for this small file,
/// and it never treats a malformed answer as a release.
pub fn pick(text: &str, key: &str) -> Option<String> {
    let at = text.find(&format!("\"{key}\""))?;
    let after = &text[at + key.len() + 2..];
    let colon = after.find(':')?;
    let rest = after[colon + 1..].trim_start();
    let mut chars = rest.chars();
    if chars.next()? != '"' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    for c in chars {
        match c {
            _ if escaped => {
                out.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    other => other,
                });
                escaped = false;
            }
            '\\' => escaped = true,
            '"' => return Some(out),
            other => out.push(other),
        }
    }
    None
}

/// Ask the server once. Any failure (offline, no curl, timeout, an answer that
/// is not the manifest) is simply "no news": the launch carries on.
pub fn check_update() -> UpdateInfo {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let manifest = fetch_manifest();
    let Some(text) = manifest else {
        return UpdateInfo { current, ..Default::default() };
    };
    let latest = pick(&text, "latest").or_else(|| pick(&text, "version")).unwrap_or_default();
    let notes = pick(&text, "notes").unwrap_or_default();
    let url = pick(&text, "url").filter(|u| u.starts_with("https://")).unwrap_or_else(|| DOWNLOAD_URL.to_string());
    UpdateInfo { newer: is_newer(&latest, &current), current, latest, url, notes }
}

fn fetch_manifest() -> Option<String> {
    let mut cmd = if cfg!(windows) { Command::new("curl.exe") } else { Command::new("curl") };
    // a window-less program: no console box may flash up (CREATE_NO_WINDOW)
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.args(["-fsS", "--max-time", "8", "-A", concat!("NikPak/", env!("CARGO_PKG_VERSION")), VERSION_URL])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(Stdio::piped());
    let out = cmd.output().ok()?;
    if !out.status.success() || out.stdout.len() > 64 << 10 {
        return None;
    }
    String::from_utf8(out.stdout).ok()
}

/// The check runs on its own thread while the window opens, so a slow network
/// never delays the program: the answer arrives through this channel.
pub type Answer = std::sync::mpsc::Receiver<UpdateInfo>;

pub fn check_in_background() -> Answer {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(600)); // let the window appear first
        let _ = tx.send(check_update());
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numbers_not_text() {
        assert!(is_newer("1.2.10", "1.2.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.1.0"));
        assert!(!is_newer("", "1.0.0"));
        assert!(!is_newer("banana", "1.0.0"));
        assert!(is_newer("v1.1.0", "1.0.0"));
    }

    #[test]
    fn reads_the_manifest() {
        let text = r#"{"latest": "1.8.0", "url": "https://nikpak.nikkoid.com/#download", "notes": "Faster. Safer."}"#;
        assert_eq!(pick(text, "latest").as_deref(), Some("1.8.0"));
        assert_eq!(pick(text, "notes").as_deref(), Some("Faster. Safer."));
        assert_eq!(pick(text, "missing"), None);
        // a quoted quote, and an escaped newline
        assert_eq!(pick(r#"{"notes": "a \"b\"\nc"}"#, "notes").as_deref(), Some("a \"b\"\nc"));
        assert_eq!(pick("not json", "latest"), None);
    }
}
