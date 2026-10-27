//! nikpak-keygen - Nikkoid's private licence tool. NOT part of the public
//! download (it is only built with  --features keygen).
//!
//!   cargo run --release --features keygen --bin nikpak-keygen -- newkeys
//!       creates keys/nikpak-private.key (SECRET) and src/license_pubkey.rs;
//!       rebuild nikpak afterwards so it contains the new public key.
//!   cargo run --release --features keygen --bin nikpak-keygen -- make \
//!       --name "Jane Doe" --email jane@example.com [--company --seats 25] [--file nikpak.key]
//!   cargo run --release --features keygen --bin nikpak-keygen -- verify --name .. --email .. --key ..
//!
//! The private key is read from keys/nikpak-private.key or from the file named
//! by the NIKPAK_PRIVATE_KEY environment variable.

#[path = "../license.rs"]
#[allow(dead_code)]
mod license;

use ed25519_dalek::{Signer, SigningKey};
use license::Kind;
use rand::RngCore;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn private_key_path() -> PathBuf {
    std::env::var_os("NIKPAK_PRIVATE_KEY").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("keys/nikpak-private.key"))
}

fn read_seed(p: &Path) -> Result<[u8; 32], String> {
    let t = std::fs::read_to_string(p).map_err(|e| format!("Cannot read the private key {}: {e}", p.display()))?;
    let hex: String = t.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<String>().split_whitespace().collect();
    if hex.len() != 64 {
        return Err("The private key file must contain 64 hexadecimal digits.".into());
    }
    let mut seed = [0u8; 32];
    for i in 0..32 {
        seed[i] = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|_| "Invalid hexadecimal in the private key.")?;
    }
    Ok(seed)
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(|s| s.as_str()) {
        Some("newkeys") => newkeys(args.iter().any(|a| a == "--force")),
        Some("make") => make(&args),
        Some("verify") => verify(&args),
        _ => {
            eprintln!("usage: nikpak-keygen newkeys [--force] | make --name N --email E [--company --seats N] [--file F] | verify --name N --email E --key K");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::from(1)
        }
    }
}

fn newkeys(force: bool) -> Result<(), String> {
    let kp = private_key_path();
    if kp.exists() && !force {
        return Err(format!(
            "{} already exists. Replacing it invalidates every licence key sold so far; use --force only if you really mean it.",
            kp.display()
        ));
    }
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    let sk = SigningKey::from_bytes(&seed);
    let pk = sk.verifying_key().to_bytes();
    if let Some(d) = kp.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let hex: String = seed.iter().map(|b| format!("{b:02x}")).collect();
    std::fs::write(
        &kp,
        format!("# NikPak licence PRIVATE key (Ed25519 seed). KEEP SECRET: whoever has it can create licence keys.\n# Copy it to the licence server (server/), keep an offline backup, never publish it.\n{hex}\n"),
    )
    .map_err(|e| e.to_string())?;
    let list: Vec<String> = pk.iter().map(|b| format!("0x{b:02x}")).collect();
    std::fs::write(
        "src/license_pubkey.rs",
        format!(
            "// NikPak licence PUBLIC key, written by `nikpak-keygen newkeys`.\n// Its private half is keys/nikpak-private.key (secret, on the licence server).\npub const PUBLIC_KEY: [u8; 32] = [\n    {}\n];\n",
            list.chunks(8).map(|c| c.join(", ")).collect::<Vec<_>>().join(",\n    ")
        ),
    )
    .map_err(|e| e.to_string())?;
    println!("New key pair created:\n  private (SECRET): {}\n  public:           src/license_pubkey.rs\nRebuild nikpak so it contains the new public key.", kp.display());
    Ok(())
}

fn make(args: &[String]) -> Result<(), String> {
    let name = arg(args, "--name").ok_or("--name is required")?;
    let email = arg(args, "--email").ok_or("--email is required")?;
    let company = args.iter().any(|a| a == "--company");
    let seats: u16 = arg(args, "--seats").map(|s| s.parse().map_err(|_| "--seats must be a number")).transpose()?.unwrap_or(1);
    if seats == 0 {
        return Err("--seats must be at least 1".into());
    }
    let kind = if company { Kind::Company } else { Kind::Personal };
    let sk = SigningKey::from_bytes(&read_seed(&private_key_path())?);
    let sig = sk.sign(&license::message(kind, seats, &name, &email)).to_bytes();
    let key = license::format_key(kind, seats, &sig);
    if sk.verifying_key().to_bytes() != license::PUBLIC_KEY {
        eprintln!("WARNING: this private key does not belong to the public key built into this source tree.");
    }
    println!("{key}");
    if let Some(f) = arg(args, "--file") {
        std::fs::write(&f, license::file_text(&name, &email, &key)).map_err(|e| e.to_string())?;
        eprintln!("licence file written: {f}");
    }
    Ok(())
}

fn verify(args: &[String]) -> Result<(), String> {
    let (n, e, k) = (arg(args, "--name").ok_or("--name")?, arg(args, "--email").ok_or("--email")?, arg(args, "--key").ok_or("--key")?);
    let l = license::verify(&n, &e, &k).map_err(|e| e.to_string())?;
    println!("valid: {}", l.describe());
    Ok(())
}
