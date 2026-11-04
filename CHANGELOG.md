# nikpak – update log

Every delivered build gets a new version number (`nikpak -V` prints it).
The newest version is at the top. Format: version – date – summary, then the details.

---

## 1.9.0 – 4 November 2026 – Cover pictures for archives

### Added
- **An archive can carry a cover picture.** Any picture (PNG, JPEG, WebP, BMP or GIF) is cropped square from its centre and stored in the header as a 1024 × 1024 JPEG. Like the legal statement it is readable without the password, and in an encrypted archive it is authenticated with every block: swapping it breaks the archive. A delete keeps it.
- **In the NikPak window**: a **▣ Cover** pill beside **§ Legal** opens a box with the picture — click the frame to choose one, or drop it on the window — and **Attach to new archives**. The cover is kept in `cover.jpg` beside the settings.
- **On the command line**: `nikpak a -cover art.png …`; `nikpak l` shows the cover's size.
- **unNik** shows the cover beside the terms and the password, and as a thumbnail in the title bar; a click shows it large.
- Library: `cover::from_picture`, `archive::cover_image()`; Nikkomander uses them for its pack box and Quick view.
- Docs: the user guide (new "Terms and cover" section), the options and exit-code tables, and the technical guide's header format, in the ten languages; the Word and PDF technical guide.

### Changed
- New dependency: `image` 0.25 (already in the tree through the clipboard crate), for reading pictures and writing the cover.

---

## 1.8.0 – 4 November 2026 – Legal statements in .nik archives

### Added
- **An archive can carry a legal statement** (terms of use, a licence, a copyright notice). Whoever opens it – unNik, nikpak on the command line, Nikkomander – must read it and click **I approve** before anything is listed, viewed or extracted. Cancel stops there; nothing is read.
- **In the NikPak window**: a **§ Legal** pill beside Fast / Medium / Best opens a box to write the terms and switch **Attach to new archives** on or off. The pill is lit while it is on. The terms are kept in `legal.txt` beside the settings.
- **On the command line**: `nikpak a -legal terms.txt bank.nik Folder\` attaches the text in `terms.txt` (UTF-8, up to 64 KB). Opening such an archive in a console shows the terms and asks YES or NO; `-accept` accepts them without asking (for scripts). A refusal ends with exit code 12.
- The statement sits in the archive header, in plain text, so it can be read before the password is asked. In an encrypted archive it is authenticated with every block: it cannot be changed or removed without the archive failing to open. A delete keeps it.

### Compatibility
- Archives without a statement are written exactly as before (version 3) and open in every nikpak.
- Archives with one use a new header flag. Older nikpak, unNik and Nikkomander refuse them ("please update"), so the terms cannot be skipped by using an old copy.

---

## 1.7.0 – 31 October 2026 – 34 operating-system skins, and an update screen

### Added
- 34 skins that dress the window as a desktop operating system: Windows 10, Windows 11, Windows Vista, Windows 7, Windows 95, macOS Tahoe, Sequoia, Sonoma, Ventura, Monterey, Big Sur, Catalina, Mojave and Sierra, OS X El Capitan, Yosemite, Mavericks, Lion, Leopard and Tiger, and Ubuntu Resolute Raccoon, Noble Numbat, Jammy Jellyfish, Focal Fossa, Bionic Beaver, Xenial Xerus, Trusty Tahr, Precise Pangolin, Natty Narwhal, Lucid Lynx, Karmic Koala, Hardy Heron, Gutsy Gibbon and Dapper Drake. NikPak and unNik share the list, so both show 89 built-in skins.
- **An update screen at launch**: when a newer version is published, NikPak says so once at start (installed version, new version, what changed) with **Cancel** and **Download**, which opens the download section of the site. The check runs on a background thread with the system curl and an eight-second limit; it never installs anything by itself and stays silent when offline. The same logic is in NikEdit, NikSQL and Nikkomander.
- The drop ring, its arrow and the text under it now sit on a small dark shadow, so they stay readable on the bright skins (macOS, Windows 7/10/11, the light Ubuntu releases).

### Changed
- **The Amiga Computer skin is redrawn as a real Kickstart 1.3 screen.** The blue desktop now carries one Workbench window – white frame, black outline, light body, the two disk icons with their orange labels, the orange highlight row, the checkered Boing ball, a blue-bodied second window and the bottom bar – instead of the two washed-out grey windows it had.
- **Its yellow lettering is readable now.** `accent-soft` was `#FFB44A`, which sat at 1.4:1 on the light panels: the "EVALUATION COPY · REGISTER" label, the modal kickers (ABOUT, SHAREWARE, REGISTRATION) and the web link were nearly invisible. It is `#5C2700` now (9.8:1 on the modal cards, 4.9:1 on the screen), the body text is `#0E1A26` (7.1:1), the dim text is `#28384E` (4.8:1), and the primary button uses white on the darkened orange (5.6:1). The window body is Amiga blue (`#C9DCF5`, veiled onto a `#6FA0DC` screen) rather than white, and the skin's corners are square, the way Amiga's were.

## 1.6.3 – 30 October 2026 – Fast, Medium and Best compression in the window

### Changed
- The drop window no longer compresses at one fixed level. Three pills at the
  centre top pick the level for the next archive: **Fast** (Zstandard 1),
  **Medium** (Zstandard 6, the default) and **Best** (`-m5`, the multi-engine
  LZMA2/PPMd method with Deflate recompression).
- **Live estimates while compressing:** under the progress ring, one line now
  shows `Ratio · Time · Size` - the compression ratio so far, the time still to
  go and the size the archive will have, extrapolated from the bytes already
  written. The three one-word labels are translated into the ten languages.
- The old fixed `-m5` is still there: it is simply called **Best** now.

### Tools
- **The Linux leg now also builds the two Debian packages** (`nikpak` and
  `unnik`, `make-deb.sh`, called by `package-unix.sh`), so `buildunix.ps1`
  downloads the `.deb` files together with the `.tar.gz`. The website's six
  download buttons (Windows, unNik-for-Windows, macOS, Linux, Debian, and
  unNik-for-Debian) are all produced by the build scripts now.

---

## 1.6.2 – 24 October 2026 – the painted skins step behind the interface

### Changed
- **The fourteen painted skins are discreet now.** Their artwork was competing
  with the window: big lettering (MSX BASIC, ZX SPECTRUM, COLECOVISION, the
  Burgers Tokyo sign) and bright objects sat exactly where the title, the logo,
  the drop ring and the status text are drawn. Every painted background now
  goes through a new `discreet()` pass in `src/skin.rs`: the art is pulled
  toward its own local mean (so lettering becomes a soft wash instead of a
  second headline), lightly blurred, and faded toward the skin's background
  colour - most behind the drop ring and the text column, a little more over
  the logo, least in the corners, where the picture still shows. The text, the
  ring and the icons are now crisply readable on all fourteen.
- The lettering the scenes draw themselves (`ART_TEXT` in
  `src/skin_scenes.rs`) is drawn at 30 % strength for the same reason; their
  colours, composition and mood are unchanged.
- unNik and the skins menu use the same backgrounds, so they are calmer too,
  and the gallery pictures of these skins were regenerated to match.

---

## 1.6.1 – 24 October 2026 – the storm and the mountains, painted again

### Changed
- **Lightning redrawn as a photograph of a storm.** Three stretched cloud
  decks (the noise is squashed vertically, so the clouds are flat sheets and
  not blobs), a channel made of long straight runs with sharp kinks instead of
  a wiggly line, and a glow built in rings - 54, 26, 10 and 3 pixels - so it
  falls off like real light instead of forming a flat halo. Four short forks
  leave the trunk at fixed heights and always head down and away. A second,
  very faint bolt far behind adds depth, the rain catches the flash, and the
  frame is much darker overall (mean brightness about 25/255, was 45) so the
  picture never competes with the interface.
- **Rocky Mountains redrawn with depth.** Four ranges instead of three, each
  one darker and sharper than the one behind (far #131E28 to near #060F14),
  with the sun setting in a col of the far range: five stacked gradients plus
  a flat flare, sunlight on the flanks that face it (a new `ridge_rim`), snow
  only on the peaks, thin lit clouds across the horizon and two rows of
  spruce in front.
- The **Lightning skin's chrome** is cooled down to match: accent, glow and
  cursor are now soft blue rather than yellow (colour corrections are kept in
  a `TWEAKS` table that `tools/gen_skins.py` preserves).

### Web site
- **Catching up with the program:** version 1.6.1 and its download names, all
  fifty-five skins in the gallery (the fourteen painted ones carry a **New**
  badge and a section of their own), and the skins page explains the box that
  stands beside the window with Apply and Cancel. Text changed in all ten
  languages, and the user guide has a new **Skins** chapter in each of them.
- `tools/site-shots.ps1` recaptures every picture of the site from the built
  program (the window states, unNik, the skins box, four languages), and
  `tools/site-shots-to-webp.py` turns the captures into the site's images.

---

## 1.6.0 – 24 October 2026 – fourteen painted skins, a real thunderstorm and better mountains

### Added
- **Fourteen new skins with painted backgrounds** (55 skins in all): Moon,
  America, Pattaya Neons, Burgers Tokyo, Sushi, Amiga Computer, MSX Computer,
  Colecovision, ZX Spectrum, Cat Life, Dog Life, Mario Bros, Counter Strike and
  Chess. Their artwork is drawn by `src/skin_scenes.rs` - storm clouds and a
  jagged bolt, moon and craters, layered mountain ridges, a neon skyline, a
  sushi board, Workbench-style windows, a chessboard in perspective - not by CSS
  gradients, so it scales to any window size. The colours of the windows, menus
  and fields still come from the skin's CSS variables, kept with the 41 others
  in `src/skins_data.rs`.
- **The skins box stands outside the program.** While the box is open the
  window widens by the width of the box, on the right: the program keeps its
  500 x 560 box and the box is put in the extra, transparent space beside it,
  so the whole interface stays visible behind the skin being tried. The box
  can be dragged by its title bar back over the program or anywhere else in
  the window, and it now has **Apply Skin** and **Cancel** buttons. A click on
  a skin tries it on the whole window; Apply keeps it and saves it for NikPak
  and unNik, Cancel (or Esc or the ×) goes back to the skin you had. Hovering
  no longer changes the skin by itself, and a click on the program does not
  close the box. (unNik's window is not widened: its box stays inside.)
- The skins footer says "55 skins" instead of claiming they are all
  Nikkomander's, and the two new texts are translated into the ten languages.

### Changed
- **Lightning (thunderstorm) redrawn.** Instead of two straight translucent
  bands, the background is now a storm: layered clouds lit from inside, one
  main channel with two or three short branches, a blue-white glow and rain.
- **Rocky Mountains redrawn.** Instead of flat wedges: a low sun, three hazy
  ranges with shaded flanks and small snow caps, and a dark spruce ridge in
  front.

### Tools
- `tools/build-shots.ps1` builds the program and saves a PNG of every new skin
  (`NIKPAK_GUI_SKIN`) and of the skins menu (`NIKPAK_GUI_SKINMENU`) in
  `screenshots\<version>\`.

---

## 1.5.6 – 24 October 2026 – skins: the 41 skins of Nikkomander in NikPak and unNik

### Added
- **Skins.** NikPak and unNik now have the same skins as Nikkomander: all 41 of
  them, from Nikkoid Blue (the default, NikPak's usual look) and Nikkoid Bright to
  Dracula, Nord, Solarized, Classic Blue, Matrix, Pizza, Christmas and 8-Bit Game.
  - **Skins button** (a palette, in the title bar of both programs): opens the
    skins menu, where every skin is drawn in its own colours. Pointing at a skin
    shows it on the whole window; a click keeps it; Esc or a click outside goes
    back to the skin you had.
  - **Sun / moon button** next to it: switches between Nikkoid Blue (dark) and
    Nikkoid Bright in one click, like the button of Nikkomander.
  - The chosen skin is **saved** (`skin=` in the settings file) and used at the
    next start. NikPak and unNik share it: a skin chosen in one is taken by the
    other within two seconds.
  - **Faithful backgrounds:** the skins' backgrounds (CSS gradients, stripes,
    stars, dots, bricks, the 8-bit checkerboard) are painted by NikPak itself from
    the same CSS as Nikkomander (`src/skin.rs`); the result matches Chromium's
    rendering pixel for pixel. Colours of text, buttons, fields, progress rings,
    dialogs, menus and unNik's folders and file icons come from the skin.
    Flat skins (Black Skin, Classic Blue …) have no glow and square corners.
  - **Your own skins:** "Import a skin…" at the bottom of the menu reads a
    Nikkomander `.skin.json` file (a name and a list of colours) and copies it to
    the `skins` folder next to the settings file; the × on an imported skin's tile
    removes it.
  - The skins come from Nikkomander's `src/skins.ts`: `tools/gen_skins.py`
    regenerates `src/skins_data.rs` when Nikkomander gets new skins.
- Test hooks: `NIKPAK_GUI_SKIN=<id>` starts with a skin (not saved),
  `NIKPAK_GUI_SKINMENU=1` opens the skins menu.
- Nine new interface texts, in the ten languages.

### Downloads for macOS and Linux
- **`package-unix.sh`** makes the macOS and Linux downloads of the web site:
  `binaries/nikpak-<version>-macos.zip` on a Mac (universal: Intel + Apple silicon)
  and `binaries/nikpak-<version>-linux-x86_64.tar.gz` on Linux (`ZIG=1` builds with
  cargo-zigbuild against glibc 2.28, so it also runs on older distributions).
  Each package holds nikpak, unNik, `install.sh` (installs the programs and the
  right-click menu; `--uninstall` removes them), README.txt and the licence.
- The web site has three download buttons (Windows, macOS, Linux / Unix); the
  visitor's system is highlighted.

### Not ported
- NikPak keeps its own fonts in every skin (Nikkomander's skins can name system
  fonts), and the few decorative CSS rules some skins add (bracketed folder names,
  border images) do not apply to NikPak's windows.

---

## 1.5.5 – 23 October 2026 – right-click menu "NikPak" in Explorer, Dolphin, Nemo, Nautilus and Finder

### Added
- **A "NikPak" submenu in the file manager's right-click menu**, like WinRAR's:
  - on files and folders: **Add to archive…** (asks for the name and the folder
    of the archive, then compresses in the NikPak window) and **Add to archive
    here** (the archive goes next to the items, named like a drop on the window);
  - on .nik archives: **Open with unNik**, **Extract files…** (unNik with its
    Extract box), **Extract here**, **Extract to its own folder** (a folder named
    after the archive) and Add to archive….
  - Several selected items go into **one** archive, and several selected
    archives are extracted one after the other in one window (Explorer starts
    one program per item: the first one gathers the others, `src/shell.rs`).
  - "Extract here / to its own folder" show unNik's small window with the
    progress ring (the password box for encrypted archives) and close it when
    everything is extracted; warnings and errors stay on screen.
  - The labels follow the language of NikPak (the ten languages).
- **Windows:** the setup has a new option, checked by default, "Add NikPak to the
  right-click menu of Explorer". It runs `nikpak.exe --install-menu` (per user, or
  `--all-users` for an all-users install); the uninstaller removes the menu.
  Windows 10 shows the submenu directly; Windows 11 shows it under
  **Show more options** (Shift+F10), like every program that is not an MSIX package.
  The commands run through unNik.exe, so no console window flashes.
- **Linux:** `nikpak --install-menu` adds KDE Dolphin service menus, Nemo actions,
  Nautilus scripts (Scripts › NikPak), the application/x-nikpak MIME type and an
  unNik entry for "Open with". **macOS:** Finder Quick Actions in ~/Library/Services.
  `nikpak --uninstall-menu` removes everything.
- **`buildall.ps1`** (or `npm run buildall`) builds nikpak.exe, unNik.exe and the
  two setups, and copies them with SHA256SUMS.txt to `binaries\`.
- **A second setup, `unnik-<version>-setup.exe`** (`installer\unnik.iss`): unNik
  alone, the free archive browser (LICENSE.txt 2a), for people who only receive
  .nik archives. It registers .nik files and adds an "unNik" submenu (Open,
  Extract files…, Extract here, Extract to its own folder); when NikPak is
  already installed it only installs the program, so the menus never double.
- New command-line options (used by the menu): `nikpak --add | --add-here <items>`,
  `unNik --extract | --extract-here | --extract-to <archives>`; `nikpak -help`
  describes them.

### Tests
- Unit tests: one archive for a selection started as three programs; the Linux
  menu files; every new text translated in the ten languages.
- Run on Linux under a virtual display: `--add-here` with three items started
  separately made one archive; `--extract-here` on two archives and
  `--extract-to` extracted correctly and closed the windows; an encrypted archive
  asked for its password and extracted identical files.
- The Windows .reg text and the macOS Quick Actions were generated and checked
  (the .plist files parse). Not tested here: Explorer and Finder themselves.

## 1.5.4 – 23 October 2026 – nikpak.exe and unNik.exe get their icons

- nikpak.exe and unNik.exe show their own icons in Explorer, on the taskbar and in shortcuts (Windows resources embedded by build.rs; a missing resource compiler only skips the icon).

## 1.5.3 – 23 October 2026 – packing folders with hundreds of thousands of files starts at once

### Changed (`src/archive.rs`)
- **Scanning is much faster.** The size of each file comes from the folder
  listing itself (no second look at every file), and only a file with the
  same name as the archive is checked against it (the check opens the file,
  far too slow to do a million times).
- **-m5 analysis on several threads.** Telling text from binary files reads
  the first 16 KiB of each file (was 64 KiB) with one thread per processor
  core, so a 250,000-file folder is analysed in seconds instead of minutes.
- **Live progress and Cancel while scanning and analysing.** The number of
  files found grows while the folders are scanned, then the analysis counts
  the files it has looked at; Cancel works in both steps (it only worked
  once compression had started).

## 1.5.2 – 22 October 2026 – the setup installs both programs and registers .nik files

### Changed (installer, `installer\nikpak.iss`)
- **Installs NikPak and unNik** with the files they need: nikpak.exe,
  unNik.exe, their icons, the .nik file icon, LICENSE.txt,
  THIRD-PARTY-NOTICES.txt and CHANGELOG.md.
- **Shortcuts:**
  - Start menu: **NikPak**, **unNik** and **NikPak licence**.
  - Desktop: **NikPak, always** (it was optional). unNik on the desktop is an
    option (unchecked by default).
- **.nik files are always registered** (no longer an option):
  - they show their own **archive icon** in Explorer (`assets\nikfile.ico`:
    a dark-blue page with a zipper and "NIK", like the zip icon), with the
    type name "NikPak archive" and the "compressed" kind;
  - a **double-click opens them in unNik**; the right-click menu shows
    "Open with unNik";
  - unNik is listed in "Open with" and in Windows' Default apps settings;
  - per user (normal install, no administrator rights) or for all users,
    following the install mode; the uninstaller removes all of it.
- Build the setup on Windows with `npm run installer` (or
  `.\build.ps1 -Installer`; Inno Setup 6.3+:
  `winget install JRSoftware.InnoSetup`) → `dist\nikpak-1.5.2-setup.exe`.

### Notes
- If .nik files were already set to open with another program through
  "Open with → Always", Windows keeps that choice; pick unNik once in
  "Open with" (or in Settings › Apps › Default apps).
- The program code is unchanged apart from the version number.

### Tests
- The new icon was checked at 16, 48 and 256 pixels.
- Not tested here: the setup itself (Inno Setup only runs on Windows).

---

## 1.5.1 – 22 October 2026 – switch between NikPak and unNik, shared language, "open folder" buttons, sound effects

### Added
- **Switch button** in both windows, left of the **i** button (a small ring
  with the other program's arrow):
  - NikPak → **unNik**: closes NikPak and starts unNik.exe. After a
    compression, unNik opens the archive just made.
  - unNik → **NikPak**: closes unNik and starts nikpak.exe (from the empty
    window and from the browser).
  - The other program is looked for in the same folder (`unNik.exe` /
    `nikpak.exe`). nikpak.exe is started without a console window. If it is
    missing, a clear message says so. The button is disabled while an
    operation is running.
- **"Open in unNik"** link on NikPak's done screen (next to "Show in folder"
  and "Compress more").
- **Shared language:** both programs read the same settings file, so the
  language chosen in one is used by the other. A window that is already open
  now also follows a change made in the other program within two seconds.
- `nikpak -help` mentions the switch buttons and the shared language.
- **Open the destination folder** (native Explorer / Finder window):
  - NikPak: a folder button between the "Save to" field and Browse. After a
    compression it opens the folder with the new archive selected; otherwise
    it opens the "Save to" folder.
  - unNik: a button in the browser's toolbar opens the folder of the last
    extraction (or, before any extraction, the archive's folder).
- **Sound effects**, made by the program itself (no sound files): a soft
  click for buttons, links and options, and a sound for each action —
  box or menu opening, work starting, archive opened, success (archive made,
  files extracted, test passed), error, delete, cancel, switching program,
  files hovering over the window, sounds on / off.
  - A **speaker button** in the title bar of both windows switches them on
    or off. They are **on by default**; the choice is shared by both
    programs (setting `sounds=on|off`).
  - Windows plays them with the system sound API (PlaySound, no extra
    library); macOS / Linux use afplay / paplay / aplay when available.

### Changed
- Translations for the new texts in the ten languages; the Chinese font
  subset was regenerated for the new characters.
- Guide version 1.5.1: §3.7 and §3.9 describe the switch buttons, the
  shared language, the open-folder buttons and the sounds.
- `nikpak -V` / `-help` piped into `head` or `more` no longer print a panic
  message when the pipe closes.

### Tests
- 35 unit tests pass (new: every sound is a valid WAV, audible and never
  clipped).
- On a virtual display: the speaker button writes `sounds=off` / `on`; the
  open-folder buttons are shown in both windows. NikPak → unNik and unNik → NikPak (each time the
  first program closes and the other one opens); "Open in unNik" after a
  compression opens the new archive in the browser; French chosen in unNik
  restarts unNik in French with the same archive and NikPak then starts in
  French; changing the language in the settings while NikPak is open
  switches it to Russian within two seconds.
- Not tested here: Windows itself, and hearing the sounds (this test
  machine has no audio). Please try the switch, folder and speaker buttons
  from the installed programs and from `target\release\`.

---

## 1.5.0 – 22 October 2026 – unNik, the free archive browser; selective extraction and delete

### Added
- **unNik (unNik.exe), a second program, free for everyone** (no registration,
  no reminder; LICENSE.txt §2a). It is built from the same project with the
  same `npm run build` / `cargo build --release` (Rust, egui, like NikPak).
  - **Same look as NikPak** when empty: the drop ring, with an arrow going
    **up**. Drop a .nik archive, click the ring, or pick a recent archive.
    `unNik archive.nik` opens one directly.
  - **The browser** (the window grows to 1120 × 720, resizable, can be
    maximised):
    - folder tree, breadcrumbs, Back / Forward / Up, double-click into
      folders;
    - search by name in the whole archive (Ctrl+F);
    - sortable list (name, size, type, date) with check boxes, Ctrl and
      Shift selection, keyboard navigation and a right-click menu;
    - details pane (size, date, folder, CRC32, how the file is stored,
      attributes) and a compression summary (space saved, method,
      encryption).
  - **Extract** all or the selection: destination (default: a folder named
    after the archive), keep the folders or not, and replace / keep both /
    skip existing files. Files taken from inside a folder are created
    without that folder's path, as in WinRAR. Reading stops after the last
    chosen file.
  - **View** text files (line numbers, find with highlighting, wrap, copy) and
    binary files (hex), up to 16 MB; **Open in Notepad** opens the whole
    file; **Open** starts a file with its own program.
  - **Delete** files and folders from an archive (with confirmation).
  - **Test** the archive. Encrypted archives ask for their password.
  - Progress ring with Cancel for every operation; ten languages, the
    language button, the About box and the Telegram / X / Discord buttons.
  - File names in any script use the system's fonts as a fallback (Segoe UI,
    Microsoft YaHei, Malgun Gothic, Nirmala UI … on Windows).
- **Command line:**
  - `nikpak d archive.nik names…` deletes files or folders (a folder takes
    its contents). The archive is written again without them, in a
    temporary file that replaces it only at the end; encrypted archives
    keep their password; media / Deflate-recompressed entries and the -m5
    engines are kept.
  - `nikpak p archive.nik [names…]` prints files to standard output.
  - Extracting **one file, one folder or a wildcard** already worked
    (`nikpak x backup.nik Docs/report.pdf`, `nikpak e … -d C:\Out`); it is now
    explained in `-help` (new SELECTING FILES section and examples), as is
    listing only some files (`nikpak l backup.nik *.pdf`).
- **Installer:** installs unNik.exe with its own icon (`assets/unnik.ico`) and
  Start menu entry, and the task "Open .nik archives with unNik" (checked by
  default) associates .nik files with unNik. `build.ps1` / `build.sh` copy
  both programs to `dist\`.

### Changed
- The code is now a library (`src/lib.rs`) used by both programs
  (`src/main.rs` = nikpak, `src/bin/unnik.rs` = unNik; the browser is
  `src/unnik.rs`). gui.rs shares its drawing helpers.
- Font subsets regenerated for the new translations (Noto Sans SC,
  Devanagari, Bengali, Arabic); JetBrains Mono (SIL OFL) added for unNik's
  viewer. assets/fonts/OFL.txt and THIRD-PARTY-NOTICES.txt updated.
- Guide version 1.5.0: new §3.9 "unNik, the free archive browser" (with
  screenshots), commands `p` and `d`, more examples, source map, installer,
  tests and limitations updated. The table of contents page numbers after
  §3 move by up to seven pages: update the field in Word (F9).

### Fixed
- **Prices:** the 1.4.4 volume prices had not reached every file in your
  folder (and `nikpak -V` still showed the 1.4.0 prices "$10 / $50 per
  seat"). Now everywhere: Personal $19; per seat 1–9 $19, 10–24 $15, 25–49
  $12, 50–99 $9, 100–199 $7, 200–499 $6, 500+ $5 (`nikpak -V`, `-help`,
  LICENSE.txt, `nikpak_price()`, server/README.md).

### Tests
- 34 unit tests pass (31 before), including: reading the index, extracting
  one file from inside a folder, deleting a folder and deleting everything
  for -m5, zstd + encryption and stored archives; wrong password;
  tree building, text decoding, viewer wrapping and search.
- Command line: single-file extraction, filtered listing, `p`, and `d` on
  stored, zstd, -m5, encrypted and media / office / gzip archives, each
  checked with `nikpak t` and by comparing extracted files.
- unNik on a virtual display: opening (also a 600 MB encrypted archive),
  wrong and right password, navigation, Ctrl / Shift selection, search,
  viewer (text, find, wrap, hex), extract (files identical to the
  originals), delete, test, About, and English, Chinese, Hindi, Arabic and
  Russian screens.
- Not tested here: Windows itself (file association, Notepad, system fonts,
  window resizing from the edges). Please try `unNik.exe` after
  `npm run build`, and the setup.

---

## 1.4.5 – 22 October 2026 – ten languages, Cancel button, window sized for X posts

### Added
- **Ten languages:** the ten most spoken in the world (Ethnologue 2025): English,
  中文 (Mandarin Chinese), हिन्दी (Hindi), Español, العربية (Arabic), Français, বাংলা
  (Bengali), Português, Русский (Russian), Bahasa Indonesia.
  - **The language button** sits at the bottom left: a flag and the
    language's own name. It opens a menu with flags; choosing a language
    saves it and **restarts NikPak automatically** in that language.
  - The first time, NikPak follows the language of Windows (or of the
    system); otherwise it uses English.
  - Everything in the window is translated, including licence errors and
    dialog titles. The command line stays in English.
  - Hindi, Bengali and Arabic are drawn correctly: letters join, vowel
    signs are placed properly, and Arabic reads right to left. egui cannot
    do this alone, so NikPak shapes these texts itself (rustybuzz,
    unicode-bidi, ab_glyph; new `textshape.rs`).
  - Long translations shrink to fit their buttons, and the reminder box
    grows with its text.
  - The translations (`i18n.rs`) were made with machine assistance; a
    review by native speakers is welcome.
- **Cancel button** during compression (a real button, plus the Esc key).
  It stops at once, removes the unfinished archive and keeps the window
  open.

### Changed
- **Window 500 × 560** (was 440 × 690), about 9:10, so a screenshot of the
  whole window fits an X picture post without cropping. The ring, logo and
  text are more compact.
- Social buttons moved to the bottom right, next to the language button.
- **Fonts:**
  - Inter now also covers Cyrillic and more Latin accents.
  - New subsets: Noto Sans SC (Chinese), Noto Sans Devanagari, Noto Sans
    Bengali and Noto Sans Arabic (all SIL OFL; about 900 KB of fonts in
    total).
  - assets/fonts/OFL.txt and THIRD-PARTY-NOTICES.txt are updated.
- Guide version 1.4.5: §3.7 covers languages, Cancel and the new size, with
  new screenshots; architecture, dependencies and tests are updated.

### Tests
- 31 unit tests pass, including:
  - every language translates every text with the same placeholders;
  - Devanagari and Arabic shaping;
  - language codes.
- On a virtual display, all ten languages were checked screen by screen:
  reminder, idle, administrator notice, progress with Cancel, done, About,
  registration and the language menu.
- Choosing French in the menu saved `language=fr` and restarted NikPak in
  French.
- Not tested here: Windows itself. Please check the restart after a
  language change and the Windows default language.

---

## 1.4.4 – 22 October 2026 – gentler volume prices (RarLab-style steps)

### Changed
- **Company prices per seat (USD):** they now step down gently like WinRAR's
  ($21 → $16 → $13 → $10 → $8 → $7 → $6), with no steep $9 → $5 drop at 500 seats:

  | Seats | 1–9 | 10–24 | 25–49 | 50–99 | 100–199 | 200–499 | 500+ |
  |---|---|---|---|---|---|---|---|
  | 1.4.3 | $19 | $15 | $13 | $11 | $10 | $9 | $5 |
  | **1.4.4** | **$19** | **$15** | **$12** | **$9** | **$7** | **$6** | **$5** |

  The personal licence stays at $19. Updated in `nikpak -help`, LICENSE.txt,
  `nikpak_price()`, server/README.md and the guide.

---

## 1.4.3 – 22 October 2026 – new prices, new copyright, About box

### Changed
- **Prices (USD, excluding VAT):**
  - **Personal licence: $19** (was $10).
  - **Company licences, per seat** (was a flat $50 per seat):

    | Seats | 1–9 | 10–24 | 25–49 | 50–99 | 100–199 | 200–499 | 500+ |
    |---|---|---|---|---|---|---|---|
    | per seat | $19 | $15 | $13 | $11 | $10 | $9 | $5 |

  - The structure follows RarLab's WinRAR price list (single $29; 2–9 $21 …
    500–999 $6).
- **Updated everywhere:**
  - the reminder (Personal $19, Company "from $5 / seat");
  - `nikpak -help` (full table);
  - `nikpak -V` / `nikpak license`;
  - LICENSE.txt §2–3;
  - server/README.md;
  - the guide §3.8 and its screenshot.
- **Server kit:** new `nikpak_price($type, $seats)` returns the per-seat price
  and the total, so buy.php and the webhook compute the amount on the server.
- Existing licence keys are unaffected; keys don't contain a price.
- **Copyright:** "© 2026 Nicolas Choukroun, distributed by Nikkoid LLC". This
  appears in LICENSE.txt (owner: Nicolas Choukroun; distributor and licensor:
  Nikkoid LLC), `nikpak -V`, `-help`, the evaluation line, the installer
  (publisher "Nikkoid LLC") and the guide.

### Added
- **About box:** a small **i** button at the top right of the window (next to
  minimise and close) opens it. It shows the version, the copyright, the
  distributor, the licence status, a link to nikpak.nikkoid.com and a
  "Licence terms" button that opens LICENSE.txt. The version/© line at the
  bottom of the window was moved into this box.
- The reminder no longer mentions the copyright (it's in the About box).

### Fixed
- Guide: the 1.4.0 "Licence" test row was in the feature table (§2.1); it is
  now in the tests table.

---

## 1.4.2 – 22 October 2026 – first test on Windows; dialogs in front

### Fixed
- **File dialogs opened behind the NikPak window on Windows.** The Choose
  files, Choose a folder and Browse dialogs are now attached to the NikPak
  window, so they always appear in front of it.

### Tests (first run on Windows 10/11, through the installer)
- **Installer:** installs and creates the Start menu entry, and NikPak starts from it.
- **Reminder:** shown at start. The Register / Test for free order changes
  between starts.
- **Registration box:** typing and pasting work. A wrong key shows its error
  inside the box. × closes it.
- **Main window:** the version line "NikPak 1.4.x · © 2026 Nikkoid" and the
  chooser in the circle work.
- **Administrator restart:** a NikPak started as administrator replaces itself
  a few seconds later with a normal-user window, as designed.

---

## 1.4.1 – 22 October 2026 – Windows installer

### Added
- **Installer (Inno Setup):** `.\build.ps1 -Installer` (or `npm run installer`)
  builds `dist\nikpak-<version>-setup.exe`. This is the file to publish on
  shareware sites.
  - It needs Inno Setup 6.3+, installed once with `winget install JRSoftware.InnoSetup`.
  - The script is `installer\nikpak.iss`; the version comes from Cargo.toml.
- **What the setup does:**
  - It installs for the current user by default, without administrator
    rights, into `%LOCALAPPDATA%\Programs\NikPak`. Installing for all users
    is optional.
  - It asks the user to accept LICENSE.txt.
  - It adds a Start menu entry, an optional desktop icon and, on request,
    the PATH entry for the command line.
  - It registers an uninstaller in Settings > Apps.
  - "Launch NikPak" at the end starts it as the normal user, never as
    administrator.
  - Updates install over the old version, and the licence is kept.
  - The setup runs in English or French.
- **NikPak icon** `assets\nikpak.ico`, used by the setup, the shortcuts and the
  uninstaller.
- Guide version 1.4.1: §13.1 "The Windows installer".

### Tests
- The build, the version and the unit tests were checked here.
- Not tested here: the setup itself, because Inno Setup only runs on Windows.

---

## 1.4.0 – 22 October 2026 – shareware edition: registration, licence keys, licence server kit

### Added
- **Shareware registration (modelled on WinRAR).** NikPak is (c) 2026 Nikkoid.
  - Individuals may use the free evaluation copy forever.
  - Companies need one licence per seat.
  - Prices: Personal **$10**, Company **$50 per seat**, sold at https://nikpak.nikkoid.com/buy.php.
- **Reminder at start** (unregistered copies only):
  - It has two buttons, **Register** and **Test for free**. Their left/right
    order is random at every start.
  - Test for free (or Esc) closes it, and NikPak works normally with no
    limits.
- **Registration box:** name, e-mail and licence key; all three must match.
  - A wrong combination shows a clear error inside the box.
  - "Buy a licence" opens the web page.
  - Once registered, NikPak never asks again on that machine. The same key
    works on any number of the holder's machines.
- **Licence keys:** `NPK1-…` keys are Ed25519 signatures of the normalised
  name and e-mail.
  - Case and extra spaces don't matter.
  - O/0 and I/L/1 mix-ups are tolerated.
  - nikpak contains only the public key, so keys cannot be forged from the
    program.
- **Command line:**
  - `nikpak register "Name" email KEY`, `nikpak license`, `nikpak unregister`;
  - `-V` and `-help` show the licence status;
  - unregistered copies print one evaluation line on stderr (silent with `-q`).
- **Company deployment:** a `nikpak.key` file next to nikpak.exe registers
  every user on that PC.
- **Licence tool** (private, never shipped):
  `cargo run --release --features keygen --bin nikpak-keygen -- newkeys | make | verify`.
- **Licence server kit** `server/` (PHP 7.2+, MySQL 5.7):
  - key generation that is byte-identical to nikpak;
  - a licence table;
  - idempotent issuing per payment;
  - resend by e-mail;
  - a `nikpak.key` file builder;
  - a CLI tool and a README (setup, payment webhook, VAT note).
- **LICENSE.txt** (EULA: shareware terms, free distribution of the
  unmodified package on download sites, no royalties on .nik archives) and
  **THIRD-PARTY-NOTICES.txt** (all 351 open-source components and their
  licences).
- **GUI:**
  - Clicking the circle opens **Choose files / Choose a folder**.
  - The version line "NikPak 1.4.0 · © 2026 Nikkoid" is shown at the
    bottom.
  - The top-left shows "EVALUATION COPY · REGISTER" or "REGISTERED TO …".
- **Copy & paste** in text fields (to paste the licence key).

### Changed
- **Administrator mode:** if nikpak.exe is started as administrator, which
  blocks drag and drop in Windows, it restarts itself once as a normal user
  so drag and drop works.
- All wrapped text in the boxes (descriptions, errors) is centred and stays
  inside the box.
- **No more LGPL code:** preflate-rs is now vendored (`vendor/preflate-rs`,
  Apache-2.0), and its LGPL `cabac` dependency is replaced by our own range
  coder (`rangecoder.rs`). This matters for a closed-source shareware
  product. Office compression is unchanged or slightly better (office set:
  6,869,247 bytes vs 6,887,729).

### Compatibility
- Office/PDF/gzip files recompressed by the 1.2–1.3 **test builds** used
  recompression method 1, which needed the removed coder. Extracting them
  now gives a clear message to use nikpak 1.3.1. All other archives are
  unchanged.

### Tests
- 26 unit tests pass, including licence key round trip and rejection of
  wrong name, e-mail or forged keys. All 52 upstream preflate-rs tests pass
  with the new coder.
- On a virtual display, the full flow was tested: reminder → register →
  wrong key error → correct key → thank you → restart shows REGISTERED.
  Both button orders, the chooser, and text staying inside the boxes were
  checked.
- Server kit: keys issued by PHP are accepted by nikpak. Repeating a
  payment returns the same key.
- Not tested here: Windows itself (administrator restart, native dialogs).

---

## 1.3.1 – 21 October 2026 – drag and drop fix, choose files, brand-coloured links

### Fixed
- **Drag and drop did not work for you.** The window's drop handling was
  tested with a real drag on a virtual display, and it works. The usual Windows
  cause is **administrator mode**: Windows (UIPI) blocks dragging from
  Explorer onto a program running as administrator, for example nikpak.exe
  started from the administrator PowerShell used for building. Changes:
  - **Click the circle**, or use the new links **choose files** /
    **choose a folder**, to pick what to compress in a normal dialog. This
    works even as administrator.
  - When nikpak runs as administrator, the window shows a notice explaining
    this.
  - **Tip:** start nikpak.exe by double-clicking it in Explorer, or from a
    normal (not administrator) PowerShell, and drag and drop works.

### Changed
- **Social links:** the Telegram, X and Discord icons are now larger, round
  buttons in their brand colours, centred under the Save to field. Hovering
  one lights it up and shows the link. They are no longer at the top right.
- **Done screen:** a new "Compress more" link, and a click on the green
  circle picks new files.
- The window is a little taller (690 px) to fit the new elements.
- Guide version 1.3.1: §1.5 and §3.7 updated (new screenshots, the
  administrator note).

### Tests
- 23 unit tests pass.
- On a virtual display: a real drag and drop of a folder (via XDND) created
  and verified the archive. Done → Compress more → idle works, and clicking
  "choose files" keeps the window responsive. Screenshots were checked for
  the normal and administrator layouts.
- Not tested here: Windows itself (this environment cannot build for Windows).

---

## 1.3.0 – 21 October 2026 – drop window and community links

### New
- **Graphical interface** (new file `src/gui.rs`). Started without arguments,
  for example by double-clicking nikpak.exe, nikpak opens a minimalist,
  frameless window instead of printing its usage:
  - It shows "Drop a file or a folder to compress it" with a drop icon in the
    centre, and accepts drag and drop of one or more files and folders.
  - Dropped items are compressed with the **best compression (-m5)**. The
    archive is named after the file, the folder, or (for several items) their
    folder, and an existing archive is never overwritten
    (`report (2).nik`).
  - Progress shows as a big percentage inside a blue circular meter, with the
    phase (analysing / compressing / finishing), the amount processed and the
    time. **Cancel** removes the unfinished archive, and closing the window
    does the same.
  - When the archive is done, the ring turns green and shows the archive size
    against the original and the time, with **Show in folder**.
  - **Save to** at the bottom: the destination folder can be typed in or
    chosen with **Browse**, and is remembered for next time (Windows:
    `%APPDATA%\nikpak\settings.txt`). The first time, it is the Desktop.
  - Design: dark-blue gradient, the NikPak logo at the top with a thin,
    semi-transparent glow, and the Inter typeface (embedded Latin subset,
    SIL Open Font License, `assets/fonts/OFL.txt`).
  - On Windows, the console window that Explorer opens for nikpak.exe is
    closed automatically; a command prompt you started it from is left alone.
    Without a display (e.g. over SSH), nikpak prints the usage as before.
- **Community links** for Telegram (https://t.me/cryptonikko), X
  (https://x.com/cryptonikkoid) and Discord
  (https://discord.com/users/1537826350759415811):
  - as clickable icons at the top right of the window (hover shows the name
    and link);
  - in `nikpak -help` (COMMUNITY section);
  - in the guide (new §1.5, with clickable links).

### Changed
- The executable is larger with the window: about 9 MB on Linux, against
  2.3 MB before. A command-line-only build is still possible:
  `cargo build --release --no-default-features`.
- Guide version 1.3.0 (50 pages): new §1.5 Community and §3.7 The graphical
  interface (with screenshots). The source map now lists engine.rs, recomp.rs
  and gui.rs. Executable sizes, the "graphical interface" comparison with
  WinRAR and the roadmap are updated.

### Requirements
- New libraries: eframe/egui 0.33 (window, OpenGL) and rfd 0.15 (folder dialog).
  Rust 1.89 is still enough.

### Tests
- 23 unit tests pass, and the CLI behaves as before.
- The window was run on a virtual display (Xvfb). The idle, hover, progress,
  done and cancelled screens were checked visually. An archive made from the
  window extracts byte for byte. Cancel and closing during compression leave
  no file behind.
- Not tested here: running on Windows and macOS (this environment cannot
  build for them). The Windows-specific code was checked by hand. Please
  report anything that looks wrong.

---

## 1.2.1 – 21 October 2026 – best compression by default

### Changed
- **The default level is now -m5 (maximum compression).** `nikpak a archive.nik files`
  now gives the smallest archive without any option: LZMA2/PPMd plus Deflate
  recompression. It is smaller than 7-Zip -mx9 on all 10 test sets
  (2.4–31.7 %). The 60 office files give 6,887,729 bytes with no option at all.
- For speed instead of size, ask for **-m2**. It is the old default, often 50
  times faster (1.7 s instead of 154 s for 190 MB of source code on one core).
- The help text (`nikpak -help`) and the guide (options §3.2, level table §6.2,
  §11.7) were updated. The guide is now "Version 1.2.1".

### Note
- At the new default, creating an archive is single-threaded, needs up to
  about 1.1 GB of memory, and is slower than 7-Zip on large office and
  source-code sets (see 1.2.0). Archives are format version 3 and need
  nikpak 1.2 or newer, as in 1.2.0.

### Tests
- 23 unit tests pass. An archive made without a level option uses the
  multi-engine method (2), is byte-identical in size to -m5, and extracts
  exactly. -m2 still gives a Zstandard archive (method 1).

---

## 1.2.0 – 21 October 2026 – beats 7-Zip on office files, text and source code

**Goal of this release:** nikpak must make smaller archives than every other
compressor, 7-Zip included. In the 1.1.0 benchmark (guide §11.7), 7-Zip -mx9
was 1.4 % smaller on text, 3.3 % on office files and 8.5 % on source code.

### New
- **Deflate recompression** (new file `src/recomp.rs`, filter 3). This applies to
  .docx, .xlsx, .pptx, .odt, .ods, .epub, .jar and .zip (ZIP containers), to .pdf
  (FlateDecode streams) and to .gz files. They are detected by content, never
  by extension. Their inner Deflate streams are decoded, the plain content is
  compressed far better, and the original Deflate bytes are rebuilt bit for
  bit on extraction with Microsoft's preflate-rs. Every file is decoded again
  while it is archived and its CRC32 compared; on any doubt it is stored
  normally. This applies at -m4 and -m5.
- **New maximum level -m5 (also -9): multi-engine archive body** (new file
  `src/engine.rs`, archive method 2):
  - Entries are split into a *binary block* (LZMA2, dictionary up to
    128 MiB) and a *text block* (PPMd variant I order 8, 192 MiB, or
    LZMA2). A trial on four contiguous 1 MiB samples picks the engine.
  - Like 7-Zip, files are sorted by extension and name, so similar and
    identical files sit next to each other.
- **Version numbering:** every delivered build increases the version. This
  update log ships with every delivery.

### Results (one core, same machine and files as guide §11.7, all 92 archives verified)
`-m5` is now the smallest of all four tools on **all 10 test sets**: 2.4–31.7 % smaller
than 7-Zip -mx9, 10.8–38.4 % smaller than RAR 7 -m5, 19.9–60.4 % smaller than ZIP -9.

| Set | nikpak 1.1.0 -m5 | **nikpak 1.2.0 -m5** | 7-Zip -mx9 | 1.2.0 vs 7-Zip |
|---|---|---|---|---|
| 60 office files (doc/docx/xls/xlsx/csv/pdf) | 10,417,750 | **6,887,729** | 10,085,779 | 31.7 % smaller |
| 18 text books (.txt) | 3,306,819 | **2,741,898** | 3,259,868 | 15.9 % smaller |
| 190 MB of source code and docs | 31,322,041 | **28,175,618** | 28,879,398 | 2.4 % smaller |
| 8 BMP images | 3,629,313 | **3,557,350** | 4,195,043 | 15.2 % smaller |
| 16 MOD files | 1,139,168 | **1,127,607** | 1,166,855 | 3.4 % smaller |

Honest costs: -m5 is slower than 7-Zip -mx9 on office files (81 s vs 26 s) and
source code (154 s vs 121 s), but faster on text (6.4 s vs 12.9 s). Text extracts
more slowly (2.7 s vs 0.2 s, because PPMd decodes as slowly as it encodes). The default
level -m2 is unchanged: it is still the fastest, but 7-Zip's default -mx5 makes smaller
archives of text, office files and source code.

### Changed
- `-m5` / `-9` now use LZMA2/PPMd instead of zstd -19 (see the costs above).
- `-m4` also recompresses Deflate files. On office files it is now 1.3 %
  smaller than 7-Zip -mx9 (16 s vs 26 s) but slower than before.
- `-m1` to `-m3` are unchanged (Zstandard, same speed).
- `-mc-` now switches off all content filters (media codecs **and** Deflate
  recompression).
- Archive format version 3. nikpak 1.2 reads all older archives (versions 1
  and 2). nikpak 1.1 refuses version 3 archives with "please update nikpak".
- Release builds use `panic = "unwind"`: preflate-rs can panic on malformed
  data, and nikpak now catches that and reports a damaged archive (exit 3),
  or stores a hostile input file normally, instead of crashing.

### Requirements
- **Rust 1.89 or newer** (needed by preflate-rs). `build.ps1` and `build.sh`
  check this and run `rustup update stable` when needed. With npm:
  `rustup update`, then `npm run build`.
- New libraries: liblzma 0.4 (built statically from C sources), ppmd-rust 1.5,
  preflate-rs 0.7. The test-only library is flate2.
- Memory at -m5: creating needs up to about 1.1 GB (1,064 MB measured on 190 MB
  of source code), and extracting up to about 350 MB (107 MB measured).

### Tests
- 23 unit tests (13 before). New tests cover Deflate round trips (ZIP at levels
  1/3/6/9, PDF, multi-member gzip), random damage to recompressed data, block
  round trips around frame boundaries, damaged and truncated blocks, hostile
  engine parameters, and the engine trial.
- 1.1 archives (store, -m2, -m5, plain and encrypted) open in 1.2.
- 60 randomly damaged or truncated -m5 archives all give exit 3, with no crash.
- Fake .docx, .pdf and .gz files filled with garbage are archived and
  restored byte for byte.

---

## 1.1.0 – October 2026 – media intelligence
- Media filters chosen per file by trial: the NAC lossless audio codec (WAV,
  AIFF, MOD samples), Shuffle and Shuffle+Delta, and the MED image predictor
  (BMP).
- Archive format version 2 (media records). Version 1 is still readable.
- `mt` command (media test), `-mc-` / `-mc+`.
- 128 MiB window and long-distance matching at -m4 and -m5.

## 1.0.0 – October 2026 – first version
- The .nik format: solid Zstandard stream; Argon2id + XChaCha20-Poly1305
  encryption with hidden file names; CRC32 per file; end marker.
- RAR-compatible command line (a x e l v lb t), exit codes, password
  generator, safe extraction (zip-slip, symlinks, setuid).
