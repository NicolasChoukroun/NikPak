//! Shareware licensing.
//!
//! A licence key is an Ed25519 signature, made with Nikkoid's private key on
//! the licence server, over the customer's name, e-mail address, licence type
//! and number of seats. nikpak only contains the PUBLIC key: it can check a
//! key but never create one, so key generators are impossible without the
//! private key (which never leaves the server).
//!
//! Key format: `NPK1-` followed by 108 Crockford base32 characters (in groups
//! of five) encoding  type:u8  seats:u16 (big endian)  signature[64].
//!
//! Signed message:
//!   "NIKPAK-LICENSE-1\n" type:u8 seats:u16be normalize(name) "\n" normalize(email)
//! normalize(): ASCII whitespace runs become one space, trimmed, ASCII letters
//! lower-cased (identical in the PHP library in server/).
//!
//! A licence is found, in this order, in `nikpak.key` next to the executable
//! (handy for companies deploying to many PCs, like RAR's rarreg.key) and in
//! the per-user settings folder (written by the registration box or by
//! `nikpak register`).
//!
//! This file is also compiled into the nikpak-keygen tool (src/bin), so it
//! must not use other modules of the crate.

use std::path::PathBuf;

include!("license_pubkey.rs");
// keys blocked in the licence manager (refunds, keys published on the Internet)
include!("license_revoked.rs");

pub const BUY_URL: &str = "https://nikpak.nikkoid.com/buy.php";
pub const PRICE_PERSONAL: &str = "$19";
pub const PRICE_COMPANY: &str = "from $5 per seat";

const CONTEXT: &[u8] = b"NIKPAK-LICENSE-1\n";
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PREFIX: &str = "NPK1";
const PAYLOAD: usize = 3 + 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Personal = 1,
    Company = 2,
}

impl Kind {
    pub fn from_u8(b: u8) -> Option<Kind> {
        match b {
            1 => Some(Kind::Personal),
            2 => Some(Kind::Company),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct License {
    pub name: String,
    pub email: String,
    pub kind: Kind,
    pub seats: u16,
}

impl License {
    pub fn describe(&self) -> String {
        match self.kind {
            Kind::Personal => format!("{} (personal licence)", self.name),
            Kind::Company if self.seats == 1 => format!("{} (company licence, 1 seat)", self.name),
            Kind::Company => format!("{} (company licence, {} seats)", self.name, self.seats),
        }
    }
}

pub fn normalize(s: &str) -> String {
    s.split_ascii_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
}

pub fn message(kind: Kind, seats: u16, name: &str, email: &str) -> Vec<u8> {
    let mut m = CONTEXT.to_vec();
    m.push(kind as u8);
    m.extend_from_slice(&seats.to_be_bytes());
    m.extend_from_slice(normalize(name).as_bytes());
    m.push(b'\n');
    m.extend_from_slice(normalize(email).as_bytes());
    m
}

#[allow(dead_code)] // used by nikpak-keygen and the tests
fn base32(data: &[u8]) -> String {
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in data {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn unbase32(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.chars() {
        let c = match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        };
        let v = ALPHABET.iter().position(|&a| a as char == c)? as u32;
        acc = (acc << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
        acc &= (1 << bits) - 1;
    }
    if acc != 0 {
        return None; // padding bits must be zero
    }
    Some(out)
}

/// Format a signed payload as a licence key.
#[allow(dead_code)] // used by nikpak-keygen and the tests
pub fn format_key(kind: Kind, seats: u16, sig: &[u8; 64]) -> String {
    let mut p = vec![kind as u8];
    p.extend_from_slice(&seats.to_be_bytes());
    p.extend_from_slice(sig);
    let b = base32(&p);
    let groups: Vec<String> = b.as_bytes().chunks(5).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
    format!("{PREFIX}-{}", groups.join("-"))
}

/// Parse a key typed or pasted by the user (spaces, dashes, line breaks and
/// lower case are accepted).
pub fn parse_key(key: &str) -> Option<(Kind, u16, [u8; 64])> {
    let compact: String = key.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let up = compact.to_ascii_uppercase();
    let body = up.strip_prefix(PREFIX)?;
    let p = unbase32(body)?;
    if p.len() != PAYLOAD {
        return None;
    }
    let kind = Kind::from_u8(p[0])?;
    let seats = u16::from_be_bytes([p[1], p[2]]);
    if seats == 0 {
        return None;
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&p[3..]);
    Some((kind, seats, sig))
}

/// Short fingerprint of a key (its signature), used for the list of blocked keys.
pub fn fingerprint(sig: &[u8; 64]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(sig)[..12].iter().map(|b| format!("{b:02x}")).collect()
}

/// Check name + e-mail + key against the public key.
pub fn verify(name: &str, email: &str, key: &str) -> Result<License, &'static str> {
    use ed25519_dalek::{Signature, VerifyingKey};
    if normalize(name).is_empty() || normalize(email).is_empty() {
        return Err("Please enter your name and your e-mail address.");
    }
    let (kind, seats, sig) = parse_key(key).ok_or("This licence key is not valid (check that it was copied completely).")?;
    let vk = VerifyingKey::from_bytes(&PUBLIC_KEY).map_err(|_| "Internal error: invalid public key.")?;
    vk.verify_strict(&message(kind, seats, name, email), &Signature::from_bytes(&sig))
        .map_err(|_| "Name, e-mail address and licence key do not match.")?;
    if REVOKED.contains(&fingerprint(&sig).as_str()) {
        return Err("This licence key has been blocked. Please contact us.");
    }
    Ok(License { name: name.split_ascii_whitespace().collect::<Vec<_>>().join(" "), email: normalize(email), kind, seats })
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

fn config_dir() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("nikpak"))
}

/// Per-user licence file written by the registration box / `nikpak register`.
pub fn user_license_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("NIKPAK_LICENSE_FILE") {
        return Some(PathBuf::from(p));
    }
    config_dir().map(|d| d.join("license.txt"))
}

/// `nikpak.key` next to the executable (deployed by companies).
pub fn exe_key_path() -> Option<PathBuf> {
    if std::env::var_os("NIKPAK_LICENSE_FILE").is_some() {
        return None; // tests: only the explicit file
    }
    std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("nikpak.key")))
}

pub fn file_text(name: &str, email: &str, key: &str) -> String {
    format!(
        "NikPak licence\r\nname={}\r\nemail={}\r\nkey={}\r\n",
        name.trim(),
        email.trim(),
        key.split_whitespace().collect::<String>()
    )
}

fn read_file(p: &std::path::Path) -> Option<License> {
    let text = std::fs::read_to_string(p).ok()?;
    let get = |k: &str| text.lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().to_string()));
    let (name, email, key) = (get("name=")?, get("email=")?, get("key=")?);
    verify(&name, &email, &key).ok()
}

/// The valid licence installed on this machine, if any.
pub fn current() -> Option<License> {
    [exe_key_path(), user_license_path()].into_iter().flatten().find_map(|p| read_file(&p))
}

/// Verify and store a licence for this user.
pub fn register(name: &str, email: &str, key: &str) -> Result<License, String> {
    let lic = verify(name, email, key).map_err(|e| e.to_string())?;
    let path = user_license_path().ok_or("Cannot find a folder to store the licence.")?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("Cannot store the licence: {e}"))?;
    }
    std::fs::write(&path, file_text(name, email, key)).map_err(|e| format!("Cannot store the licence: {e}"))?;
    Ok(lic)
}

/// Remove the per-user licence (`nikpak unregister`).
pub fn unregister() -> bool {
    user_license_path().map(|p| std::fs::remove_file(p).is_ok()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn sign(seed: [u8; 32], kind: Kind, seats: u16, name: &str, email: &str) -> (String, [u8; 32]) {
        let sk = SigningKey::from_bytes(&seed);
        let sig = sk.sign(&message(kind, seats, name, email)).to_bytes();
        (format_key(kind, seats, &sig), sk.verifying_key().to_bytes())
    }

    #[test]
    fn base32_roundtrip() {
        for n in [0usize, 1, 2, 3, 4, 5, 66, 67, 68] {
            let d: Vec<u8> = (0..n as u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect();
            assert_eq!(unbase32(&base32(&d)).unwrap(), d);
        }
        assert_eq!(base32(&[0u8; 67]).len(), 108);
    }

    #[test]
    fn keys_verify_and_reject() {
        use ed25519_dalek::{Signature, VerifyingKey};
        let (key, pk) = sign([7u8; 32], Kind::Company, 25, "Jane  Doe", "Jane@Example.com");
        assert!(key.starts_with("NPK1-"));
        let (kind, seats, sig) = parse_key(&key).unwrap();
        assert_eq!((kind, seats), (Kind::Company, 25));
        let vk = VerifyingKey::from_bytes(&pk).unwrap();
        // tolerant matching: case, spaces, dashes, lower-case key
        let m = message(kind, seats, " jane doe ", "jane@example.COM");
        assert!(vk.verify_strict(&m, &Signature::from_bytes(&sig)).is_ok());
        let pasted = key.to_lowercase().replace('-', " ");
        assert_eq!(parse_key(&pasted).unwrap().2, sig);
        // anything else must fail
        for (n, e) in [("Jane Doex", "jane@example.com"), ("Jane Doe", "jane@example.org")] {
            assert!(vk.verify_strict(&message(kind, seats, n, e), &Signature::from_bytes(&sig)).is_err());
        }
        assert!(vk.verify_strict(&message(Kind::Company, 26, "Jane Doe", "jane@example.com"), &Signature::from_bytes(&sig)).is_err());
        // a flipped character breaks the key
        let mut bad: Vec<char> = key.chars().collect();
        let i = bad.len() - 3;
        bad[i] = if bad[i] == 'A' { 'B' } else { 'A' };
        let bad: String = bad.into_iter().collect();
        if let Some((k2, s2, sig2)) = parse_key(&bad) {
            assert!(vk.verify_strict(&message(k2, s2, "Jane Doe", "jane@example.com"), &Signature::from_bytes(&sig2)).is_err());
        }
        assert!(parse_key("NPK1-ABC").is_none());
        assert!(parse_key("hello").is_none());
    }

    #[test]
    fn product_key_rejects_foreign_signatures() {
        // a key signed with any other private key is refused by the built-in public key
        let (key, _) = sign([9u8; 32], Kind::Personal, 1, "Mallory", "m@example.com");
        assert!(verify("Mallory", "m@example.com", &key).is_err());
    }
}
