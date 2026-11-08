//! Split volumes and self-extracting archives.
//!
//! # Volumes
//!
//! A large archive can be cut into volumes while it is written:
//!
//! ```text
//!   photos.nik     volume 1 (the archive's first bytes)
//!   photos.n01     volume 2
//!   photos.n02     volume 3 ...
//! ```
//!
//! The volumes are the archive's own byte stream cut at `size` byte
//! boundaries, so reading is reading the files one after the other (see
//! [`Chain`]). Every volume except the last is exactly the same size, which is
//! how the reader knows whether another one should follow: a full volume whose
//! successor is missing makes it fail with "the archive continues in ..., which
//! is missing" instead of a puzzling "unexpected end of file".
//!
//! Volume 1 is a normal `.nik` file (the format knows nothing about the
//! split), so an older nikpak still recognizes it - and reports a truncated
//! archive, which is exactly what it is without its volumes.
//!
//! # Self-extracting archives
//!
//! A self-extracting archive is `unNik.exe` with the archive appended:
//!
//! ```text
//!   [ unNik.exe ][ archive bytes ][ trailer 40 bytes ]
//! ```
//!
//! The trailer says where the archive starts and how long it is, and carries a
//! CRC32 of itself, so a plain `unNik.exe` is never mistaken for one. unNik
//! looks for it in its own executable at start-up ([`self_embedded`]) and then
//! shows a small "extract" window instead of its drop window; nikpak and unNik
//! opened normally read such a file too, because [`Chain`] starts at the
//! archive's offset.
//!
//! A split self-extracting archive has the first volume inside the `.exe` and
//! the rest in `photos.n01` … next to it.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::Relaxed;

use crate::archive::PROGRESS;

/// The smallest volume accepted: the archive header (magic, salt, nonce, the
/// legal statement and the cover) has to fit in it, and a split that produces
/// thousands of files is never what is wanted.
pub const MIN_VOLUME: u64 = 64 * 1024;

/// How many volumes are looked for at most.
const MAX_VOLUMES: u32 = 90_000;

// ---------------------------------------------------------------------------
// Volume names
// ---------------------------------------------------------------------------

/// The file of volume `index` (1 = the first): the archive's own name for the
/// first volume, then `name.n01`, `name.n02` … `name.n99`, `name.100` …
pub fn volume_name(main: &Path, index: u32) -> PathBuf {
    if index <= 1 {
        return main.to_path_buf();
    }
    let n = index - 1;
    let stem = main.file_stem().map(|s| s.to_os_string()).unwrap_or_else(|| main.as_os_str().to_os_string());
    let mut name = stem;
    name.push(if n < 100 { format!(".n{n:02}") } else { format!(".{n}") });
    main.with_file_name(name)
}

/// The name a self-extracting archive takes: `photos.nik` -> `photos.exe`.
pub fn sfx_name(main: &Path) -> PathBuf {
    let stem = main.file_stem().map(|s| s.to_os_string()).unwrap_or_else(|| main.as_os_str().to_os_string());
    let mut name = stem;
    name.push(if cfg!(windows) { ".exe" } else { ".run" });
    main.with_file_name(name)
}

/// Is this file the first volume of a set (does a `.n01` sit beside it)?
pub fn is_split(main: &Path) -> bool {
    extra_volume(main, 2).is_some()
}

/// The first volume of the set `path` is a volume of: `photos.n01` (or
/// `photos.100`) -> `photos.nik` - or `photos.exe`, when the set is
/// self-extracting. `path` itself when it is not a volume of a set.
///
/// Every volume opens the **whole** archive (a `Chain` always starts at volume
/// 1), so a program given a volume - dropped on unNik or on the NikPak window,
/// or chosen in a file dialog - opens the set the way the user meant instead of
/// failing with "bad signature".
pub fn first_volume(path: &Path) -> PathBuf {
    let Some(name) = path.file_name().map(|s| s.to_string_lossy().into_owned()) else {
        return path.to_path_buf();
    };
    let Some(dot) = name.rfind('.') else { return path.to_path_buf() };
    let (stem, ext) = (&name[..dot], &name[dot + 1..]);
    // `.n01` … `.n99` are only ever volumes; `.100` … are digits, which any file
    // could have, so those also need a first volume that really is an archive
    let n_form = ext.len() == 3
        && ext.as_bytes()[0].eq_ignore_ascii_case(&b'n')
        && ext[1..].bytes().all(|c| c.is_ascii_digit());
    let digits = ext.len() >= 3 && ext.bytes().all(|c| c.is_ascii_digit());
    if stem.is_empty() || !(n_form || digits) {
        return path.to_path_buf();
    }
    // volume 1 is `photos.nik`, or `photos.exe` for a self-extracting set
    for suffix in [".nik", if cfg!(windows) { ".exe" } else { ".run" }] {
        let mut first = stem.to_string();
        first.push_str(suffix);
        let candidate = path.with_file_name(first);
        if !candidate.is_file() {
            continue;
        }
        // a plain `.n01` is unambiguous; a numeric extension is only believed when
        // what it points at is a nikpak archive (or a self-extracting one)
        if n_form || embedded(&candidate).is_some() || starts_with_magic(&candidate) {
            return candidate;
        }
    }
    path.to_path_buf()
}

/// Does the file start with the archive magic? (8 bytes read)
fn starts_with_magic(p: &Path) -> bool {
    let mut magic = [0u8; 8];
    File::open(p).and_then(|mut f| f.read_exact(&mut magic)).is_ok() && magic == crate::format::MAGIC
}

/// The size of volume `index` of `main`, when it is there. An empty file is
/// not a volume: a set whose last volume is filled exactly to its size gets an
/// empty successor from the writer (that is how the reader tells "full" from
/// "last"), and that one does not count.
fn extra_volume(main: &Path, index: u32) -> Option<u64> {
    let m = fs::metadata(volume_name(main, index)).ok()?;
    (m.len() > 0).then_some(m.len())
}

/// How many volumes of `main` are on the disk.
pub fn volume_count(main: &Path) -> u32 {
    let mut n = 1;
    while n < MAX_VOLUMES && extra_volume(main, n + 1).is_some() {
        n += 1;
    }
    n
}

/// The bytes the archive takes on the disk, all volumes together (and without
/// the executable in front of a self-extracting one). The window's "done" line
/// uses it, so a split archive is not reported as one volume.
pub fn total_size(main: &Path) -> u64 {
    let mut total = match embedded(main) {
        Some(e) => e.len,
        None => fs::metadata(main).map(|m| m.len()).unwrap_or(0),
    };
    let mut k = 2;
    while k < MAX_VOLUMES {
        match extra_volume(main, k) {
            Some(len) => {
                total += len;
                k += 1;
            }
            None => break,
        }
    }
    total
}

/// Parse a volume size: `700m`, `4.7g`, `1024k`, `700 MB`, `2G`, `471859200`.
/// Returns bytes, or None when the text is not a size.
pub fn parse_size(text: &str) -> Option<u64> {
    // "700m", "700 MB", "700MB", "4.7g", "4.7 GiB", "471859200": the number
    // runs up to the first letter, and the unit is everything after it - which
    // may be two letters, so taking just the last character is not enough
    let t: String = text.trim().replace(' ', "").replace(',', ".");
    if t.is_empty() {
        return None;
    }
    let split = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '+' || c == '-')).unwrap_or(t.len());
    let (num, unit) = (&t[..split], t[split..].to_ascii_lowercase());
    let mult: u64 = match unit.as_str() {
        "" | "b" | "o" => 1,
        "k" | "kb" | "kib" => 1024,
        "m" | "mb" | "mib" => 1024 * 1024,
        "g" | "gb" | "gib" => 1024 * 1024 * 1024,
        "t" | "tb" | "tib" => 1024u64 * 1024 * 1024 * 1024,
        _ => return None,
    };
    let value: f64 = num.parse().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }
    let bytes = value * mult as f64;
    if bytes < 1.0 || bytes > u64::MAX as f64 / 2.0 {
        return None;
    }
    Some(bytes.round() as u64)
}

/// A size as it is shown in the window: "700 MB", "4.70 GB", "1.50 TB".
pub fn size_text(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1000.0 && i < U.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else if v >= 100.0 {
        format!("{v:.0} {}", U[i])
    } else if v >= 10.0 {
        format!("{v:.1} {}", U[i])
    } else {
        format!("{v:.2} {}", U[i])
    }
}

// ---------------------------------------------------------------------------
// The self-extracting trailer
// ---------------------------------------------------------------------------

/// `NIKPAKSFX`, a version byte and two spare bytes (the trailer carries a CRC,
/// so a longer one would be a new magic).
pub const SFX_MAGIC: [u8; 12] = *b"NIKPAKSFX\x01\x00\x00";
/// The magic, offset:u64, len:u64, total:u64 and crc32:u32.
pub const SFX_TRAILER: u64 = 40;

/// The archive hidden in a self-extracting executable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Embedded {
    /// where the archive's bytes start in the file
    pub offset: u64,
    /// how many bytes of the archive this file holds
    pub len: u64,
}

/// Is `path` a self-extracting executable made by nikpak / unNik? Reads only
/// the last 40 bytes.
pub fn embedded(path: &Path) -> Option<Embedded> {
    let mut f = File::open(path).ok()?;
    let total = f.metadata().ok()?.len();
    if total < SFX_TRAILER {
        return None;
    }
    f.seek(SeekFrom::Start(total - SFX_TRAILER)).ok()?;
    let mut t = [0u8; SFX_TRAILER as usize];
    f.read_exact(&mut t).ok()?;
    if t[..12] != SFX_MAGIC {
        return None;
    }
    let crc = u32::from_le_bytes(t[36..40].try_into().ok()?);
    if crc32fast::hash(&t[..36]) != crc {
        return None;
    }
    let offset = u64::from_le_bytes(t[12..20].try_into().ok()?);
    let len = u64::from_le_bytes(t[20..28].try_into().ok()?);
    // the archive has to lie inside the file, in front of the trailer
    if offset.checked_add(len).is_some_and(|end| end <= total - SFX_TRAILER) {
        Some(Embedded { offset, len })
    } else {
        None
    }
}

/// The archive appended to *this* program, when it is a self-extracting one.
pub fn self_embedded() -> Option<Embedded> {
    embedded(&std::env::current_exe().ok()?)
}

fn trailer_bytes(offset: u64, len: u64) -> [u8; SFX_TRAILER as usize] {
    let mut t = [0u8; SFX_TRAILER as usize];
    t[..12].copy_from_slice(&SFX_MAGIC);
    t[12..20].copy_from_slice(&offset.to_le_bytes());
    t[20..28].copy_from_slice(&len.to_le_bytes());
    t[28..36].copy_from_slice(&(offset + len).to_le_bytes());
    let crc = crc32fast::hash(&t[..36]);
    t[36..40].copy_from_slice(&crc.to_le_bytes());
    t
}

/// The executable copied in front of a self-extracting archive: unNik, which is
/// installed next to nikpak and carries the unpack window.
pub fn stub() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let want = if cfg!(windows) { "unnik.exe" } else { "unnik" };
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.is_file() && p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(want)))
}

// ---------------------------------------------------------------------------
// Writing: one file, a set of volumes, or an .exe
// ---------------------------------------------------------------------------

/// Where an archive is written: a single file, a set of volumes, or a
/// self-extracting `.exe` (which may be split too).
///
/// Everything is first written into `….tmp` files beside the final names and
/// renamed into place by [`Out::finish`], so a cancelled or failed compression
/// leaves nothing behind (a dropped `Out` removes its temporary files).
pub struct Out {
    base: PathBuf,
    /// bytes per volume (None: one file)
    size: Option<u64>,
    /// the executable copied in front of the archive, and its length
    sfx: Option<(PathBuf, u64)>,
    /// the volume being written (1 = the first)
    vol: u32,
    cur: Option<File>,
    /// bytes written into the volume being written
    written: u64,
    /// temporary file of each volume written so far, in order
    done: Vec<PathBuf>,
}

impl Out {
    /// Start writing the archive `base`. `sfx` is `(executable, its length)`:
    /// its bytes are copied in front of the archive, and the trailer is added.
    pub fn create(base: &Path, split: Option<u64>, sfx: Option<(PathBuf, u64)>) -> io::Result<Out> {
        let mut o = Out {
            base: base.to_path_buf(),
            size: split.map(|s| s.max(MIN_VOLUME)),
            sfx,
            vol: 0,
            cur: None,
            written: 0,
            done: Vec::new(),
        };
        o.open_volume(1)?;
        Ok(o)
    }

    fn temp_of(&self, index: u32) -> PathBuf {
        let mut s = volume_name(&self.base, index).into_os_string();
        s.push(".tmp");
        PathBuf::from(s)
    }

    fn open_volume(&mut self, index: u32) -> io::Result<()> {
        let p = self.temp_of(index);
        let f = File::create(&p)?;
        self.cur = Some(f);
        self.vol = index;
        self.written = 0;
        self.done.push(p);
        Ok(())
    }

    /// Close the volume being written and start the next one.
    fn roll(&mut self) -> io::Result<()> {
        let next = self.vol + 1;
        if let Some(f) = self.cur.take() {
            let _ = f.sync_all();
        }
        self.open_volume(next)
    }

    /// Put the files in their final places and return the number of volumes
    /// written (1 when the archive is one file), or of files when it is an
    /// `.exe` plus volumes.
    pub fn finish(mut self) -> io::Result<u32> {
        if let Some(f) = self.cur.take() {
            let _ = f.sync_all();
        }
        let count = self.vol;
        // cloned, not taken: if a rename fails below, `drop` still removes the
        // temporary files that were not moved yet
        let temps = self.done.clone();
        // ---- a self-extracting archive: stub + first volume + trailer ----
        if let Some((stub, stub_len)) = self.sfx.clone() {
            let first = temps.first().cloned().unwrap_or_default();
            let len = fs::metadata(&first).map(|m| m.len()).unwrap_or(0);
            // the stub is read first: when an existing .exe is rewritten (the
            // window's delete), stub and base are the same file
            let mut head = Vec::new();
            let mut stub_file = File::open(&stub)?;
            Read::by_ref(&mut stub_file).take(stub_len).read_to_end(&mut head)?;
            drop(stub_file);
            let mut out = File::create(&self.base)?;
            out.write_all(&head)?;
            let mut src = File::open(&first)?;
            io::copy(&mut src, &mut out)?;
            out.write_all(&trailer_bytes(stub_len, len))?;
            out.sync_all()?;
            drop(out);
            let _ = fs::remove_file(&first);
            // the first volume is the .exe itself, so the others move down one
            for (k, tmp) in temps.iter().skip(1).enumerate() {
                put(tmp, &volume_name(&self.base, k as u32 + 2))?;
            }
        } else {
            for (k, tmp) in temps.iter().enumerate() {
                put(tmp, &volume_name(&self.base, k as u32 + 1))?;
            }
        }
        // ---- drop the volumes of an older, longer set --------------------
        let mut extra = count + 1;
        while extra < MAX_VOLUMES && volume_name(&self.base, extra).is_file() {
            let _ = fs::remove_file(volume_name(&self.base, extra));
            extra += 1;
        }
        self.done.clear();
        Ok(count)
    }
}

/// Move a finished temporary file onto its final name.
fn put(temp: &Path, target: &Path) -> io::Result<()> {
    if target.exists() {
        let _ = fs::remove_file(target);
    }
    fs::rename(temp, target)
}

impl Write for Out {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut left = buf;
        while !left.is_empty() {
            let room = match self.size {
                Some(s) => s.saturating_sub(self.written),
                None => u64::MAX,
            };
            if room == 0 {
                self.roll()?;
                continue;
            }
            let take = (left.len() as u64).min(room) as usize;
            match self.cur.as_mut() {
                Some(f) => f.write_all(&left[..take])?,
                None => return Err(io::Error::other("the archive file was closed too early")),
            }
            self.written += take as u64;
            left = &left[take..];
        }
        // A volume filled exactly to its size gets its successor at once: an
        // empty last volume then tells the reader that the set ends here,
        // instead of making a complete archive look like a missing volume.
        if let Some(s) = self.size {
            if self.written >= s {
                self.roll()?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.cur.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

impl Drop for Out {
    fn drop(&mut self) {
        // A finished Out has emptied its list; this is the cancel and error
        // path, where the temporary files must not stay beside the user's data.
        for p in &self.done {
            let _ = fs::remove_file(p);
        }
    }
}

// ---------------------------------------------------------------------------
// Reading: a set of volumes, or the archive inside an .exe
// ---------------------------------------------------------------------------

/// Reads an archive that may be spread over several files and may start at an
/// offset inside a self-extracting executable: it looks like one continuous
/// stream to the archive parser.
pub struct Chain {
    /// the .nik / .exe the reader was given
    main: PathBuf,
    /// where the archive starts in the first volume (0 unless it is an .exe)
    offset: u64,
    /// how many bytes of the archive the first volume holds (an .exe carries
    /// the trailer behind them, which is not part of the archive)
    head: u64,
    /// the volume being read (1 = the first)
    vol: u32,
    cur: Option<File>,
    /// bytes of the volume being read that are still to be read
    left: u64,
    /// bytes read from the volume being read
    in_vol: u64,
    /// payload length of the first volume, when the archive is a set
    /// (0 when it is one file)
    expect: u64,
    /// bytes of the whole archive, for the progress display
    total: u64,
    /// how many volumes were found when the chain was opened
    volumes: u32,
}

impl Chain {
    /// Open `path`, following `.n01`, `.n02` … when they are there, and
    /// starting at the archive's offset when `path` is a self-extracting .exe.
    pub fn open(path: &Path) -> io::Result<Chain> {
        let hidden = embedded(path);
        let offset = hidden.map(|e| e.offset).unwrap_or(0);
        let first_len = match hidden {
            Some(e) => e.len,
            None => fs::metadata(path)?.len().saturating_sub(offset),
        };
        let mut c = Chain {
            main: path.to_path_buf(),
            offset,
            head: first_len,
            vol: 1,
            cur: None,
            left: 0,
            in_vol: 0,
            expect: 0,
            total: first_len,
            volumes: 1,
        };
        c.open_volume(1)?;
        // a set of volumes reports its whole size, so the progress bar of the
        // window covers all of them
        let mut k = 2;
        while k < MAX_VOLUMES {
            match extra_volume(path, k) {
                Some(len) => {
                    c.total += len;
                    k += 1;
                }
                None => break,
            }
        }
        if k > 2 {
            c.volumes = k - 1;
            // every volume but the last is exactly this size: that is how the
            // reader knows whether another one should follow
            c.expect = first_len;
        }
        Ok(c)
    }

    /// The archive's header says it is a set: this is the size of a volume, so
    /// a full first volume must be followed by another one even when no `.n01`
    /// was found beside it (and the missing one can be named).
    pub fn set_volume_size(&mut self, size: u64) {
        if size > 0 {
            self.expect = size;
        }
    }

    /// How many volumes the archive is made of (1 when it is one file).
    pub fn volumes(&self) -> u32 {
        self.volumes
    }

    /// The bytes of the whole archive (all volumes together).
    pub fn total(&self) -> u64 {
        self.total
    }

    fn open_volume(&mut self, index: u32) -> io::Result<()> {
        let path = volume_name(&self.main, index);
        let mut f = File::open(&path).map_err(|e| {
            if index == 1 {
                e
            } else {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("The archive continues in '{}', which is missing.", path.display()),
                )
            }
        })?;
        if index == 1 {
            if self.offset > 0 {
                f.seek(SeekFrom::Start(self.offset))?;
            }
            // the payload measured when the chain was opened: for an .exe it
            // stops in front of the trailer, which is not archive data
            self.left = self.head;
        } else {
            self.left = f.metadata().map(|m| m.len()).unwrap_or(0);
        }
        self.cur = Some(f);
        self.vol = index;
        self.in_vol = 0;
        Ok(())
    }
}

impl Read for Chain {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.cur.is_none() {
                return Ok(0);
            }
            let want = (buf.len() as u64).min(self.left) as usize;
            if want == 0 {
                // this volume is finished; does another one follow?
                let full = self.expect > 0 && self.in_vol >= self.expect;
                let next = self.vol + 1;
                self.cur = None;
                if full && next < MAX_VOLUMES {
                    self.open_volume(next)?;
                    continue;
                }
                return Ok(0);
            }
            let n = match self.cur.as_mut() {
                Some(f) => f.read(&mut buf[..want])?,
                None => return Ok(0),
            };
            if n == 0 {
                // shorter than its size says: treat it as the end of the volume
                self.left = 0;
                continue;
            }
            self.left -= n as u64;
            self.in_vol += n as u64;
            PROGRESS.done.fetch_add(n as u64, Relaxed);
            return Ok(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // the parent's `use` lines are not re-exported by the glob above
    use std::fs;
    use std::io;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn names() {
        let p = Path::new("/tmp/photos.nik");
        // compared as paths: PathBuf and &Path are different types to assert_eq
        assert_eq!(volume_name(p, 1).as_path(), p);
        assert_eq!(volume_name(p, 2).as_path(), Path::new("/tmp/photos.n01"));
        assert_eq!(volume_name(p, 10).as_path(), Path::new("/tmp/photos.n09"));
        assert_eq!(volume_name(p, 11).as_path(), Path::new("/tmp/photos.n10"));
        assert_eq!(volume_name(p, 101).as_path(), Path::new("/tmp/photos.100"));
        let exe = if cfg!(windows) { "/tmp/photos.exe" } else { "/tmp/photos.run" };
        assert_eq!(sfx_name(p).as_path(), Path::new(exe));
    }

    /// A volume of a set opens the whole archive: the program is handed the
    /// first volume, so `photos.n01` dropped on unNik reads `photos.nik` too.
    #[test]
    fn a_volume_maps_to_its_first_volume() {
        let d = std::env::temp_dir().join(format!("nikpak-firstvol-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        // a real archive header, so the numeric-extension check believes it
        let mut header = crate::format::MAGIC.to_vec();
        header.extend_from_slice(&[3, 0, 0, 0]);
        let first = d.join("photos.nik");
        fs::write(&first, &header).unwrap();
        fs::write(d.join("photos.n01"), b"volume").unwrap();
        assert_eq!(first_volume(&d.join("photos.n01")), first);
        assert_eq!(first_volume(&d.join("photos.N07")), first, "any case");
        assert_eq!(first_volume(&d.join("photos.100")), first, "volume 100 and up too");
        // the first volume, a plain file and a .nik stay themselves
        assert_eq!(first_volume(&first), first);
        assert_eq!(first_volume(&d.join("notes.txt")), d.join("notes.txt"));
        assert_eq!(first_volume(&d.join("a.7z")), d.join("a.7z"));
        // a numeric extension whose `.nik` is not an archive (or is absent) is itself
        fs::write(d.join("backup.2024"), b"x").unwrap();
        assert_eq!(first_volume(&d.join("backup.2024")), d.join("backup.2024"));
        fs::write(d.join("report.nik"), b"not an archive").unwrap();
        assert_eq!(first_volume(&d.join("report.100")), d.join("report.100"));
        // a self-extracting set keeps its first volume in the .exe
        let exe = d.join(if cfg!(windows) { "disc.exe" } else { "disc.run" });
        fs::write(&exe, b"stub").unwrap();
        assert_eq!(first_volume(&d.join("disc.n02")), exe);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn sizes() {
        assert_eq!(parse_size("700m"), Some(700 * 1024 * 1024));
        assert_eq!(parse_size("700 MB"), Some(700 * 1024 * 1024));
        assert_eq!(parse_size("4.7g"), Some(5_046_586_573));
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("1024k"), Some(1024 * 1024));
        assert_eq!(parse_size("700mb"), Some(700 * 1024 * 1024));
        assert_eq!(parse_size("2 GiB"), Some(2 * 1024 * 1024 * 1024));
        assert_eq!(parse_size("4,7 GB"), Some(5_046_586_573));
        assert_eq!(parse_size("0"), None);
        assert_eq!(parse_size("-5m"), None);
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size(""), None);
        assert_eq!(size_text(700 * 1024 * 1024), "734 MB");
        assert_eq!(size_text(500), "500 B");
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("nikpak-split-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn volumes_round_trip() {
        let d = scratch("rt");
        let base = d.join("a.nik");
        let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
        let mut w = Out::create(&base, Some(MIN_VOLUME), None).unwrap();
        w.write_all(&data).unwrap();
        assert_eq!(w.finish().unwrap(), 5, "300000 bytes in 64 KiB volumes");
        assert!(is_split(&base));
        assert_eq!(volume_count(&base), 5);
        let mut back = Vec::new();
        Chain::open(&base).unwrap().read_to_end(&mut back).unwrap();
        assert_eq!(back, data);
        // a missing volume is reported as such
        fs::remove_file(volume_name(&base, 3)).unwrap();
        let mut c = Chain::open(&base).unwrap();
        let e = c.read_to_end(&mut Vec::new()).unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        assert!(e.to_string().contains("a.n02"), "{e}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn one_file_and_sfx() {
        let d = scratch("sfx");
        let base = d.join("b.nik");
        let mut w = Out::create(&base, None, None).unwrap();
        w.write_all(b"hello").unwrap();
        assert_eq!(w.finish().unwrap(), 1);
        assert!(!is_split(&base));
        assert_eq!(fs::read(&base).unwrap(), b"hello");
        assert!(embedded(&base).is_none());

        // an .exe: the stub in front, the trailer behind
        let stub = d.join("stub.bin");
        fs::write(&stub, b"STUBBYTES").unwrap();
        let exe = sfx_name(&base);
        let mut w = Out::create(&exe, None, Some((stub.clone(), 9))).unwrap();
        w.write_all(b"world").unwrap();
        w.finish().unwrap();
        let bytes = fs::read(&exe).unwrap();
        assert_eq!(&bytes[..9], b"STUBBYTES");
        assert_eq!(&bytes[9..14], b"world");
        assert_eq!(embedded(&exe), Some(Embedded { offset: 9, len: 5 }));
        let mut back = Vec::new();
        Chain::open(&exe).unwrap().read_to_end(&mut back).unwrap();
        assert_eq!(back, b"world");
        let _ = fs::remove_dir_all(&d);
    }
}
