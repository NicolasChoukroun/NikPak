//! unNik - the free browser and extractor for .nik archives (no registration).
//!
//!   unNik                       opens the window: drop a .nik archive on it
//!   unNik archive.nik           opens that archive (the setup makes unNik the
//!                               program for .nik files)
//!   unNik --extract a.nik       opens it with its Extract box     (right-click menu)
//!   unNik --extract-here a.nik  extracts next to the archive      (right-click menu)
//!   unNik --extract-to a.nik    extracts into a folder named "a"  (right-click menu)
//!   unNik --add / --add-here    passes the items on to nikpak (no console window)

// a window program: no console window on Windows
#![windows_subsystem = "windows"]

use nikpak::unnik::Quick;
use std::path::PathBuf;

fn main() {
    nikpak::recomp::install_panic_hook();
    // An installed unNik keeps its folder on the PATH too (it holds both
    // programs), so `nikpak` and `unNik` work in any command prompt.
    nikpak::envpath::ensure_in_path();
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    let flag = args.first().map(|a| a.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let rest = || nikpak::shell::make_absolute(args.iter().skip(1).map(PathBuf::from));
    let result = match flag.as_str() {
        "--add" | "--add-here" => nikpak::unnik::forward_to_nikpak(&args),
        "--extract" => nikpak::unnik::run_quick(Quick::Dialog, rest()),
        "--extract-here" => nikpak::unnik::run_quick(Quick::Here, rest()),
        "--extract-to" => nikpak::unnik::run_quick(Quick::ToFolder, rest()),
        _ => nikpak::unnik::run(args.first().map(PathBuf::from)),
    };
    if let Err(e) = result {
        eprintln!("unNik needs a graphical display: {e}");
        std::process::exit(2);
    }
}
