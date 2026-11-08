//! **NikPak packages** — a `.nik` that is a *product*: an encrypted, compressed
//! bundle of files wrapped in a header that carries the package's name, author,
//! description, cover picture and an end-user licence agreement ("EULA").
//!
//! This is the NikPak sibling of Nikkosynth's soundbank container. Both are `.nik`
//! files with the same shape — a plaintext JSON header, then one sealed body — but
//! they are **different formats**:
//!
//! ```text
//! NikPak archive      89 4E 49 4B 0D 0A 1A 0A …   ("\x89NIK\r\n\x1a\n" magic)
//! NikPak package      "NIKPACK2" then one JSON line, then the sealed body
//! Nikkosynth bank     "NIKPACK1" then one JSON line, then the sealed body
//! ```
//!
//! The `2` magic is the whole point: a NikPak package will **not** import in
//! Nikkosynth (which only accepts `NIKPACK1`), and a Nikkosynth bank is refused here.
//!
//! ## What the header exposes, and what it does not
//!
//! Only the header is plaintext: the package's name, author, description, its EULA
//! (shown **before** the password, and which must be approved before anything is
//! decrypted), its cover picture, the entry count, and the salt / nonce. The file
//! names and the file contents live inside one sealed stream, so a package that is
//! sold cannot be read without its password.
//!
//! ## The body
//!
//! ```text
//! record := type:u8
//!   REC_FILE : name_len:u16 name mtime:i64 mode:u32 size:u64 data[size] crc32:u32
//!   REC_DIR  : name_len:u16 name mtime:i64 mode:u32
//!   REC_END  : count:u64
//! ```
//!
//! The record stream is zstd-compressed and, when a password was given, encrypted
//! with a SHA-256 CTR keystream and sealed with an HMAC-SHA-256 tag over the header
//! and ciphertext (the same deliberately small, standard-library-only construction
//! Nikkosynth's soundbank uses). A wrong password and a tampered file fail the same
//! way, so they cannot be told apart.

use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::error::{NikError, Result};

/// The container's magic, so a wrong file is rejected before any parsing.
pub const MAGIC: &[u8; 8] = b"NIKPACK2";
/// The only container version this build writes and reads.
pub const PACKAGE_VERSION: u32 = 1;

/// Longest EULA a package may carry (bytes of UTF-8).
pub const MAX_EULA_LEN: usize = 64 * 1024;
/// Longest description a package may carry.
pub const MAX_DESCRIPTION_LEN: usize = 4 * 1024;
/// Longest author name a package may carry.
pub const MAX_AUTHOR_LEN: usize = 256;
/// Longest package name.
pub const MAX_NAME_LEN: usize = 256;
/// Longest entry name inside the body (a '/'-separated path).
const MAX_ENTRY_NAME_LEN: usize = 4096;
/// Largest cover picture (the encoded JPEG or PNG), matching `cover::MAX_COVER_BYTES`.
pub const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;
/// Largest body accepted after decompression (refuses a "zip bomb").
const MAX_BODY: u64 = 512 * 1024 * 1024;
/// Largest plaintext header line read before the newline (a cover can make it
/// several MB once base64-encoded).
const MAX_HEADER: usize = 8 * 1024 * 1024;

/// SHA-256 iterations for the key derivation. The same order of magnitude as
/// Nikkosynth's container 3, so a stolen package cannot be brute-forced at full
/// SHA-256 speed (roughly half a second per guess).
pub const KDF_ROUNDS: u32 = 600_000;

const REC_END: u8 = 0;
const REC_FILE: u8 = 1;
const REC_DIR: u8 = 2;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Everything the package box can read **without** the password: enough for the
/// box that shows the picture, name, description, the EULA and asks for the key.
#[derive(Clone, Debug, Default)]
pub struct PackageInfo {
    pub name: String,
    pub author: String,
    pub description: String,
    pub eula: Option<String>,
    /// How many entries (files and folders) the package holds.
    pub count: usize,
    /// The cover picture, JPEG or PNG bytes.
    pub cover: Option<Vec<u8>>,
    /// The media type of the cover ("image/jpeg" / "image/png").
    pub cover_type: Option<String>,
    pub encrypted: bool,
}

/// One entry of a package being created. Files keep a source path; folders carry
/// only their name.
#[derive(Clone, Debug)]
pub struct PackEntry {
    /// Stored path, '/' separated, without a trailing '/'.
    pub name: String,
    /// Where the file's bytes come from (empty for folders).
    pub src: PathBuf,
    pub is_dir: bool,
    pub mtime: i64,
    pub mode: u32,
    pub size: u64,
}

/// A package in memory, ready to be sealed.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    pub author: String,
    pub description: String,
    pub eula: Option<String>,
    /// The cover picture (JPEG or PNG bytes), already checked by `cover::kind`.
    pub cover: Option<Vec<u8>>,
    pub entries: Vec<PackEntry>,
}

// ---------------------------------------------------------------------------
// Small primitives
// ---------------------------------------------------------------------------

fn hash(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// Derive the 64-byte working key (`[encryption 32 | verify 32]`) from a password.
fn derive(password: &str, salt: &[u8], rounds: u32) -> [u8; 64] {
    let mut state = hash(&[salt, password.as_bytes()]);
    for _ in 1..rounds.max(1) {
        state = hash(&[&state, salt, password.as_bytes()]);
    }
    let enc = hash(&[&state, b"enc"]);
    let ver = hash(&[&state, b"mac"]);
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&enc);
    out[32..].copy_from_slice(&ver);
    out
}

/// XOR the body with the SHA-256 CTR keystream.
fn keystream_xor(key: &[u8; 64], nonce: &[u8], data: &mut [u8]) {
    let mut counter: u64 = 0;
    let mut pos = 0usize;
    while pos < data.len() {
        let block = hash(&[&key[..32], nonce, &counter.to_le_bytes()]);
        let n = (data.len() - pos).min(32);
        for i in 0..n {
            data[pos + i] ^= block[i];
        }
        pos += n;
        counter = counter.wrapping_add(1);
    }
}

/// HMAC-SHA-256 over the parts (the header and the ciphertext).
fn tag_hmac(key: &[u8; 64], parts: &[&[u8]]) -> [u8; 32] {
    let mut k = [0u8; 64];
    k[..32].copy_from_slice(&key[32..]);
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    for p in parts {
        inner.update(p);
    }
    let inner_digest: [u8; 32] = inner.finalize().into();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_digest);
    outer.finalize().into()
}

/// Constant-time comparison for the tag.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..b.len()).step_by(2) {
        let hi = (b[i] as char).to_digit(16)?;
        let lo = (b[i + 1] as char).to_digit(16)?;
        out.push((hi * 16 + lo) as u8);
    }
    Some(out)
}

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    out
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
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
    if bits >= 6 {
        return None;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The header: hand-written JSON, so no JSON dependency is needed (see
// soundbank.rs, which does the same for Nikkosynth's header).
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// The value of a top-level string / number / boolean field of the header's JSON
/// object, scanned by hand (the header is one flat object of strings, numbers and
/// booleans written by `seal`). `None` when the field is absent.
fn json_field(json: &str, key: &str) -> Option<String> {
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

/// Read a JSON string starting at the opening quote; returns it decoded and the
/// index just past its closing quote.
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
                let rest = std::str::from_utf8(&b[i..]).ok()?;
                let ch = rest.chars().next()?;
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Collecting the files to package
// ---------------------------------------------------------------------------

fn mtime_of(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn mode_of(md: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    md.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(md: &std::fs::Metadata) -> u32 {
    if md.is_dir() {
        0o755
    } else if md.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

fn base_name(p: &Path) -> String {
    crate::util::clean_name_components(p)
        .last()
        .cloned()
        .unwrap_or_default()
}

/// Walk `paths` into a flat list of entries, folders first and their contents
/// under them (the stored name is the path *inside* the package). Symbolic links
/// are skipped, as they are for archives.
pub fn collect(paths: &[PathBuf]) -> Vec<PackEntry> {
    fn walk(p: &Path, name: String, out: &mut Vec<PackEntry>) {
        let md = match std::fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(_) => return,
        };
        if md.file_type().is_symlink() {
            return;
        }
        if md.is_dir() {
            out.push(PackEntry {
                name: name.clone(),
                src: PathBuf::new(),
                is_dir: true,
                mtime: mtime_of(&md),
                mode: mode_of(&md),
                size: 0,
            });
            let Ok(rd) = std::fs::read_dir(p) else { return };
            let mut children: Vec<_> = rd.filter_map(|e| e.ok()).collect();
            children.sort_by_key(|e| e.file_name());
            for e in children {
                let fname = match e.file_name().into_string() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let child_name = if name.is_empty() { fname } else { format!("{name}/{fname}") };
                walk(&e.path(), child_name, out);
            }
        } else if md.is_file() {
            out.push(PackEntry {
                name,
                src: p.to_path_buf(),
                is_dir: false,
                mtime: mtime_of(&md),
                mode: mode_of(&md),
                size: md.len(),
            });
        }
    }

    let mut out = Vec::new();
    for p in paths {
        walk(p, base_name(p), &mut out);
    }
    out
}

// ---------------------------------------------------------------------------
// Records (the inner stream)
// ---------------------------------------------------------------------------

fn write_rec_header<W: Write>(
    w: &mut W,
    kind: u8,
    name: &[u8],
    mtime: i64,
    mode: u32,
    size: Option<u64>,
) -> std::io::Result<()> {
    w.write_all(&[kind])?;
    w.write_all(&(name.len() as u16).to_le_bytes())?;
    w.write_all(name)?;
    w.write_all(&mtime.to_le_bytes())?;
    w.write_all(&mode.to_le_bytes())?;
    if let Some(s) = size {
        w.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SEAL (create)
// ---------------------------------------------------------------------------

fn clean_eula(text: Option<&str>) -> Result<Option<String>> {
    let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
    let t = t.replace("\r\n", "\n");
    if t.len() > MAX_EULA_LEN {
        return Err(NikError::Usage(format!(
            "The EULA is too long (at most {MAX_EULA_LEN} bytes)."
        )));
    }
    Ok(Some(t))
}

fn checked_text<'a>(value: &'a str, what: &str, max: usize) -> Result<&'a str> {
    let v = value.trim();
    if v.len() > max {
        return Err(NikError::Usage(format!("The {what} is too long (at most {max} bytes).")));
    }
    Ok(v)
}

fn checked_cover(pic: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    let Some(pic) = pic.filter(|p| !p.is_empty()) else { return Ok(None) };
    if crate::cover::kind(pic).is_none() {
        return Err(NikError::Usage("The cover must be a JPEG or PNG picture.".into()));
    }
    if pic.len() > MAX_COVER_BYTES {
        return Err(NikError::Usage("The cover picture is too large (at most 4 MB).".into()));
    }
    Ok(Some(pic.to_vec()))
}

/// Serialise a package into a `.nik` payload. `password` of `None` leaves the body
/// unencrypted (the box always asks for a password).
pub fn seal(pkg: &Package, password: Option<&str>) -> Result<Vec<u8>> {
    seal_with_rounds(pkg, password, KDF_ROUNDS)
}

fn seal_with_rounds(pkg: &Package, password: Option<&str>, rounds: u32) -> Result<Vec<u8>> {
    let name = checked_text(&pkg.name, "name", MAX_NAME_LEN)?;
    if name.is_empty() {
        return Err(NikError::Usage("Give the package a name.".into()));
    }
    let author = checked_text(&pkg.author, "author", MAX_AUTHOR_LEN)?;
    let description = checked_text(&pkg.description, "description", MAX_DESCRIPTION_LEN)?;
    let eula = clean_eula(pkg.eula.as_deref())?;
    let cover = checked_cover(pkg.cover.as_deref())?;
    if pkg.entries.is_empty() {
        return Err(NikError::NoFiles("There are no files to package.".into()));
    }

    let encrypted = match password {
        Some(pw) => {
            if pw.trim().is_empty() {
                return Err(NikError::Password(
                    "A package cannot be exported without a password.".into(),
                ));
            }
            true
        }
        None => false,
    };

    let salt = crate::crypto::random_bytes::<16>();
    let nonce = crate::crypto::random_bytes::<16>();

    // ---- header (plaintext, one JSON line) --------------------------------
    let mut header = format!(
        "{{\"format\":\"nikpak-package\",\"version\":{PACKAGE_VERSION},\"name\":\"{}\",\"count\":{},\"encrypted\":{}",
        json_escape(name),
        pkg.entries.len(),
        encrypted
    );
    if !author.is_empty() {
        header.push_str(&format!(",\"author\":\"{}\"", json_escape(author)));
    }
    if !description.is_empty() {
        header.push_str(&format!(",\"description\":\"{}\"", json_escape(description)));
    }
    if let Some(e) = &eula {
        header.push_str(&format!(",\"eula\":\"{}\"", json_escape(e)));
    }
    if let Some(pic) = &cover {
        header.push_str(&format!(
            ",\"cover\":\"{}\",\"coverType\":\"{}\"",
            b64_encode(pic),
            crate::cover::kind(pic).unwrap_or("image/jpeg")
        ));
    }
    if encrypted {
        header.push_str(&format!(",\"salt\":\"{}\",\"nonce\":\"{}\"", hex(&salt), hex(&nonce)));
    }
    header.push_str(",\"cipher\":\"sha256-ctr\",\"compression\":\"zstd\"}");
    let header_bytes = header.as_bytes().to_vec();

    // ---- body: zstd of the record stream ----------------------------------
    let mut body = Vec::new();
    {
        let mut enc = zstd::stream::write::Encoder::new(&mut body, 19)
            .map_err(|e| NikError::Fatal(format!("Cannot start the compressor: {e}")))?;
        for e in &pkg.entries {
            let name_bytes = e.name.as_bytes();
            if name_bytes.is_empty() || name_bytes.len() > MAX_ENTRY_NAME_LEN {
                return Err(NikError::Usage("An entry name is missing or too long.".into()));
            }
            if e.is_dir {
                write_rec_header(&mut enc, REC_DIR, name_bytes, e.mtime, e.mode, None)
                    .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
                continue;
            }
            let mut f = std::fs::File::open(&e.src)
                // `err`, not `e`: `e` is the entry the loop walks
                .map_err(|err| NikError::Fatal(format!("Cannot read '{}': {err}", e.src.display())))?;
            write_rec_header(&mut enc, REC_FILE, name_bytes, e.mtime, e.mode, Some(e.size))
                .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
            let mut crc = crc32fast::Hasher::new();
            let mut remaining = e.size;
            let mut buf = vec![0u8; 256 * 1024];
            while remaining > 0 {
                let want = remaining.min(buf.len() as u64) as usize;
                let n = f.read(&mut buf[..want])
                    .map_err(|err| NikError::Fatal(format!("Read error on '{}': {err}", e.src.display())))?;
                if n == 0 {
                    return Err(NikError::Fatal(format!(
                        "'{}' became shorter while it was being packaged.",
                        e.src.display()
                    )));
                }
                crc.update(&buf[..n]);
                enc.write_all(&buf[..n])
                    .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
                remaining -= n as u64;
            }
            enc.write_all(&crc.finalize().to_le_bytes())
                .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
        }
        enc.write_all(&[REC_END])
            .and_then(|_| enc.write_all(&(pkg.entries.len() as u64).to_le_bytes()))
            .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
        enc.finish()
            .map_err(|e| NikError::Fatal(format!("Cannot finish the package: {e}")))?;
    }

    // ---- seal -------------------------------------------------------------
    let mut out = Vec::with_capacity(MAGIC.len() + 1 + header.len() + body.len() + 32);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&header_bytes);
    out.push(b'\n');
    if encrypted {
        let key = derive(password.unwrap(), &salt, rounds);
        keystream_xor(&key, &nonce, &mut body);
        let tag = tag_hmac(&key, &[header_bytes.as_slice(), body.as_slice()]);
        out.extend_from_slice(&body);
        out.extend_from_slice(&tag);
    } else {
        out.extend_from_slice(&body);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Split + inspect
// ---------------------------------------------------------------------------

/// What a package says about itself before its password is asked for.
pub fn inspect(bytes: &[u8]) -> Result<PackageInfo> {
    let (header, _, _) = split(bytes)?;
    info_from_header(header)
}

fn info_from_header(h: &str) -> Result<PackageInfo> {
    Ok(PackageInfo {
        name: json_field(h, "name").unwrap_or_else(|| "Package".into()),
        author: json_field(h, "author").unwrap_or_default(),
        description: json_field(h, "description").unwrap_or_default(),
        eula: json_field(h, "eula"),
        count: json_field(h, "count").and_then(|c| c.parse().ok()).unwrap_or(0),
        cover: json_field(h, "cover").and_then(|b| b64_decode(&b)),
        cover_type: json_field(h, "coverType"),
        encrypted: json_field(h, "encrypted").as_deref() == Some("true"),
    })
}

/// Split a payload into its header text, its body and (when encrypted) its tag.
fn split(bytes: &[u8]) -> Result<(&str, &[u8], Option<&[u8]>)> {
    if bytes.len() < MAGIC.len() + 2 || &bytes[..MAGIC.len()] != MAGIC {
        return Err(NikError::Corrupt("Not a NikPak package (.nik).".into()));
    }
    let rest = &bytes[MAGIC.len()..];
    let nl = rest
        .iter()
        .position(|b| *b == b'\n')
        .ok_or_else(|| NikError::Corrupt("The package has no header.".into()))?;
    if nl > MAX_HEADER {
        return Err(NikError::Corrupt("The package header is too large.".into()));
    }
    let header = std::str::from_utf8(&rest[..nl])
        .map_err(|_| NikError::Corrupt("The package header is not text.".into()))?;
    let body_and_tag = &rest[nl + 1..];
    let encrypted = json_field(header, "encrypted").as_deref() == Some("true");
    if json_field(header, "format").as_deref() != Some("nikpak-package") {
        return Err(NikError::Corrupt("Not a NikPak package (.nik).".into()));
    }
    if encrypted {
        if body_and_tag.len() < 32 {
            return Err(NikError::Corrupt("The package is truncated.".into()));
        }
        let (body, tag) = body_and_tag.split_at(body_and_tag.len() - 32);
        Ok((header, body, Some(tag)))
    } else {
        Ok((header, body_and_tag, None))
    }
}

// ---------------------------------------------------------------------------
// Decode (verify + decrypt + decompress) the record stream
// ---------------------------------------------------------------------------

/// Decode a package's body back to the record stream, after checking the EULA was
/// approved and the password is right. Fails cleanly — and identically — on a wrong
/// password and on a damaged file.
pub fn decode(bytes: &[u8], password: &str, approved: Option<&str>) -> Result<Vec<u8>> {
    decode_with_rounds(bytes, password, approved, KDF_ROUNDS)
}

fn decode_with_rounds(
    bytes: &[u8],
    password: &str,
    approved: Option<&str>,
    rounds: u32,
) -> Result<Vec<u8>> {
    let (header, body, tag) = split(bytes)?;

    let eula = json_field(header, "eula");
    if let Some(terms) = &eula {
        if approved != Some(terms.as_str()) {
            return Err(NikError::Legal(
                "This package's EULA must be approved before it is opened.".into(),
            ));
        }
    }

    let plain = if json_field(header, "encrypted").as_deref() == Some("true") {
        let salt = from_hex(json_field(header, "salt").as_deref().unwrap_or(""))
            .ok_or_else(|| NikError::Corrupt("The package has no salt.".into()))?;
        let nonce = from_hex(json_field(header, "nonce").as_deref().unwrap_or(""))
            .ok_or_else(|| NikError::Corrupt("The package has no nonce.".into()))?;
        if salt.len() != 16 || nonce.len() != 16 {
            return Err(NikError::Corrupt("The package header is damaged.".into()));
        }
        let key = derive(password, &salt, rounds);
        let want = tag_hmac(&key, &[header.as_bytes(), body]);
        let tag = tag.ok_or_else(|| NikError::Corrupt("The package is truncated.".into()))?;
        if !ct_eq(&want, tag) {
            return Err(NikError::Password(
                "Wrong password (or the package is damaged).".into(),
            ));
        }
        let mut p = body.to_vec();
        keystream_xor(&key, &nonce, &mut p);
        p
    } else {
        body.to_vec()
    };

    // decompress the record stream
    let dec = zstd::stream::read::Decoder::new(Cursor::new(&plain))
        .map_err(|e| NikError::Corrupt(format!("The package body is not readable: {e}")))?;
    let mut out = Vec::new();
    dec.take(MAX_BODY + 1)
        .read_to_end(&mut out)
        .map_err(|e| NikError::Corrupt(format!("The package body is not readable: {e}")))?;
    if out.len() as u64 > MAX_BODY {
        return Err(NikError::Corrupt("The package is too large to open.".into()));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// EXTRACT
// ---------------------------------------------------------------------------

fn read_u8<R: Read>(r: &mut R) -> std::io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}
fn read_u16<R: Read>(r: &mut R) -> std::io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}
fn read_u32<R: Read>(r: &mut R) -> std::io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}
fn read_u64<R: Read>(r: &mut R) -> std::io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn unique_path(p: &Path) -> PathBuf {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    for i in 1..10_000 {
        let c = dir.join(format!("{stem} ({i}){ext}"));
        if !c.exists() && std::fs::symlink_metadata(&c).is_err() {
            return c;
        }
    }
    p.to_path_buf()
}

/// Open a package and write its files into `dest`, preserving the folder structure
/// inside the package. `approved` is the EULA the user accepted, exactly as
/// [`inspect`] returned it. Returns the number of files written.
pub fn extract(path: &Path, password: &str, approved: Option<&str>, dest: &Path) -> Result<u64> {
    let bytes = std::fs::read(path)
        .map_err(|e| NikError::Fatal(format!("Cannot open '{}': {e}", path.display())))?;
    extract_bytes(&bytes, password, approved, dest)
}

pub fn extract_bytes(bytes: &[u8], password: &str, approved: Option<&str>, dest: &Path) -> Result<u64> {
    let (header, _, _) = split(bytes)?;
    let info = info_from_header(header)?;
    let stream = decode(bytes, password, approved)?;

    if dest.exists() && !dest.is_dir() {
        return Err(NikError::Fatal(format!("Destination '{}' is not a folder.", dest.display())));
    }
    std::fs::create_dir_all(dest)
        .map_err(|e| NikError::Fatal(format!("Cannot create destination '{}': {e}", dest.display())))?;

    let mut r = Cursor::new(stream);
    let mut buf = vec![0u8; 256 * 1024];
    // files written (the number the window reports)
    let mut written = 0u64;
    // every record read, folders included - what the writer counted in REC_END
    let mut records = 0u64;
    let mut seen: HashSet<String> = HashSet::new();

    let declared = loop {
        let kind = read_u8(&mut r).map_err(|e| from_io(e, "Reading package"))?;
        match kind {
            REC_END => break read_u64(&mut r).map_err(|e| from_io(e, "Reading package"))?,
            REC_FILE | REC_DIR => {
                let nlen = read_u16(&mut r).map_err(|e| from_io(e, "Reading package"))? as usize;
                if nlen > MAX_ENTRY_NAME_LEN {
                    return Err(NikError::Corrupt("The package is damaged (bad name length).".into()));
                }
                let mut nb = vec![0u8; nlen];
                r.read_exact(&mut nb).map_err(|e| from_io(e, "Reading package"))?;
                let name = String::from_utf8(nb)
                    .map_err(|_| NikError::Corrupt("The package is damaged (invalid name).".into()))?;
                let mtime = read_u64(&mut r).map_err(|e| from_io(e, "Reading package"))? as i64;
                // the permission bits are recorded but not used when extracting a package
                let _mode = read_u32(&mut r).map_err(|e| from_io(e, "Reading package"))?;
                let is_dir = kind == REC_DIR;
                let size = if is_dir { 0 } else { read_u64(&mut r).map_err(|e| from_io(e, "Reading package"))? };
                records += 1;

                let comps = match crate::util::sanitize_entry_name(&name) {
                    Ok(c) => c,
                    Err(why) => {
                        return Err(NikError::Corrupt(format!(
                            "The package contains an unsafe entry '{name}': {why}"
                        )))
                    }
                };
                if is_dir {
                    let p = crate::util::join_components(dest, &comps);
                    std::fs::create_dir_all(&p).map_err(|e| {
                        NikError::Fatal(format!("Cannot create folder '{}': {e}", p.display()))
                    })?;
                    continue;
                }

                if let Err(why) = crate::util::check_no_symlinks(dest, &comps) {
                    return Err(NikError::Fatal(format!("Refusing to write '{name}': {why}")));
                }
                let mut target = crate::util::join_components(dest, &comps);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        NikError::Fatal(format!("Cannot create folder '{}': {e}", parent.display()))
                    })?;
                }
                if std::fs::symlink_metadata(&target).is_ok() {
                    target = unique_path(&target);
                }
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)
                    .map_err(|e| NikError::Fatal(format!("Cannot create '{}': {e}", target.display())))?;
                let mut crc = crc32fast::Hasher::new();
                let mut remaining = size;
                while remaining > 0 {
                    let want = remaining.min(buf.len() as u64) as usize;
                    let n = r.read(&mut buf[..want]).map_err(|e| from_io(e, "Reading package"))?;
                    if n == 0 {
                        drop(f);
                        let _ = std::fs::remove_file(&target);
                        return Err(NikError::Corrupt("The package is damaged (truncated file).".into()));
                    }
                    crc.update(&buf[..n]);
                    f.write_all(&buf[..n]).map_err(|e| {
                        let _ = std::fs::remove_file(&target);
                        NikError::Fatal(format!("Write error on '{}': {e}", target.display()))
                    })?;
                    remaining -= n as u64;
                }
                let stored = read_u32(&mut r).map_err(|e| {
                    let _ = std::fs::remove_file(&target);
                    from_io(e, "Reading package")
                })?;
                if stored != crc.finalize() {
                    drop(f);
                    let _ = std::fs::remove_file(&target);
                    return Err(NikError::Corrupt(format!(
                        "CRC mismatch in '{}': the package is damaged. The file was not kept.",
                        name
                    )));
                }
                if mtime >= 0 {
                    let _ = f.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(mtime as u64));
                }
                drop(f);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode((_mode & 0o777) | 0o400));
                }
                written += 1;
                seen.insert(name);
            }
            _ => return Err(NikError::Corrupt("The package is damaged (unknown record).".into())),
        }
    };
    // The writer stores the number of *entries* (folders included) after
    // REC_END, so it is compared with every record read - not with `written`,
    // which counts files only. (Comparing it with `written` made every package
    // that contains a folder fail to extract: "Entry count mismatch".)
    if declared != records {
        return Err(NikError::Corrupt("Entry count mismatch: the package is damaged.".into()));
    }

    // The EULA stays with the extracted files, like Nikkosynth writes its terms
    // beside an imported bank.
    if let Some(terms) = &info.eula {
        let mut text = format!("{}\n\n", info.name);
        if !info.description.trim().is_empty() {
            text.push_str(info.description.trim());
            text.push_str("\n\n");
        }
        text.push_str(terms);
        text.push('\n');
        let _ = std::fs::write(dest.join("Terms of use.txt"), text);
    }

    let _ = seen;
    Ok(written)
}

fn from_io(e: std::io::Error, ctx: &str) -> NikError {
    crate::error::from_io(e, ctx)
}

// ---------------------------------------------------------------------------
// Reading a package from disk
// ---------------------------------------------------------------------------

/// Is this file a NikPak package? Reads the first eight bytes only.
pub fn is_package(path: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut magic = [0u8; 8];
    f.read_exact(&mut magic).is_ok() && &magic == MAGIC
}

/// Read a package's header (name, author, description, EULA, cover, count) without
/// its password — what the box shows before asking for it. Reads only the header
/// line, never the body, so it stays cheap even for a large package.
pub fn read_info(path: &Path) -> Result<PackageInfo> {
    let mut f = std::fs::File::open(path)
        .map_err(|e| NikError::Fatal(format!("Cannot open '{}': {e}", path.display())))?;
    let mut head = Vec::with_capacity(4096);
    let mut buf = [0u8; 8192];
    let mut nl = false;
    while head.len() < MAX_HEADER {
        let n = f
            .read(&mut buf)
            .map_err(|e| NikError::Fatal(format!("Cannot read '{}': {e}", path.display())))?;
        if n == 0 {
            break;
        }
        if let Some(at) = buf[..n].iter().position(|b| *b == b'\n') {
            head.extend_from_slice(&buf[..at]);
            nl = true;
            break;
        }
        head.extend_from_slice(&buf[..n]);
    }
    if !nl {
        return Err(NikError::Corrupt("The package has no readable header.".into()));
    }
    if head.len() < MAGIC.len() || &head[..MAGIC.len()] != MAGIC {
        return Err(NikError::Corrupt("Not a NikPak package.".into()));
    }
    let text = std::str::from_utf8(&head[MAGIC.len()..])
        .map_err(|_| NikError::Corrupt("The package header is not text.".into()))?;
    info_from_header(text)
}

/// The one-sentence answer for anything that needs one line.
pub fn describe(info: &PackageInfo) -> String {
    let mut s = format!("This is a NikPak package ({} file{}", info.count, if info.count == 1 { "" } else { "s" });
    if !info.author.trim().is_empty() {
        s.push_str(&format!(", by {}", info.author.trim()));
    }
    s.push_str("), not a NikPak archive. Open it with unNik or NikPak to unpack it.");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory for one test. **One per call**: `cargo test` runs these
    /// in parallel, and with a directory shared by the whole module the tests
    /// deleted each other's files halfway through - a round trip then "lost" an
    /// entry, and a wrong-password test got a read error instead of the
    /// password error it was looking for.
    fn unique_dir(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("nikpak-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> (PathBuf, Package) {
        let dir = unique_dir("pkg-src");
        std::fs::create_dir_all(dir.join("Docs/sub")).unwrap();
        std::fs::write(dir.join("Docs/a.txt"), "alpha\n".repeat(400)).unwrap();
        std::fs::write(dir.join("Docs/sub/b.txt"), "beta\n".repeat(300)).unwrap();
        std::fs::write(dir.join("bin.dat"), (0..5000u32).map(|i| (i * 7 % 251) as u8).collect::<Vec<_>>()).unwrap();
        let entries = collect(&[dir.join("Docs"), dir.join("bin.dat")]);
        let pkg = Package {
            name: "Mountains".into(),
            author: "Nicolas Choukroun".into(),
            description: "Cold pads and bells.".into(),
            eula: None,
            cover: None,
            entries,
        };
        (dir, pkg)
    }

    fn seal_fast(pkg: &Package, pw: Option<&str>) -> Vec<u8> {
        seal_with_rounds(pkg, pw, 2).unwrap()
    }

    fn decode_fast(bytes: &[u8], pw: &str, approved: Option<&str>) -> Result<Vec<u8>> {
        decode_with_rounds(bytes, pw, approved, 2)
    }

    fn extract_fast(bytes: &[u8], pw: &str, approved: Option<&str>, dest: &Path) -> Result<u64> {
        extract_bytes(bytes, pw, approved, dest)
    }

    #[test]
    fn a_package_round_trips() {
        let (dir, pkg) = sample();
        let bytes = seal_fast(&pkg, Some("correct horse"));
        let info = inspect(&bytes).unwrap();
        assert_eq!(info.name, "Mountains");
        assert_eq!(info.author, "Nicolas Choukroun");
        assert_eq!(info.count, pkg.entries.len());
        assert!(info.encrypted);

        let out = dir.join("out");
        let n = extract_fast(&bytes, "correct horse", None, &out).unwrap();
        assert_eq!(n, 3);
        assert_eq!(std::fs::read_to_string(out.join("Docs/a.txt")).unwrap(), "alpha\n".repeat(400));
        assert_eq!(std::fs::read_to_string(out.join("Docs/sub/b.txt")).unwrap(), "beta\n".repeat(300));
        assert_eq!(std::fs::read(out.join("bin.dat")).unwrap(), (0..5000u32).map(|i| (i * 7 % 251) as u8).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_password_fails_cleanly() {
        let (dir, pkg) = sample();
        let bytes = seal_fast(&pkg, Some("open sesame"));
        let err = decode_fast(&bytes, "open sesamf", None).unwrap_err();
        assert!(matches!(err, NikError::Password(_)));
        assert!(err.to_string().contains("Wrong password"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_eula_must_be_approved() {
        const TERMS: &str = "Licensed to one user.\nNo resale.";
        let (dir, mut pkg) = sample();
        pkg.eula = Some(format!("  {TERMS}\r\n"));
        let bytes = seal_fast(&pkg, Some("k3y"));
        let info = inspect(&bytes).unwrap();
        assert_eq!(info.eula.as_deref(), Some(TERMS));

        assert!(matches!(
            decode_fast(&bytes, "k3y", None),
            Err(NikError::Legal(_))
        ));
        assert!(decode_fast(&bytes, "k3y", Some("other words")).is_err());
        assert!(decode_fast(&bytes, "k3y", Some(TERMS)).is_ok());

        // changing the EULA on disk breaks the package
        let at = bytes.windows(7).position(|w| w == b"No resa").unwrap();
        let mut forged = bytes.clone();
        forged[at] = b'n';
        let changed = inspect(&forged).unwrap().eula.unwrap();
        assert!(decode_fast(&forged, "k3y", Some(&changed)).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tampered_byte_is_rejected() {
        let (dir, pkg) = sample();
        let mut bytes = seal_fast(&pkg, Some("k3y"));
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xFF;
        assert!(decode_fast(&bytes, "k3y", None).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_readable_survives_in_an_encrypted_package() {
        let (dir, pkg) = sample();
        let bytes = seal_fast(&pkg, Some("k3y"));
        let hay = String::from_utf8_lossy(&bytes);
        assert!(!hay.contains("alpha"), "a file name leaked in plaintext");
        assert!(!hay.contains("Docs"), "a folder name leaked in plaintext");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unencrypted_package_is_smaller_and_opens() {
        let (dir, pkg) = sample();
        let bytes = seal_fast(&pkg, None);
        let info = inspect(&bytes).unwrap();
        assert!(!info.encrypted);
        let out = dir.join("out");
        assert_eq!(extract_fast(&bytes, "", None, &out).unwrap(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_cover_travels_with_the_package() {
        let jpeg: Vec<u8> = [0xFF, 0xD8, 0xFF, 0xE0]
            .iter()
            .copied()
            .chain((0..3000).map(|i| (i % 251) as u8))
            .collect();
        let (dir, mut pkg) = sample();
        pkg.cover = Some(jpeg.clone());
        let bytes = seal_fast(&pkg, Some("k3y"));
        let info = inspect(&bytes).unwrap();
        assert_eq!(info.cover_type.as_deref(), Some("image/jpeg"));
        assert_eq!(info.cover.as_deref(), Some(jpeg.as_slice()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unsafe_entry_name_cannot_escape_the_destination() {
        let dir = unique_dir("pkg-esc");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x"), "hi").unwrap();
        let pkg = Package {
            name: "Evil".into(),
            author: String::new(),
            description: String::new(),
            eula: None,
            cover: None,
            entries: vec![PackEntry {
                name: "../../escape.txt".into(),
                src: dir.join("x"),
                is_dir: false,
                mtime: 0,
                mode: 0o644,
                size: 2,
            }],
        };
        let bytes = seal_fast(&pkg, Some("k3y"));
        let out = dir.join("out");
        // `..` is refused outright rather than silently re-homed
        assert!(extract_fast(&bytes, "k3y", None, &out).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_base64_codec_round_trips() {
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
        assert_eq!(b64_decode("TWFu").unwrap(), b"Man");
        assert!(b64_decode("TW#u").is_none());
    }

    #[test]
    fn the_kdf_is_not_trivially_cheap() {
        // A smoke test, not a benchmark: with SHA-NI a round is very fast, so
        // 600 000 of them legitimately take well under 100 ms on a current
        // CPU. What must never happen is the loop being skipped (a wrong
        // `rounds`, or `1..rounds` turned into a single pass) - that would
        // make a guess almost free.
        let salt = [7u8; 16];
        let t = std::time::Instant::now();
        let _ = derive("password", &salt, KDF_ROUNDS);
        let full = t.elapsed();
        let t = std::time::Instant::now();
        let _ = derive("password", &salt, 1);
        let once = t.elapsed();
        assert!(
            full > once * 10 && full > std::time::Duration::from_millis(20),
            "600 000 rounds took {full:?} against {once:?} for one: the KDF is too cheap"
        );
    }

    #[test]
    fn a_foreign_file_is_not_a_package() {
        assert!(inspect(b"PK\x03\x04 not a package").is_err());
        assert!(inspect(b"NIKPACK1 then some bank bytes").is_err());
        assert!(inspect(&[]).is_err());
    }
}
