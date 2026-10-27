//! Right-click menu of the file manager ("NikPak" submenu, like WinRAR's).
//!
//! Items for files and folders:  Add to archive…  /  Add to archive here
//! Items for .nik archives:      Open with unNik  /  Extract files…  /  Extract here  /
//!                               Extract to its own folder  /  Add to archive…
//!
//!   nikpak --install-menu   [--all-users]   adds the menu for this user (or all users)
//!   nikpak --uninstall-menu [--all-users]   removes it
//!
//! * Windows: registry verbs (a cascading "NikPak" submenu) under
//!   AllFilesystemObjects and SystemFileAssociations\.nik, written with
//!   `reg import`. The Windows setup runs this; on Windows 11 the submenu is
//!   in "Show more options" (Shift+F10), like every classic menu entry.
//! * Linux: KDE Dolphin service menus, Nemo actions, Nautilus scripts
//!   (Scripts > NikPak), the application/x-nikpak MIME type and an
//!   unNik .desktop file for "Open with".
//! * macOS: Finder Quick Actions in ~/Library/Services.
//!
//! The commands the menu runs:
//!   nikpak --add <items>          Add to archive… (asks for name and place)
//!   nikpak --add-here <items>     Add to archive here (next to the items)
//!   unNik  --extract <archive>    Extract files… (opens the archive and its Extract box)
//!   unNik  --extract-here <archives>
//!   unNik  --extract-to <archives>   (a folder named after each archive)
//!
//! File managers that start one process per selected item (Windows Explorer)
//! are handled by [`collect`]: the first process gathers the items of the
//! others for a moment and works on all of them; the others exit at once.

use crate::i18n::t;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

/// Paths given on the command line made absolute (Nautilus passes names
/// relative to the current folder).
pub fn absolute(args: &[String]) -> Vec<PathBuf> {
    make_absolute(args.iter().filter(|a| !a.is_empty() && !a.starts_with("--")).map(PathBuf::from))
}

pub fn make_absolute(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_default();
    paths
        .into_iter()
        .filter(|p| !p.as_os_str().is_empty() && !p.to_string_lossy().starts_with("--"))
        .map(|p| if p.is_absolute() { p } else { cwd.join(p) })
        .collect()
}

// ---------------------------------------------------------------------------
// One window for a multiple selection
// ---------------------------------------------------------------------------

/// Gathers the items of a multiple selection when the file manager starts one
/// process per item. Returns all the items for the process that does the work,
/// None for the others (which should exit).
pub fn collect(verb: &str, items: Vec<PathBuf>) -> Option<Vec<PathBuf>> {
    if items.is_empty() {
        return None;
    }
    let key = {
        // same verb + same folder = same selection
        let parent = items[0].parent().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in parent.bytes().chain(verb.bytes()) {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
        format!("{verb}-{h:016x}")
    };
    let base = std::env::temp_dir().join("nikpak-shell");
    let dir = base.join(&key);
    for _attempt in 0..3 {
        let _ = std::fs::create_dir_all(&dir);
        let leader = dir.join("leader");
        // a leader that crashed leaves its file: take over after 30 s
        if let Ok(age) = std::fs::metadata(&leader).and_then(|m| m.modified()) {
            if SystemTime::now().duration_since(age).unwrap_or_default() > Duration::from_secs(30) {
                let _ = std::fs::remove_file(&leader);
            }
        }
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&leader) {
            Ok(_) => return Some(lead(&dir, items)),
            Err(_) => {
                // follower: hand the items to the leader and exit
                let name = format!("{}-{}.item", std::process::id(), SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos());
                let tmp = dir.join(format!("{name}.part"));
                let list: String = items.iter().map(|p| format!("{}\n", p.display())).collect();
                if std::fs::write(&tmp, list).is_ok() && std::fs::rename(&tmp, dir.join(&name)).is_ok() {
                    return None;
                }
                let _ = std::fs::remove_file(&tmp);
                // the leader has just finished: try to become the next leader
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    Some(items)
}

fn lead(dir: &Path, mut items: Vec<PathBuf>) -> Vec<PathBuf> {
    let count = || std::fs::read_dir(dir).map(|r| r.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "item")).count()).unwrap_or(0);
    let start = Instant::now();
    let (mut last, mut quiet) = (count(), Instant::now());
    // wait until no new item arrived for 350 ms (at most 4 s)
    while start.elapsed() < Duration::from_secs(4) {
        std::thread::sleep(Duration::from_millis(60));
        let n = count();
        if n != last {
            last = n;
            quiet = Instant::now();
        } else if quiet.elapsed() >= Duration::from_millis(350) {
            break;
        }
    }
    // move the folder away so that late followers start a new selection
    let done = dir.with_file_name(format!("{}.done-{}", dir.file_name().unwrap_or_default().to_string_lossy(), std::process::id()));
    let mut from = dir.to_path_buf();
    for _ in 0..20 {
        if std::fs::rename(dir, &done).is_ok() {
            from = done.clone();
            break;
        }
        std::thread::sleep(Duration::from_millis(25)); // Windows: a follower is still writing
    }
    std::thread::sleep(Duration::from_millis(40));
    let mut files: Vec<PathBuf> = std::fs::read_dir(&from).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "item")).collect()).unwrap_or_default();
    files.sort();
    for f in files {
        if let Ok(s) = std::fs::read_to_string(&f) {
            items.extend(s.lines().filter(|l| !l.is_empty()).map(PathBuf::from));
        }
    }
    let _ = std::fs::remove_dir_all(&from);
    // keep Explorer's order, without duplicates
    let mut seen = std::collections::HashSet::new();
    items.retain(|p| seen.insert(p.clone()));
    items
}

// ---------------------------------------------------------------------------
// Menu labels
// ---------------------------------------------------------------------------

pub struct Labels {
    pub add: String,
    pub add_here: String,
    pub open: String,
    pub extract: String,
    pub extract_here: String,
    pub extract_to: String,
}

pub fn labels() -> Labels {
    Labels {
        add: t("Add to archive…").into(),
        add_here: t("Add to archive here").into(),
        open: t("Open with unNik").into(),
        extract: t("Extract files…").into(),
        extract_here: t("Extract here").into(),
        extract_to: t("Extract to its own folder").into(),
    }
}

fn programs() -> Result<(PathBuf, PathBuf), String> {
    let nikpak = std::env::current_exe().map_err(|e| e.to_string())?;
    let nikpak = std::fs::canonicalize(&nikpak).unwrap_or(nikpak);
    let unnik = crate::gui::sibling_exe("unNik").ok_or_else(|| "unNik was not found next to nikpak".to_string())?;
    #[cfg(windows)]
    let (nikpak, unnik) = (strip_unc(nikpak), strip_unc(unnik));
    Ok((nikpak, unnik))
}

/// canonicalize() on Windows returns \\?\C:\..., which Explorer does not accept in commands.
#[cfg(windows)]
fn strip_unc(p: PathBuf) -> PathBuf {
    let s = p.display().to_string();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p,
    }
}

/// `nikpak --install-menu` / `--uninstall-menu`. Returns a message for the console.
pub fn install(all_users: bool) -> Result<String, String> {
    crate::i18n::set(crate::gui::load_language());
    let (nikpak, unnik) = programs()?;
    platform::install(&nikpak, &unnik, &labels(), all_users)
}

pub fn uninstall(all_users: bool) -> Result<String, String> {
    platform::uninstall(all_users)
}

// ---------------------------------------------------------------------------
// Windows: registry
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod platform {
    use super::Labels;
    use std::path::Path;

    const FILES: &str = r"AllFilesystemObjects\shell\NikPak";
    const NIK: &str = r"SystemFileAssociations\.nik\shell\NikPak";

    fn root(all_users: bool) -> &'static str {
        if all_users {
            r"HKEY_LOCAL_MACHINE\Software\Classes"
        } else {
            r"HKEY_CURRENT_USER\Software\Classes"
        }
    }

    /// A string value in .reg syntax.
    fn q(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    struct Item<'a> {
        key: &'a str,
        label: &'a str,
        icon: &'a Path,
        command: String,
        separator_before: bool,
    }

    fn cascade(out: &mut String, base: &str, icon: &Path, applies_to: Option<&str>, items: &[Item]) {
        out.push_str(&format!("[{base}]\r\n\"MUIVerb\"=\"NikPak\"\r\n\"Icon\"={}\r\n\"SubCommands\"=\"\"\r\n\"MultiSelectModel\"=\"Player\"\r\n", q(&format!("{},0", icon.display()))));
        if let Some(a) = applies_to {
            out.push_str(&format!("\"AppliesTo\"={}\r\n", q(a)));
        }
        out.push_str("\r\n");
        for it in items {
            out.push_str(&format!(
                "[{base}\\shell\\{}]\r\n\"MUIVerb\"={}\r\n\"Icon\"={}\r\n\"MultiSelectModel\"=\"Player\"\r\n",
                it.key,
                q(it.label),
                q(&format!("{},0", it.icon.display()))
            ));
            if it.separator_before {
                out.push_str("\"CommandFlags\"=dword:00000020\r\n");
            }
            out.push_str(&format!("\r\n[{base}\\shell\\{}\\command]\r\n@={}\r\n\r\n", it.key, q(&it.command)));
        }
    }

    pub fn install(nikpak: &Path, unnik: &Path, l: &Labels, all_users: bool) -> Result<String, String> {
        let r = root(all_users);
        let _ = remove(all_users); // old entries (other item keys) go first
        // unNik is a window program: starting the menu commands through it avoids
        // the console window that nikpak.exe (a console program) would flash
        let add = |verb: &str| format!("\"{}\" {verb} \"%1\"", unnik.display());
        let un = add;
        let mut reg = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
        cascade(
            &mut reg,
            &format!("{r}\\{FILES}"),
            nikpak,
            Some("NOT System.FileExtension:=.nik"),
            &[
                Item { key: "01add", label: &l.add, icon: nikpak, command: add("--add"), separator_before: false },
                Item { key: "02addhere", label: &l.add_here, icon: nikpak, command: add("--add-here"), separator_before: false },
            ],
        );
        cascade(
            &mut reg,
            &format!("{r}\\{NIK}"),
            nikpak,
            None,
            &[
                Item { key: "01open", label: &l.open, icon: unnik, command: format!("\"{}\" \"%1\"", unnik.display()), separator_before: false },
                Item { key: "02extract", label: &l.extract, icon: unnik, command: un("--extract"), separator_before: false },
                Item { key: "03here", label: &l.extract_here, icon: unnik, command: un("--extract-here"), separator_before: false },
                Item { key: "04to", label: &l.extract_to, icon: unnik, command: un("--extract-to"), separator_before: false },
                Item { key: "05add", label: &l.add, icon: nikpak, command: add("--add"), separator_before: true },
            ],
        );
        import(&reg)?;
        Ok(format!(
            "The NikPak right-click menu was added ({}). On Windows 11 it is under \"Show more options\".",
            if all_users { "all users" } else { "this user" }
        ))
    }

    fn remove(all_users: bool) -> Result<(), String> {
        let r = root(all_users);
        import(&format!("Windows Registry Editor Version 5.00\r\n\r\n[-{r}\\{FILES}]\r\n\r\n[-{r}\\{NIK}]\r\n\r\n"))
    }

    pub fn uninstall(all_users: bool) -> Result<String, String> {
        remove(all_users)?;
        Ok("The NikPak right-click menu was removed.".into())
    }

    /// `reg import` of a UTF-16 .reg file (exact quoting, no shell involved).
    fn import(text: &str) -> Result<(), String> {
        use std::os::windows::process::CommandExt;
        let file = std::env::temp_dir().join(format!("nikpak-menu-{}.reg", std::process::id()));
        let mut bytes = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            bytes.extend_from_slice(&u.to_le_bytes());
        }
        std::fs::write(&file, bytes).map_err(|e| e.to_string())?;
        let out = std::process::Command::new("reg")
            .arg("import")
            .arg(&file)
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output()
            .map_err(|e| e.to_string());
        let _ = std::fs::remove_file(&file);
        let out = out?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("reg import failed: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}

// ---------------------------------------------------------------------------
// Linux: Dolphin, Nemo, Nautilus, MIME type, "Open with"
// ---------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::Labels;
    use std::path::{Path, PathBuf};

    fn data_home() -> Result<PathBuf, String> {
        if let Some(d) = std::env::var_os("XDG_DATA_HOME").filter(|d| !d.is_empty()) {
            return Ok(PathBuf::from(d));
        }
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")).ok_or_else(|| "HOME is not set".into())
    }

    /// Every file this function writes (also what uninstall removes).
    fn files(d: &Path, l: Option<&Labels>) -> Vec<PathBuf> {
        let mut v = vec![
            d.join("kio/servicemenus/nikpak.desktop"),
            d.join("kio/servicemenus/nikpak-archive.desktop"),
            d.join("kservices5/ServiceMenus/nikpak.desktop"),
            d.join("kservices5/ServiceMenus/nikpak-archive.desktop"),
            d.join("nemo/actions/nikpak-1-add.nemo_action"),
            d.join("nemo/actions/nikpak-2-add-here.nemo_action"),
            d.join("nemo/actions/nikpak-3-extract.nemo_action"),
            d.join("nemo/actions/nikpak-4-extract-here.nemo_action"),
            d.join("nemo/actions/nikpak-5-extract-to.nemo_action"),
            d.join("mime/packages/nikpak.xml"),
            d.join("applications/unnik.desktop"),
        ];
        if let Some(l) = l {
            let s = d.join("nautilus/scripts/NikPak");
            for name in [&l.add, &l.add_here, &l.extract, &l.extract_here, &l.extract_to] {
                v.push(s.join(script_name(name)));
            }
        }
        v
    }

    fn script_name(label: &str) -> String {
        label.replace(['/', '\\'], "-")
    }

    fn desktop_exec(p: &Path) -> String {
        // .desktop Exec quoting
        format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\"").replace('`', "\\`").replace('$', "\\$"))
    }

    fn sh(p: &Path) -> String {
        format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
    }

    fn write(path: &Path, text: &str, executable: bool) -> Result<(), String> {
        use std::os::unix::fs::PermissionsExt;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        if executable {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
        }
        Ok(())
    }

    pub fn install(nikpak: &Path, unnik: &Path, l: &Labels, all_users: bool) -> Result<String, String> {
        if all_users {
            return Err("--all-users is only available on Windows; run --install-menu as each user.".into());
        }
        let d = data_home()?;
        let (np, un) = (desktop_exec(nikpak), desktop_exec(unnik));

        // MIME type, so that file managers know .nik archives
        write(
            &d.join("mime/packages/nikpak.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n  <mime-type type=\"application/x-nikpak\">\n    <comment>NikPak archive</comment>\n    <sub-class-of type=\"application/octet-stream\"/>\n    <magic priority=\"60\"><match type=\"string\" value=\"\\x89NIK\\r\\n\\x1a\\n\" offset=\"0\"/></magic>\n    <glob pattern=\"*.nik\"/>\n  </mime-type>\n</mime-info>\n",
            false,
        )?;
        let _ = std::process::Command::new("update-mime-database").arg(d.join("mime")).output();
        write(
            &d.join("applications/unnik.desktop"),
            &format!("[Desktop Entry]\nType=Application\nName=unNik\nComment=Browse and extract .nik archives\nExec={un} %f\nMimeType=application/x-nikpak;\nCategories=Utility;Archiving;\nTerminal=false\n"),
            false,
        )?;
        let _ = std::process::Command::new("update-desktop-database").arg(d.join("applications")).output();

        // KDE Dolphin (KF6 and KF5)
        let kde_files = format!(
            "[Desktop Entry]\nType=Service\nMimeType=all/allfiles;inode/directory;\nActions=nikpakAdd;nikpakAddHere;\nX-KDE-Submenu=NikPak\nX-KDE-Priority=TopLevel\n\n[Desktop Action nikpakAdd]\nName={}\nIcon=archive-insert\nExec={np} --add %F\n\n[Desktop Action nikpakAddHere]\nName={}\nIcon=archive-insert\nExec={np} --add-here %F\n",
            l.add, l.add_here
        );
        let kde_nik = format!(
            "[Desktop Entry]\nType=Service\nMimeType=application/x-nikpak;\nActions=nikpakExtract;nikpakHere;nikpakTo;\nX-KDE-Submenu=NikPak\nX-KDE-Priority=TopLevel\n\n[Desktop Action nikpakExtract]\nName={}\nIcon=archive-extract\nExec={un} --extract %f\n\n[Desktop Action nikpakHere]\nName={}\nIcon=archive-extract\nExec={un} --extract-here %F\n\n[Desktop Action nikpakTo]\nName={}\nIcon=archive-extract\nExec={un} --extract-to %F\n",
            l.extract, l.extract_here, l.extract_to
        );
        for dir in ["kio/servicemenus", "kservices5/ServiceMenus"] {
            write(&d.join(dir).join("nikpak.desktop"), &kde_files, true)?;
            write(&d.join(dir).join("nikpak-archive.desktop"), &kde_nik, true)?;
        }

        // Nemo (Cinnamon)
        let nemo = |file: &str, name: &str, exec: String, ext: &str, sel: &str, icon: &str| -> Result<(), String> {
            write(
                &d.join("nemo/actions").join(file),
                &format!("[Nemo Action]\nName=NikPak: {}\nComment={}\nExec={exec}\nIcon-Name={icon}\nSelection={sel}\nExtensions={ext};\n", name.replace('_', "__"), name),
                false,
            )
        };
        nemo("nikpak-1-add.nemo_action", &l.add, format!("{np} --add %F"), "any", "notnone", "archive-insert")?;
        nemo("nikpak-2-add-here.nemo_action", &l.add_here, format!("{np} --add-here %F"), "any", "notnone", "archive-insert")?;
        nemo("nikpak-3-extract.nemo_action", &l.extract, format!("{un} --extract %F"), "nik", "s", "archive-extract")?;
        nemo("nikpak-4-extract-here.nemo_action", &l.extract_here, format!("{un} --extract-here %F"), "nik", "notnone", "archive-extract")?;
        nemo("nikpak-5-extract-to.nemo_action", &l.extract_to, format!("{un} --extract-to %F"), "nik", "notnone", "archive-extract")?;

        // Nautilus (GNOME Files): Scripts > NikPak > ...
        let s = d.join("nautilus/scripts/NikPak");
        let (npq, unq) = (sh(nikpak), sh(unnik));
        let only_nik = "for f in \"$@\"; do case \"$f\" in *.nik|*.NIK) set -- \"$@\" \"$f\";; esac; shift; done\n[ $# -gt 0 ] || exit 0\n";
        write(&s.join(script_name(&l.add)), &format!("#!/bin/sh\nexec {npq} --add \"$@\"\n"), true)?;
        write(&s.join(script_name(&l.add_here)), &format!("#!/bin/sh\nexec {npq} --add-here \"$@\"\n"), true)?;
        write(&s.join(script_name(&l.extract)), &format!("#!/bin/sh\n{only_nik}exec {unq} --extract \"$1\"\n"), true)?;
        write(&s.join(script_name(&l.extract_here)), &format!("#!/bin/sh\n{only_nik}exec {unq} --extract-here \"$@\"\n"), true)?;
        write(&s.join(script_name(&l.extract_to)), &format!("#!/bin/sh\n{only_nik}exec {unq} --extract-to \"$@\"\n"), true)?;

        Ok(format!(
            "The NikPak right-click menu was added for Dolphin (KDE), Nemo (Cinnamon) and Nautilus (GNOME: Scripts > NikPak), in {}.\nRestart the file manager if it was open (nautilus -q, nemo -q).",
            d.display()
        ))
    }

    pub fn uninstall(_all_users: bool) -> Result<String, String> {
        let d = data_home()?;
        for f in files(&d, None) {
            let _ = std::fs::remove_file(f);
        }
        let _ = std::fs::remove_dir_all(d.join("nautilus/scripts/NikPak"));
        let _ = std::process::Command::new("update-mime-database").arg(d.join("mime")).output();
        let _ = std::process::Command::new("update-desktop-database").arg(d.join("applications")).output();
        Ok("The NikPak right-click menu was removed.".into())
    }

    #[cfg(test)]
    pub(super) fn all_files(d: &Path, l: &Labels) -> Vec<PathBuf> {
        files(d, Some(l))
    }
}

// ---------------------------------------------------------------------------
// macOS: Finder Quick Actions
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod platform {
    use super::Labels;
    use std::path::{Path, PathBuf};

    const PREFIX: &str = "NikPak - ";

    fn services() -> Result<PathBuf, String> {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Services")).ok_or_else(|| "HOME is not set".into())
    }

    fn xml(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    fn sh(p: &Path) -> String {
        format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
    }

    fn workflow(dir: &Path, title: &str, script: &str) -> Result<(), String> {
        let c = dir.join(format!("{PREFIX}{}.workflow", title.replace('/', "-"))).join("Contents");
        std::fs::create_dir_all(&c).map_err(|e| e.to_string())?;
        let info = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>NSServices</key>
	<array>
		<dict>
			<key>NSBackgroundColorName</key>
			<string>background</string>
			<key>NSIconName</key>
			<string>NSActionTemplate</string>
			<key>NSMenuItem</key>
			<dict>
				<key>default</key>
				<string>NikPak: {}</string>
			</dict>
			<key>NSMessage</key>
			<string>runWorkflowAsService</string>
			<key>NSRequiredContext</key>
			<dict>
				<key>NSApplicationIdentifier</key>
				<string>com.apple.finder</string>
			</dict>
			<key>NSSendFileTypes</key>
			<array>
				<string>public.item</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#,
            xml(title)
        );
        let doc = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AMApplicationBuild</key>
	<string>523</string>
	<key>AMApplicationVersion</key>
	<string>2.10</string>
	<key>AMDocumentVersion</key>
	<string>2</string>
	<key>actions</key>
	<array>
		<dict>
			<key>action</key>
			<dict>
				<key>AMAccepts</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Optional</key>
					<true/>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>AMActionVersion</key>
				<string>2.0.3</string>
				<key>AMApplication</key>
				<array>
					<string>Automator</string>
				</array>
				<key>AMParameterProperties</key>
				<dict>
					<key>COMMAND_STRING</key>
					<dict/>
					<key>CheckedForUserDefaultShell</key>
					<dict/>
					<key>inputMethod</key>
					<dict/>
					<key>shell</key>
					<dict/>
					<key>source</key>
					<dict/>
				</dict>
				<key>AMProvides</key>
				<dict>
					<key>Container</key>
					<string>List</string>
					<key>Types</key>
					<array>
						<string>com.apple.cocoa.string</string>
					</array>
				</dict>
				<key>ActionBundlePath</key>
				<string>/System/Library/Automator/Run Shell Script.action</string>
				<key>ActionName</key>
				<string>Run Shell Script</string>
				<key>ActionParameters</key>
				<dict>
					<key>COMMAND_STRING</key>
					<string>{}</string>
					<key>CheckedForUserDefaultShell</key>
					<true/>
					<key>inputMethod</key>
					<integer>1</integer>
					<key>shell</key>
					<string>/bin/sh</string>
					<key>source</key>
					<string></string>
				</dict>
				<key>BundleIdentifier</key>
				<string>com.apple.RunShellScript</string>
				<key>CFBundleVersion</key>
				<string>2.0.3</string>
				<key>CanShowSelectedItemsWhenRun</key>
				<false/>
				<key>CanShowWhenRun</key>
				<true/>
				<key>Category</key>
				<array>
					<string>AMCategoryUtilities</string>
				</array>
				<key>Class Name</key>
				<string>RunShellScriptAction</string>
				<key>InputUUID</key>
				<string>6A1D4A9C-7F4B-4C70-9C9E-0F6D3A1B2C01</string>
				<key>Keywords</key>
				<array>
					<string>Shell</string>
					<string>Script</string>
				</array>
				<key>OutputUUID</key>
				<string>6A1D4A9C-7F4B-4C70-9C9E-0F6D3A1B2C02</string>
				<key>UUID</key>
				<string>6A1D4A9C-7F4B-4C70-9C9E-0F6D3A1B2C03</string>
				<key>UnlocalizedApplications</key>
				<array>
					<string>Automator</string>
				</array>
				<key>arguments</key>
				<dict/>
				<key>isViewVisible</key>
				<integer>1</integer>
				<key>location</key>
				<string>309.000000:253.000000</string>
				<key>nibPath</key>
				<string>/System/Library/Automator/Run Shell Script.action/Contents/Resources/Base.lproj/main.nib</string>
			</dict>
			<key>isViewVisible</key>
			<integer>1</integer>
		</dict>
	</array>
	<key>connectors</key>
	<dict/>
	<key>workflowMetaData</key>
	<dict>
		<key>applicationBundleIDsByPath</key>
		<dict/>
		<key>applicationPaths</key>
		<array/>
		<key>inputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>outputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>presentationMode</key>
		<integer>15</integer>
		<key>processesInput</key>
		<false/>
		<key>serviceApplicationBundleID</key>
		<string>com.apple.finder</string>
		<key>serviceApplicationPath</key>
		<string>/System/Library/CoreServices/Finder.app</string>
		<key>serviceInputTypeIdentifier</key>
		<string>com.apple.Automator.fileSystemObject</string>
		<key>serviceOutputTypeIdentifier</key>
		<string>com.apple.Automator.nothing</string>
		<key>serviceProcessesInput</key>
		<false/>
		<key>systemImageName</key>
		<string>NSActionTemplate</string>
		<key>useAutomaticInputType</key>
		<false/>
		<key>workflowTypeIdentifier</key>
		<string>com.apple.Automator.servicesMenu</string>
	</dict>
</dict>
</plist>
"#,
            xml(script)
        );
        std::fs::write(c.join("Info.plist"), info).map_err(|e| e.to_string())?;
        std::fs::write(c.join("document.wflow"), doc).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn install(nikpak: &Path, unnik: &Path, l: &Labels, all_users: bool) -> Result<String, String> {
        if all_users {
            return Err("--all-users is only available on Windows; run --install-menu as each user.".into());
        }
        let _ = uninstall(false);
        let dir = services()?;
        let (np, un) = (sh(nikpak), sh(unnik));
        let only_nik = "for f in \"$@\"; do case \"$f\" in *.nik|*.NIK) set -- \"$@\" \"$f\";; esac; shift; done\n[ $# -gt 0 ] || exit 0\n";
        workflow(&dir, &l.add, &format!("{np} --add \"$@\" >/dev/null 2>&1 &"))?;
        workflow(&dir, &l.add_here, &format!("{np} --add-here \"$@\" >/dev/null 2>&1 &"))?;
        workflow(&dir, &l.extract, &format!("{only_nik}{un} --extract \"$1\" >/dev/null 2>&1 &"))?;
        workflow(&dir, &l.extract_here, &format!("{only_nik}{un} --extract-here \"$@\" >/dev/null 2>&1 &"))?;
        workflow(&dir, &l.extract_to, &format!("{only_nik}{un} --extract-to \"$@\" >/dev/null 2>&1 &"))?;
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").output();
        Ok(format!("The NikPak Quick Actions were added to {}: right-click > Quick Actions in Finder.", dir.display()))
    }

    pub fn uninstall(_all_users: bool) -> Result<String, String> {
        let dir = services()?;
        if let Ok(rd) = std::fs::read_dir(&dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with(PREFIX) && name.ends_with(".workflow") {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        let _ = std::process::Command::new("/System/Library/CoreServices/pbs").arg("-update").output();
        Ok("The NikPak Quick Actions were removed.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_one_selection() {
        let dir = std::env::temp_dir().join(format!("nikpak-collect-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.txt");
        let b = dir.join("b.txt");
        let c = dir.join("c.txt");
        let verb = format!("test{}", std::process::id());
        let (va, vb) = (verb.clone(), verb.clone());
        let (a2, b2) = (a.clone(), b.clone());
        let leader = std::thread::spawn(move || collect(&va, vec![a2]));
        std::thread::sleep(Duration::from_millis(40));
        let f1 = std::thread::spawn(move || collect(&vb, vec![b2]));
        let f2 = collect(&verb, vec![c.clone()]);
        let f1 = f1.join().unwrap();
        let all = leader.join().unwrap().expect("first process leads");
        assert!(f1.is_none() && f2.is_none());
        assert_eq!(all.len(), 3);
        assert!(all.contains(&a) && all.contains(&b) && all.contains(&c));
        // a later selection starts again
        let again = collect(&verb, vec![a.clone()]).unwrap();
        assert_eq!(again, vec![a]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_menu_files() {
        let home = std::env::temp_dir().join(format!("nikpak-menu-test-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &home);
        let l = labels();
        let np = Path::new("/opt/Nik Pak/nikpak");
        let un = Path::new("/opt/Nik Pak/unNik");
        platform::install(np, un, &l, false).unwrap();
        for f in platform::all_files(&home, &l) {
            assert!(f.is_file(), "missing {}", f.display());
        }
        let kde = std::fs::read_to_string(home.join("kio/servicemenus/nikpak.desktop")).unwrap();
        assert!(kde.contains("Exec=\"/opt/Nik Pak/nikpak\" --add %F"));
        let script = std::fs::read_to_string(home.join("nautilus/scripts/NikPak").join(&l.extract_here)).unwrap();
        assert!(script.contains("'/opt/Nik Pak/unNik' --extract-here"));
        platform::uninstall(false).unwrap();
        assert!(!home.join("kio/servicemenus/nikpak.desktop").exists());
        assert!(!home.join("nautilus/scripts/NikPak").exists());
        let _ = std::fs::remove_dir_all(home);
    }
}
