//! Keep the folder of the installed programs on the PATH.
//!
//! `nikpak` and `unNik` should open when they are typed in any command prompt,
//! wherever the user is. The Windows setup and the Unix `install.sh` add the
//! folder at install time; [`ensure_in_path`] repeats it at every start, so an
//! installation whose box was left unticked (or whose PATH was edited
//! afterwards) repairs itself.
//!
//! Only a real installation folder is ever added. A build in `target\release`,
//! a copy on a USB key or a folder in the temp directory is ignored, so running
//! a test build never pollutes the user's PATH.
//!
//! Nothing here is allowed to stop the program: a PATH that cannot be read or
//! written is simply left alone.

use std::path::{Path, PathBuf};

/// The folder holding this program, when it is a real installation.
pub fn install_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    if is_install_dir(&dir) {
        Some(dir)
    } else {
        None
    }
}

/// Add the installation folder to the PATH when it is missing. Does nothing for
/// a build tree.
pub fn ensure_in_path() {
    let Some(dir) = install_dir() else { return };
    #[cfg(windows)]
    windows::add(&dir);
    #[cfg(unix)]
    unix::add(&dir);
}

// ---------------------------------------------------------------------------
// Which folders count as an installation
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn is_install_dir(dir: &Path) -> bool {
    fn norm(p: &Path) -> String {
        p.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase()
    }
    // {autopf} is Program Files; a per-user setup uses %LOCALAPPDATA%\Programs.
    let mut roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|k| std::env::var_os(k).map(PathBuf::from))
        .collect();
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(local).join("Programs"));
    }
    let d = norm(dir);
    roots.iter().any(|r| {
        let r = norm(r);
        d == r || d.starts_with(&format!("{r}\\"))
    })
}

#[cfg(unix)]
fn is_install_dir(dir: &Path) -> bool {
    let d = dir.to_string_lossy();
    if d == "/usr/local/bin" || d == "/usr/bin" || d == "/bin" {
        return true;
    }
    if d.starts_with("/opt/") {
        return true;
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .is_some_and(|h| dir == h.join(".local/bin").as_path())
}

// ---------------------------------------------------------------------------
// Windows: the user's PATH (HKCU\Environment), which needs no administrator
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows {
    use std::path::Path;

    type Hkey = isize;
    const HKCU: Hkey = 0x8000_0001u32 as Hkey;
    const HKLM: Hkey = 0x8000_0002u32 as Hkey;
    const KEY_READ: u32 = 0x0002_0019;
    const KEY_SET_VALUE: u32 = 0x0000_0002;
    const REG_EXPAND_SZ: u32 = 2;
    const ENV_KEY: &str = "Environment";
    const MACHINE_KEY: &str = r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment";

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(hkey: Hkey, sub_key: *const u16, options: u32, access: u32, out: *mut Hkey) -> i32;
        fn RegQueryValueExW(hkey: Hkey, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
        fn RegSetValueExW(hkey: Hkey, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
        fn RegCloseKey(hkey: Hkey) -> i32;
    }

    #[link(name = "user32")]
    extern "system" {
        fn SendMessageTimeoutW(hwnd: isize, msg: u32, wparam: usize, lparam: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// The `Path` value of a key, as stored (REG_SZ or REG_EXPAND_SZ).
    fn read_path(root: Hkey, key: &str) -> Option<String> {
        // SAFETY: plain Win32 registry calls on a key opened just before, with
        // a buffer sized by the length the first query returned.
        unsafe {
            let mut h: Hkey = 0;
            if RegOpenKeyExW(root, wide(key).as_ptr(), 0, KEY_READ, &mut h) != 0 {
                return None;
            }
            let name = wide("Path");
            let (mut kind, mut bytes) = (0u32, 0u32);
            let sized = RegQueryValueExW(h, name.as_ptr(), std::ptr::null_mut(), &mut kind, std::ptr::null_mut(), &mut bytes) == 0 && bytes > 0;
            if !sized {
                RegCloseKey(h);
                return None;
            }
            let mut buf = vec![0u8; bytes as usize];
            let mut got = bytes;
            let rc = RegQueryValueExW(h, name.as_ptr(), std::ptr::null_mut(), &mut kind, buf.as_mut_ptr(), &mut got);
            RegCloseKey(h);
            if rc != 0 {
                return None;
            }
            let mut text: Vec<u16> = buf[..got as usize]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            while text.last() == Some(&0) {
                text.pop();
            }
            Some(String::from_utf16_lossy(&text))
        }
    }

    /// Write the user's `Path` (HKCU\Environment). The user's own PATH needs no
    /// administrator rights, which is why the machine one is not touched.
    fn write_user_path(value: &str) -> bool {
        // SAFETY: RegSetValueExW copies the bytes before returning; the vectors
        // stay alive for the whole call.
        unsafe {
            let mut h: Hkey = 0;
            if RegOpenKeyExW(HKCU, wide(ENV_KEY).as_ptr(), 0, KEY_READ | KEY_SET_VALUE, &mut h) != 0 {
                return false;
            }
            let name = wide("Path");
            let data: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
            let raw = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
            let rc = RegSetValueExW(h, name.as_ptr(), 0, REG_EXPAND_SZ, raw.as_ptr(), raw.len() as u32);
            RegCloseKey(h);
            rc == 0
        }
    }

    /// WM_SETTINGCHANGE("Environment"): Explorer passes the new PATH to the
    /// programs started after this. Shells already running keep theirs until
    /// they are started again.
    fn announce() {
        let what = wide("Environment");
        let mut result: usize = 0;
        // SAFETY: HWND_BROADCAST with a WM_SETTINGCHANGE and a pointer to a
        // string that outlives the call.
        unsafe {
            SendMessageTimeoutW(0xFFFF, 0x001A, 0, what.as_ptr() as isize, 0x0002, 4000, &mut result);
        }
    }

    /// Is `dir` one of the entries of a `;`-separated PATH?
    fn contains(path: &str, dir: &str) -> bool {
        let want = dir.trim_end_matches('\\').to_lowercase();
        path.split(';')
            .any(|p| p.trim().trim_end_matches('\\').to_lowercase() == want)
    }

    pub(super) fn add(dir: &Path) {
        let dir = dir.to_string_lossy().into_owned();
        // Already on the user's PATH or on the machine's: nothing to do.
        let on_user = read_path(HKCU, ENV_KEY).is_some_and(|p| contains(&p, &dir));
        let on_machine = read_path(HKLM, MACHINE_KEY).is_some_and(|p| contains(&p, &dir));
        if on_user || on_machine {
            return;
        }
        let mut path = read_path(HKCU, ENV_KEY).unwrap_or_default();
        if !path.is_empty() && !path.ends_with(';') {
            path.push(';');
        }
        path.push_str(&dir);
        if write_user_path(&path) {
            announce();
        }
    }
}

// ---------------------------------------------------------------------------
// Unix: the startup file of the user's shell
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod unix {
    use std::io::Write as _;
    use std::path::{Path, PathBuf};

    pub(super) fn add(dir: &Path) {
        // macOS: a window started from the Dock gets launchd's short PATH, but
        // every login shell gets these two from /etc/paths and Homebrew, so
        // there is nothing to add (and the user's shell files stay untouched).
        #[cfg(target_os = "macos")]
        {
            if dir == Path::new("/usr/local/bin") || dir == Path::new("/opt/homebrew/bin") {
                return;
            }
        }
        let dir = dir.to_string_lossy().into_owned();
        // Already effective in this process (it was started from a shell that
        // knows it), so the startup file is fine.
        if std::env::var("PATH").is_ok_and(|p| p.split(':').any(|x| x == dir.as_str())) {
            return;
        }
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return;
        };
        let shell = std::env::var("SHELL").unwrap_or_default();
        let wanted = if shell.ends_with("zsh") {
            home.join(".zshrc")
        } else if shell.ends_with("bash") {
            home.join(".bashrc")
        } else {
            home.join(".profile")
        };
        // Prefer a file that exists; create the shell's own one otherwise.
        let profile = home.join(".profile");
        let file = if wanted.is_file() {
            wanted
        } else if profile.is_file() {
            profile
        } else {
            wanted
        };
        let line = format!("export PATH=\"{dir}:$PATH\"");
        if std::fs::read_to_string(&file).is_ok_and(|s| s.contains(line.as_str())) {
            return;
        }
        let mut text = String::from("\n# added by NikPak so that `nikpak` opens in any shell\n");
        text.push_str(&line);
        text.push('\n');
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&file) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}
