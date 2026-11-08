//! **Nikkosynth soundbanks** — a `.nik` that is not a NikPak archive.
//!
//! Both products ship a file called `.nik`, and they are different formats:
//!
//! ```text
//! NikPak archive     89 4E 49 4B 0D 0A 1A 0A …   (the "\x89NIK\r\n\x1a\n" magic)
//! Nikkosynth bank    "NIKPACK1" then one JSON line, then the encrypted body
//! ```
//!
//! unNik used to answer *"is not a .nik archive (bad signature)"* for the second,
//! which is true and useless: the file **is** a `.nik`, and a soundbank the player
//! just exported is exactly the thing they will try to open. So this module reads the
//! part of a soundbank that is deliberately readable **without its password** — the
//! header line: the bank's name, author, description, terms, patch count and cover —
//! and unNik shows that as a card instead of an error.
//!
//! **It does not decrypt anything, on purpose.** The sounds, their names and their
//! categories live inside one encrypted, compressed stream; that is what makes a sold
//! soundbank impossible to lift out of the application's files, and an archiver that
//! could read it would be a way around that. So unNik reads what the format exposes
//! and tells the player to open the bank in Nikkosynth.

use std::io::Read;
use std::path::Path;

use crate::error::{NikError, Result};

/// The soundbank container's magic (`nikpack.rs` in Nikkosynth writes it).
pub const MAGIC: &[u8; 8] = b"NIKPACK1";

/// The plaintext header is one JSON line. This bounds how much is read before the
/// line ends, so a file that is not really a bank cannot make unNik read gigabytes:
/// the header can legitimately carry a cover picture, so the ceiling is generous
/// (4 MB of picture becomes ~5.5 MB of base64) but still a ceiling.
const MAX_HEADER: usize = 8 * 1024 * 1024;

/// What a soundbank says about itself before its password is asked for.
#[derive(Clone, Debug, Default)]
pub struct Info {
    pub name: String,
    pub author: String,
    pub description: String,
    pub legal: Option<String>,
    /// How many patches the bank holds.
    pub count: usize,
    /// The container version the bank was written with.
    pub pack_version: u64,
    /// The cover picture, JPEG or PNG bytes.
    pub cover: Option<Vec<u8>>,
    /// The bank's own folder (`Users` for a player's bank, `Factory` for ours).
    pub owner: String,
}

/// Is this file a Nikkosynth soundbank? Reads the first eight bytes only.
pub fn is_soundbank(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic).is_ok() && &magic == MAGIC
}

/// Read a soundbank's header. Fails only when the file is not a bank or its header is
/// cut short — never on the contents, which are opaque by design.
pub fn read_info(path: &Path) -> Result<Info> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| NikError::Fatal(format!("Cannot open '{}': {e}", path.display())))?;
    let mut head = Vec::with_capacity(4096);
    let mut buf = [0u8; 8192];
    let mut nl = None;
    while head.len() < MAX_HEADER {
        let n = f
            .read(&mut buf)
            .map_err(|e| NikError::Fatal(format!("Cannot read '{}': {e}", path.display())))?;
        if n == 0 {
            break;
        }
        if let Some(at) = buf[..n].iter().position(|b| *b == b'\n') {
            head.extend_from_slice(&buf[..at]);
            nl = Some(());
            break;
        }
        head.extend_from_slice(&buf[..n]);
    }
    if nl.is_none() {
        return Err(NikError::Corrupt(
            "This Nikkosynth soundbank has no readable header.".into(),
        ));
    }
    if head.len() < MAGIC.len() || &head[..MAGIC.len()] != MAGIC {
        return Err(NikError::Corrupt("Not a Nikkosynth soundbank.".into()));
    }
    let text = String::from_utf8_lossy(&head[MAGIC.len()..]).into_owned();
    Ok(Info {
        name: field(&text, "name").unwrap_or_else(|| "Nikkosynth soundbank".into()),
        author: field(&text, "author").unwrap_or_default(),
        description: field(&text, "description").unwrap_or_default(),
        legal: field(&text, "legal"),
        count: field(&text, "count").and_then(|c| c.parse().ok()).unwrap_or(0),
        pack_version: field(&text, "packVersion").and_then(|c| c.parse().ok()).unwrap_or(0),
        cover: field(&text, "cover").and_then(|b64| crate::soundbank::b64_decode(&b64)),
        owner: field(&text, "owner").unwrap_or_else(|| "Users".into()),
    })
}

/// The value of a top-level string or number field of the header's JSON object.
///
/// A hand-written scan rather than a JSON dependency: the header is one flat object of
/// strings and integers written by Nikkosynth's `serde_json`, and pulling `serde_json`
/// into this crate for six fields would be a new dependency on the whole build. The
/// scan is a real one — it walks the object, tracking which strings are **keys** and
/// which are values — because a first version searched for `"name"` anywhere in the
/// line, and a description that happened to quote a key name would have been read as
/// the field itself.
fn field(json: &str, key: &str) -> Option<String> {
    let b = json.as_bytes();
    let mut i = skip_ws(b, 0);
    if b.get(i) != Some(&b'{') {
        return None;
    }
    i += 1;
    loop {
        i = skip_ws(b, i);
        if b.get(i) != Some(&b'"') {
            return None;
        }
        let (name, next) = read_string(b, i)?;
        i = skip_ws(b, next);
        if b.get(i) != Some(&b':') {
            return None;
        }
        i = skip_ws(b, i + 1);
        // A string value, or a number / true / false / null up to the next separator.
        let (value, next) = if b.get(i) == Some(&b'"') {
            let (s, n) = read_string(b, i)?;
            (Some(s), n)
        } else {
            let mut j = i;
            while j < b.len() && b[j] != b',' && b[j] != b'}' {
                j += 1;
            }
            let raw = String::from_utf8_lossy(&b[i..j]).trim().to_string();
            let v = if raw.is_empty() || raw == "null" { None } else { Some(raw) };
            (v, j)
        };
        if name == key {
            return value;
        }
        i = skip_ws(b, next);
        match b.get(i) {
            Some(&b',') => i += 1,
            _ => return None,
        }
    }
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Read a JSON string starting at the opening quote; returns it decoded and the index
/// just past its closing quote.
fn read_string(b: &[u8], start: usize) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut i = start + 1;
    while i < b.len() {
        match b[i] {
            b'"' => return Some((out, i + 1)),
            b'\\' => {
                i += 1;
                match b.get(i) {
                    Some(&b'n') => out.push('\n'),
                    Some(&b't') => out.push('\t'),
                    Some(&b'r') => out.push('\r'),
                    Some(&b'b') => out.push('\u{8}'),
                    Some(&b'f') => out.push('\u{c}'),
                    Some(&b'u') => {
                        let hex = b.get(i + 1..i + 5)?;
                        let n = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                        out.push(char::from_u32(n).unwrap_or('\u{fffd}'));
                        i += 4;
                    }
                    Some(&other) => out.push(other as char),
                    None => return None,
                }
                i += 1;
            }
            _ => {
                // Copy a whole UTF-8 character: the header is text, so decode from the
                // remainder rather than pushing single bytes.
                let rest = std::str::from_utf8(&b[i..]).ok()?;
                let ch = rest.chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

/// Standard base64 (padding optional), for the header's cover picture.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'\n' | b'\r' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// The one-sentence answer for anything that needs one line: the CLI, a drag-and-drop,
/// the "Test" command.
pub fn describe(info: &Info) -> String {
    let mut s = format!(
        "This is a Nikkosynth soundbank, not a NikPak archive ({} patch{}",
        info.count,
        if info.count == 1 { "" } else { "es" }
    );
    if !info.author.trim().is_empty() {
        s.push_str(&format!(", by {}", info.author.trim()));
    }
    s.push(')');
    s.push_str(". Its sounds are encrypted; open it in Nikkosynth to import them.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bank(header: &str) -> Vec<u8> {
        let mut v = MAGIC.to_vec();
        v.extend_from_slice(header.as_bytes());
        v.push(b'\n');
        v.extend_from_slice(&[0u8; 64]);
        v
    }

    #[test]
    fn the_header_fields_are_read_without_a_password() {
        let bytes = bank(
            r#"{"format":"nikkosynth-bank","packVersion":3,"name":"Mountains","kind":"user","owner":"Users","count":25,"salt":"00","nonce":"00","author":"N. Choukroun","description":"Cold pads, \"winter\" keys.","legal":"No resale.","coverType":"image/png","cover":"iVBORw0KGgo="}"#,
        );
        let dir = std::env::temp_dir().join(format!("nik-sb-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("Mountains.nik");
        std::fs::write(&p, &bytes).unwrap();

        assert!(is_soundbank(&p));
        let info = read_info(&p).unwrap();
        assert_eq!(info.name, "Mountains");
        assert_eq!(info.author, "N. Choukroun");
        assert_eq!(info.description, "Cold pads, \"winter\" keys.");
        assert_eq!(info.legal.as_deref(), Some("No resale."));
        assert_eq!(info.count, 25);
        assert_eq!(info.pack_version, 3);
        assert_eq!(info.owner, "Users");
        assert_eq!(info.cover.as_deref(), Some(&b"\x89PNG\r\n\x1a\n"[..]));
        assert!(describe(&info).contains("25 patches"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bank_without_an_author_or_cover_still_reads() {
        let bytes = bank(r#"{"format":"nikkosynth-bank","packVersion":3,"name":"Bare","count":1}"#);
        let dir = std::env::temp_dir().join(format!("nik-sb2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("Bare.nik");
        std::fs::write(&p, &bytes).unwrap();
        let info = read_info(&p).unwrap();
        assert_eq!(info.name, "Bare");
        assert_eq!(info.count, 1);
        assert!(info.cover.is_none());
        assert_eq!(describe(&info), "This is a Nikkosynth soundbank, not a NikPak archive (1 patch). Its sounds are encrypted; open it in Nikkosynth to import them.");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_nikpak_archive_is_not_a_soundbank() {
        let bytes = crate::format::MAGIC.to_vec();
        let dir = std::env::temp_dir().join(format!("nik-sb3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("archive.nik");
        std::fs::write(&p, &bytes).unwrap();
        assert!(!is_soundbank(&p));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn base64_round_trips() {
        assert_eq!(b64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(b64_decode("TWE=").unwrap(), b"Ma");
        assert!(b64_decode("TW#u").is_none());
    }
}
