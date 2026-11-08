//! Creating, listing, testing and extracting .nik archives.
//!
//! Inner stream (before compression/encryption), all integers little-endian:
//!
//!   record := type:u8
//!     REC_FILE : name_len:u16 name mtime:i64 mode:u32 size:u64 data[size] crc32:u32
//!     REC_DIR  : name_len:u16 name mtime:i64 mode:u32
//!     REC_END  : count:u64
//!
//! The REC_END marker lets us detect truncated archives even without
//! encryption.

use crate::cli::{Command, EpMode, Level, MediaMode, Options, Overwrite};
use crate::crypto::{self, DecryptReader, EncryptWriter};
use crate::engine::{self, BlockReader, BlockWriter, Engine};
use crate::error::{from_io, NikError, Result};
use crate::format::*;
use crate::media;
use crate::recomp;
use crate::split::{self, Chain, Out};
use crate::util::*;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};


/// println! that exits quietly when stdout is closed (e.g. `nikpak lb x | head`).
macro_rules! outln {
    () => { outln!("") };
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        if writeln!(std::io::stdout(), $($arg)*).is_err() {
            std::process::exit(0);
        }
    }};
}

const IO_BUF: usize = 256 * 1024;

// ===========================================================================
// Progress of `create` (read by the graphical interface)
// ===========================================================================

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering::Relaxed};

pub struct Progress {
    /// bytes of input already processed
    pub done: AtomicU64,
    /// total bytes of input
    pub total: AtomicU64,
    /// bytes already written to the archive (create); the window uses it to
    /// estimate the final size and ratio while compressing
    pub out: AtomicU64,
    /// PHASE_* below
    pub phase: AtomicU8,
    /// files already added (create)
    pub files_done: AtomicU64,
    /// files to add (create)
    pub files_total: AtomicU64,
    /// set by the GUI to abort; `create` then fails and removes the temp file
    pub cancel: AtomicBool,
}

pub static PROGRESS: Progress = Progress {
    done: AtomicU64::new(0),
    total: AtomicU64::new(0),
    out: AtomicU64::new(0),
    phase: AtomicU8::new(0),
    files_done: AtomicU64::new(0),
    files_total: AtomicU64::new(0),
    cancel: AtomicBool::new(false),
};
pub const PHASE_SCAN: u8 = 1;
pub const PHASE_COMPRESS: u8 = 2;
pub const PHASE_FINISH: u8 = 3;

/// Warnings of the last operation (the window shows them; the command line
/// prints them on stderr as before).
pub static WARNINGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn warn(msg: String) {
    eprintln!("WARNING: {msg}");
    if let Ok(mut w) = WARNINGS.lock() {
        if w.len() < 1000 {
            w.push(msg);
        }
    }
}

/// Counts the bytes written to the archive in PROGRESS.out, so the window can
/// estimate the final size and ratio while compressing. `Out` writes one file,
/// a set of volumes, or a self-extracting .exe (see split.rs).
struct CountedW(Out);

impl Write for CountedW {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.0.write(b)?;
        PROGRESS.out.fetch_add(n as u64, Relaxed);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl CountedW {
    fn into_out(self) -> Out {
        self.0
    }
}

fn check_cancel() -> Result<()> {
    if PROGRESS.cancel.load(Relaxed) {
        Err(NikError::Fatal("Cancelled.".into()))
    } else {
        Ok(())
    }
}

// ===========================================================================
// Writer-side plumbing
// ===========================================================================

enum Sink {
    Plain(BufWriter<CountedW>),
    Enc(EncryptWriter<BufWriter<CountedW>>),
}

impl Write for Sink {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Sink::Plain(w) => w.write(b),
            Sink::Enc(w) => w.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Sink::Plain(w) => w.flush(),
            Sink::Enc(w) => w.flush(),
        }
    }
}

impl Sink {
    /// Flush everything and hand the output, so the caller can put the file (or
    /// the volumes, or the .exe) in place with `Out::finish`.
    fn finish(self) -> io::Result<Out> {
        fn take(mut w: BufWriter<CountedW>) -> io::Result<Out> {
            w.flush()?;
            Ok(w.into_inner().map_err(|e| e.into_error())?.into_out())
        }
        match self {
            Sink::Plain(w) => take(w),
            Sink::Enc(w) => take(w.finish()?),
        }
    }
}

enum Compressor {
    Store(Sink),
    Zstd(zstd::stream::write::Encoder<'static, Sink>),
    Blocks(BlockWriter<Sink>),
}

impl Write for Compressor {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Compressor::Store(w) => w.write(b),
            Compressor::Zstd(w) => w.write(b),
            Compressor::Blocks(w) => w.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Compressor::Store(w) => w.flush(),
            Compressor::Zstd(w) => w.flush(),
            Compressor::Blocks(w) => w.flush(),
        }
    }
}

impl Compressor {
    fn finish(self) -> io::Result<Out> {
        match self {
            Compressor::Store(s) => s.finish(),
            Compressor::Zstd(e) => e.finish()?.finish(),
            Compressor::Blocks(b) => b.finish()?.finish(),
        }
    }
}

// ===========================================================================
// Collecting files to add
// ===========================================================================

struct Item {
    src: PathBuf,
    name: String,
    is_dir: bool,
    mtime: i64,
    mode: u32,
    /// file size seen while scanning (0 for folders)
    size: u64,
}

fn mtime_of(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn mode_of(md: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    md.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(md: &fs::Metadata) -> u32 {
    if md.is_dir() {
        0o755
    } else if md.permissions().readonly() {
        0o444
    } else {
        0o644
    }
}

struct Collector<'a> {
    items: Vec<Item>,
    warnings: u32,
    exclude: &'a [String],
    quiet: bool,
    archive_canon: Option<PathBuf>,
    /// file name of the archive, lower case: only paths with this name are canonicalized
    /// (canonicalize opens the file, far too slow to do for every one of a million files)
    archive_name: Option<String>,
}

impl<'a> Collector<'a> {
    fn warn(&mut self, msg: String) {
        self.warnings += 1;
        eprintln!("WARNING: {msg}");
    }

    fn excluded(&self, name: &str) -> bool {
        self.exclude.iter().any(|x| filter_matches(x, name))
    }

    fn add_path(&mut self, src: &Path, name: String, top: bool) {
        let md = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => {
                self.warn(format!("Cannot access '{}': {e}", src.display()));
                return;
            }
        };
        self.add_entry(src, name, top, md);
    }

    /// `md`: the entry's own metadata (not followed); for folder children it comes from the
    /// directory listing itself, which costs no extra system call on Windows.
    fn add_entry(&mut self, src: &Path, name: String, top: bool, md: fs::Metadata) {
        if md.file_type().is_symlink() {
            self.warn(format!("Skipping symbolic link '{}'", src.display()));
            return;
        }
        if let (Some(a), Some(an)) = (&self.archive_canon, &self.archive_name) {
            let same_name = src.file_name().map(|f| f.to_string_lossy().to_lowercase() == *an).unwrap_or(false);
            if same_name {
                if let Ok(c) = fs::canonicalize(src) {
                    if &c == a {
                        return; // never add the archive to itself
                    }
                }
            }
        }
        if !name.is_empty() {
            if name.len() > MAX_NAME_LEN {
                self.warn(format!("Name too long, skipped: '{}'", src.display()));
                return;
            }
            if self.excluded(&name) {
                return;
            }
        }
        if md.is_dir() {
            if !name.is_empty() {
                self.items.push(Item {
                    src: src.to_path_buf(),
                    name: name.clone(),
                    is_dir: true,
                    mtime: mtime_of(&md),
                    mode: mode_of(&md),
                    size: 0,
                });
            }
            if check_cancel().is_err() {
                return;
            }
            let rd = match fs::read_dir(src) {
                Ok(r) => r,
                Err(e) => {
                    self.warn(format!("Cannot read folder '{}': {e}", src.display()));
                    return;
                }
            };
            let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let fname = match e.file_name().into_string() {
                    Ok(s) => s,
                    Err(_) => {
                        self.warn(format!("Skipping non-Unicode file name in '{}'", src.display()));
                        continue;
                    }
                };
                let child_name = if name.is_empty() { fname } else { format!("{name}/{fname}") };
                match e.metadata() {
                    Ok(cmd) => self.add_entry(&e.path(), child_name, false, cmd),
                    Err(_) => self.add_path(&e.path(), child_name, false),
                }
            }
        } else if md.is_file() {
            let _ = top;
            self.items.push(Item {
                src: src.to_path_buf(),
                name,
                is_dir: false,
                mtime: mtime_of(&md),
                mode: mode_of(&md),
                size: md.len(),
            });
            // live count for the progress display while a big tree is scanned
            PROGRESS.files_total.fetch_add(1, Relaxed);
        } else {
            self.warn(format!("Skipping special file '{}'", src.display()));
        }
    }
}

fn base_name_for(arg: &Path, is_dir: bool, ep: EpMode) -> String {
    let comps = clean_name_components(arg);
    match ep {
        EpMode::Keep => comps.join("/"),
        EpMode::Names => comps.last().cloned().unwrap_or_default(),
        EpMode::BaseDir => {
            if is_dir {
                String::new()
            } else {
                comps.last().cloned().unwrap_or_default()
            }
        }
    }
}

fn expand_args(args: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let mut out = Vec::new();
    let mut warns = Vec::new();
    for a in args {
        let p = Path::new(a);
        if p.exists() || fs::symlink_metadata(p).is_ok() || !has_wildcard(a) {
            out.push(p.to_path_buf());
            continue;
        }
        match glob::glob(a) {
            Ok(paths) => {
                let mut n = 0;
                for g in paths.flatten() {
                    out.push(g);
                    n += 1;
                }
                if n == 0 {
                    warns.push(format!("No files match '{a}'"));
                }
            }
            Err(_) => warns.push(format!("Invalid wildcard pattern '{a}'")),
        }
    }
    (out, warns)
}

// ===========================================================================
// CREATE
// ===========================================================================

pub fn create(o: &Options) -> Result<u8> {
    let started = Instant::now();
    PROGRESS.done.store(0, Relaxed);
    PROGRESS.total.store(0, Relaxed);
    PROGRESS.out.store(0, Relaxed);
    PROGRESS.phase.store(PHASE_SCAN, Relaxed);
    PROGRESS.files_done.store(0, Relaxed);
    PROGRESS.files_total.store(0, Relaxed);
    let archive = PathBuf::from(&o.archive);

    // ---- a self-extracting archive is an .exe, not a .nik ---------------
    // The archive is written inside a copy of unNik, so whoever gets the file
    // can unpack it without installing anything.
    let (archive, sfx) = if o.sfx {
        let stub = split::stub().ok_or_else(|| {
            NikError::Fatal(
                "Cannot make a self-extracting archive: unNik is not next to nikpak.\n\
                 The .exe is unNik with the archive appended, so the two must sit together."
                    .into(),
            )
        })?;
        let len = fs::metadata(&stub).map(|m| m.len()).unwrap_or(0);
        (split::sfx_name(&archive), Some((stub, len)))
    } else {
        (archive, None)
    };
    let archive_display = archive.display().to_string();

    if let Some(parent) = archive.parent() {
        if !parent.as_os_str().is_empty() && !parent.is_dir() {
            return Err(NikError::Fatal(format!(
                "Cannot create '{}': the folder '{}' does not exist.",
                archive_display,
                parent.display()
            )));
        }
    }
    if archive.is_dir() {
        return Err(NikError::Fatal(format!("'{}' is a folder, not an archive file.", archive_display)));
    }
    if archive.exists() {
        let allow = match o.overwrite {
            Overwrite::All => true,
            Overwrite::Skip => false,
            Overwrite::Rename => false,
            Overwrite::Ask => matches!(
                ask_yes_no(&format!("Archive '{}' already exists. Overwrite it?", archive_display)),
                Some('y') | Some('a')
            ),
        };
        if !allow {
            return Err(NikError::Fatal(format!(
                "Archive '{}' already exists. Adding to an existing archive is not supported;\n\
                 use -o+ to overwrite it, or choose another name.",
                archive_display
            )));
        }
    }

    // ---- collect --------------------------------------------------------
    let archive_canon = archive
        .parent()
        .map(|p| if p.as_os_str().is_empty() { Path::new(".") } else { p })
        .and_then(|p| fs::canonicalize(p).ok())
        .and_then(|p| archive.file_name().map(|f| p.join(f)));
    let archive_name = archive.file_name().map(|f| f.to_string_lossy().to_lowercase());
    let mut col = Collector {
        items: Vec::new(),
        warnings: 0,
        exclude: &o.excludes,
        quiet: o.quiet,
        archive_canon,
        archive_name,
    };
    let (paths, warns) = expand_args(&o.files);
    for w in warns {
        col.warn(w);
    }
    for p in &paths {
        let md = match fs::symlink_metadata(p) {
            Ok(m) => m,
            Err(_) => {
                col.warn(format!("Cannot find '{}'", p.display()));
                continue;
            }
        };
        let name = base_name_for(p, md.is_dir(), o.ep);
        if name.is_empty() && !md.is_dir() {
            col.warn(format!("Cannot determine a name for '{}'", p.display()));
            continue;
        }
        col.add_path(p, name, true);
    }
    let _ = col.quiet;
    let warnings_collect = col.warnings;
    let items = col.items;
    check_cancel()?;
    if items.is_empty() {
        return Err(NikError::NoFiles("No files to add: nothing matched the given names.".into()));
    }

    PROGRESS.total.store(items.iter().filter(|it| !it.is_dir).map(|it| it.size).sum(), Relaxed);
    PROGRESS.files_total.store(items.iter().filter(|it| !it.is_dir).count() as u64, Relaxed);

    // ---- password -------------------------------------------------------
    let password = password_for_create(&o.password)?;

    // ---- header ---------------------------------------------------------
    let method = match o.level {
        Level::Store => METHOD_STORE,
        Level::Zstd(_) => METHOD_ZSTD,
        Level::Max => METHOD_BLOCKS,
    };
    let header = Header {
        encrypted: password.is_some(),
        method,
        kdf_mem: KDF_MEM_KIB,
        kdf_time: KDF_TIME,
        kdf_lanes: KDF_LANES,
        salt: crypto::random_bytes::<SALT_LEN>(),
        nonce_prefix: crypto::random_bytes::<NONCE_PREFIX_LEN>(),
        legal: checked_legal(o.legal.as_deref())?,
        cover: checked_cover(o.cover.as_deref())?,
        // the volumes, when the archive is split (see split.rs)
        split: o.split.map(|s| s.max(split::MIN_VOLUME)),
    };
    let header_bytes = header.to_bytes();

    // ---- open the output (a temp file, or the first temp volume) ---------
    let out = Out::create(&archive, o.split, sfx.clone())
        .map_err(|e| NikError::Fatal(format!("Cannot create '{}': {e}", archive_display)))?;
    let mut bw = BufWriter::with_capacity(IO_BUF, CountedW(out));
    bw.write_all(&header_bytes).map_err(|e| from_io(e, "Write error"))?;

    let sink = if let Some(pw) = &password {
        if !o.quiet {
            outln!("Deriving encryption key (Argon2id, 64 MiB)...");
        }
        let key = crypto::derive_key(pw, &header.salt, header.kdf_mem, header.kdf_time, header.kdf_lanes)
            .map_err(NikError::Fatal)?;
        Sink::Enc(EncryptWriter::new(bw, &key, &header.nonce_prefix, header_bytes.clone()))
    } else {
        Sink::Plain(bw)
    };

    let threads = if o.threads > 0 {
        o.threads
    } else {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1)
    };
    let parallel = parallel_wanted(threads);
    let filters_on = o.media != MediaMode::Off && o.level != Level::Store;
    // Deflate recompression at -m4 and -m5: at lower levels the fast zstd
    // settings gain little on the expanded data and the analysis costs time.
    let recomp_on = filters_on && o.level.effort() >= 15;

    PROGRESS.files_done.store(0, Relaxed);
    PROGRESS.phase.store(PHASE_COMPRESS, Relaxed);

    // ---- write entries ----------------------------------------------------
    let mut warnings = warnings_collect;
    let stats: Stats;
    let out: Out;
    if o.level == Level::Max {
        // -m5: blocks of LZMA2 / PPMd, compressed one per thread at the same
        // time (the reader has always accepted any number of blocks)
        let blocks = plan_blocks(&items, o.quiet, recomp_on, threads)?;
        let (back, st) = write_body_blocks(sink, &blocks, &items, o, filters_on, recomp_on, parallel, &archive)?;
        warnings += st.warnings;
        stats = st;
        PROGRESS.phase.store(PHASE_FINISH, Relaxed);
        out = back.finish().map_err(|e| from_io(e, "Cannot finish archive"))?;
    } else {
        let mut comp = match o.level {
            Level::Zstd(lvl) => {
                let mut enc = zstd::stream::write::Encoder::new(sink, lvl)
                    .map_err(|e| from_io(e, "Cannot start compressor"))?;
                if threads > 1 {
                    enc.multithread(threads as u32).map_err(|e| from_io(e, "Cannot enable multithreading"))?;
                }
                let _ = enc.include_checksum(true);
                // High levels: 128 MiB window + long-distance matching finds repeats
                // that are far away (large source trees, many similar documents).
                // 2^27 is within the decoder's default window limit, so every
                // zstd-capable reader can still decode it.
                if lvl >= 15 {
                    let _ = enc.window_log(27);
                    let _ = enc.long_distance_matching(true);
                }
                Compressor::Zstd(enc)
            }
            _ => Compressor::Store(sink),
        };
        let all: Vec<usize> = (0..items.len()).collect();
        let mut st = Stats::default();
        if parallel {
            write_items_pipelined(&mut comp, &items, &all, o, filters_on, recomp_on, threads, &archive, &mut st)?;
        } else {
            write_items_list(&mut comp, &items, &all, o, filters_on, recomp_on, &mut st, true)?;
        }
        warnings += st.warnings;
        stats = st;
        PROGRESS.phase.store(PHASE_FINISH, Relaxed);
        comp.write_all(&[REC_END]).map_err(|e| from_io(e, "Write error"))?;
        comp.write_all(&stats.count.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
        out = comp.finish().map_err(|e| from_io(e, "Cannot finish archive"))?;
    }
    let count = stats.count;
    let total_in = stats.total_in;
    let media_files = stats.media;
    let deflate_files = stats.deflate;
    let volumes = out
        .finish()
        .map_err(|e| NikError::Fatal(format!("Cannot create '{}': {e}", archive_display)))?;

    // one file, or a set of volumes (with the .exe holding the first of them)
    let files = volumes;
    let out_size = fs::metadata(&archive).map(|m| m.len()).unwrap_or(0);
    if !o.quiet {
        let secs = started.elapsed().as_secs_f64();
        let ratio = if total_in > 0 { out_size as f64 * 100.0 / total_in as f64 } else { 100.0 };
        outln!();
        outln!(
            "Done. {} entries, {} -> {} ({:.1}%), {:.2}s{}",
            count,
            human_size(total_in),
            human_size(out_size),
            ratio,
            secs,
            if header.encrypted { ", encrypted" } else { "" }
        );
        if files > 1 {
            outln!("The archive is split into {} files of {}.", files, split::size_text(o.split.unwrap_or(0)));
        }
        if sfx.is_some() {
            outln!(
                "Self-extracting archive: '{}' unpacks itself on any Windows PC (no nikpak needed).",
                archive_display
            );
        }
        if media_files > 0 {
            outln!("{} media file(s) were compressed with media filters.", media_files);
        }
        if deflate_files > 0 {
            outln!("{} office/PDF/gzip file(s) had their Deflate streams recompressed.", deflate_files);
        }
    }
    Ok(if warnings > 0 { 1 } else { 0 })
}

/// Write the -m5 body: every block, in order, then a last small block that
/// holds the end record, then the body's end marker. The blocks are made at
/// the same time on several threads (`parallel`), each into its own holding
/// area (memory, then a temporary file), and only copied into the archive in
/// this order - so the archive itself is still written strictly sequentially,
/// which is what encryption and the volume splitter need.
fn write_body_blocks(
    sink: Sink,
    blocks: &[BlockPlan],
    items: &[Item],
    o: &Options,
    filters_on: bool,
    recomp_on: bool,
    parallel: bool,
    archive: &Path,
) -> Result<(Sink, Stats)> {
    let mut sink = sink;
    let mut stats = Stats::default();
    if parallel && blocks.len() > 1 {
        // each block may keep this much in memory before spilling to a file
        let per_block = (BLOCK_SPILL_TOTAL / blocks.len() as u64).clamp(8 << 20, 64 << 20);
        let results: Vec<(Result<Stats>, Spill)> = std::thread::scope(|s| {
            let handles: Vec<_> = blocks
                .iter()
                .enumerate()
                .map(|(k, b)| {
                    let path = spill_path(archive, k as u64);
                    s.spawn(move || {
                        let mut sp = Spill::new(path, per_block);
                        let mut st = Stats::default();
                        let r = (|| -> Result<()> {
                            let mut bw = BlockWriter::new(&mut sp);
                            // one block is one thread: the engine is LZMA2 or PPMd,
                            // which are single-threaded by nature
                            bw.start(b.engine, 1).map_err(|e| from_io(e, "Cannot start compressor"))?;
                            write_items_list(&mut bw, items, &b.items, o, filters_on, recomp_on, &mut st, false)?;
                            // `end` closes this block; the body's end marker is
                            // written once, after the last block, by the caller
                            bw.end().map_err(|e| from_io(e, "Cannot finish compressor"))?;
                            Ok(())
                        })();
                        (r.map(|()| st), sp)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join().unwrap_or_else(|_| {
                        (Err(NikError::Fatal("Internal error while compressing.".into())), Spill::new(PathBuf::new(), 0))
                    })
                })
                .collect()
        });
        for (r, sp) in results {
            let st = r?;
            sp.finish().map_err(|e| from_io(e, "Write error"))?.copy_to(&mut sink)?;
            for line in &st.logs {
                outln!("{}", line);
            }
            stats.merge(&st);
        }
    } else {
        let mut bw = BlockWriter::new(&mut sink);
        for b in blocks {
            check_cancel()?;
            bw.start(b.engine, 1).map_err(|e| from_io(e, "Cannot start compressor"))?;
            write_items_list(&mut bw, items, &b.items, o, filters_on, recomp_on, &mut stats, true)?;
            bw.end().map_err(|e| from_io(e, "Cannot finish compressor"))?;
        }
    }
    // ---- the end record, in a small block of its own ---------------------
    // (it must live in *some* block: the record stream is what blocks carry)
    check_cancel()?;
    let mut tail = BlockWriter::new(&mut sink);
    tail.start(Engine::lzma2_for(1 << 20, false), 1)
        .map_err(|e| from_io(e, "Cannot start compressor"))?;
    tail.write_all(&[REC_END]).map_err(|e| from_io(e, "Write error"))?;
    tail.write_all(&stats.count.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
    // `finish` writes the body's end marker and hands the archive writer back;
    // the match (not `let _ =`) makes sure the borrow of `sink` is over here
    match tail.finish() {
        Ok(_) => {}
        Err(e) => return Err(from_io(e, "Cannot finish archive")),
    }
    Ok((sink, stats))
}

/// Decode a Deflate-recompressed transform again and compare it with the
/// original (length + CRC32) before trusting it with the user's data.
fn recomp_verified(t: &[u8], size: u64, crc: u32) -> bool {
    let mut cw = media::CrcWrite::new(io::sink());
    recomp::decode(&mut io::Cursor::new(t), size, &mut cw).is_ok() && cw.finish() == (crc, size)
}

/// Is this file text (source code, documents, CSV ...) rather than binary?
fn looks_like_text(b: &[u8]) -> bool {
    if b.is_empty() {
        return false;
    }
    let mut ctrl = 0usize;
    let mut high = 0usize;
    for &c in b {
        match c {
            0 => return false,
            b'\t' | b'\n' | b'\r' | 0x0c => {}
            1..=0x1f | 0x7f => ctrl += 1,
            0x80..=0xff => high += 1,
            _ => {}
        }
    }
    ctrl * 100 <= b.len() && high * 100 <= b.len() * 30
}

/// One block of the -m5 body: which entries go in it, and the engine it is
/// compressed with. Blocks hold whole entries only, and are independent.
struct BlockPlan {
    items: Vec<usize>,
    engine: Engine,
}

/// Smallest block worth making: below this a block's own framing and the lost
/// cross-entry matches cost more than the extra thread wins.
const MIN_BLOCK_BYTES: u64 = 8 << 20;

/// Split a (sorted) list of items into at most `k` chunks of roughly equal
/// *bytes*, keeping the list's order so neighbouring files stay together
/// (which is what makes the block compress well). Entries are never split.
fn split_by_bytes(list: &[usize], sizes: &[u64], k: usize) -> Vec<Vec<usize>> {
    if list.is_empty() {
        return Vec::new();
    }
    if k <= 1 {
        return vec![list.to_vec()];
    }
    let total: u64 = list.iter().map(|&i| sizes[i]).sum();
    let target = (total / k as u64).max(1);
    let mut out: Vec<Vec<usize>> = Vec::with_capacity(k);
    let mut cur: Vec<usize> = Vec::new();
    let mut acc = 0u64;
    for (pos, &i) in list.iter().enumerate() {
        cur.push(i);
        acc += sizes[i];
        let chunks_left = k - out.len();
        let items_left = list.len() - (pos + 1);
        // close this chunk once it has its share - but always leave at least
        // one item for each chunk still to come, so none comes out empty
        if acc >= target && chunks_left > 1 && items_left >= chunks_left - 1 {
            out.push(std::mem::take(&mut cur));
            acc = 0;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// How many blocks a group of `bytes` deserves: one per thread, but never a
/// block smaller than MIN_BLOCK_BYTES (a 200 MB archive on 16 threads makes 8
/// blocks of 25 MB; a 20 MB archive makes 2), never more chunks than items, and
/// at most MAX_BLOCKS - every encoder holds a dictionary in RAM, and past eight
/// at once the memory cost outweighs the cores.
fn block_count(bytes: u64, items: usize, threads: usize) -> usize {
    let by_size = (bytes / MIN_BLOCK_BYTES).max(1) as usize;
    by_size.min(threads.clamp(1, MAX_BLOCKS)).min(items.max(1))
}

/// Most blocks compressed at the same time.
const MAX_BLOCKS: usize = 8;
/// What all the block encoders together may take: LZMA2 needs about 11 times
/// its dictionary, PPMd about its model size, so the per-block size follows
/// from how many blocks run at once.
const ENCODER_RAM_BUDGET: u64 = 1536 << 20;

/// Largest dictionary one LZMA2 block may use when `blocks` run together.
fn lzma_dict_cap(blocks: usize) -> u32 {
    (ENCODER_RAM_BUDGET / (11 * blocks.max(1) as u64)).clamp(1 << 20, 128 << 20) as u32
}

/// An LZMA2 engine for one block, its dictionary sized to the block but never
/// past the share the memory budget allows.
fn lzma_block(bytes: u64, text: bool, blocks: usize) -> Engine {
    match Engine::lzma2_for(bytes.max(1), text) {
        Engine::Lzma2 { dict, lc, lp, pb } => Engine::Lzma2 { dict: dict.min(lzma_dict_cap(blocks)), lc, lp, pb },
        e => e,
    }
}

/// A PPMd engine for one block, its model sized to the block but never past
/// the share the memory budget allows.
fn ppmd_block(bytes: u64, blocks: usize) -> Engine {
    let cap = (ENCODER_RAM_BUDGET / blocks.max(1) as u64).clamp(1 << 20, engine::PPMD_MEM as u64) as u32;
    match Engine::ppmd_for(bytes.max(1)) {
        Engine::Ppmd { order, mem } => Engine::Ppmd { order, mem: mem.min(cap) },
        e => e,
    }
}

/// -m5: cut the entries into blocks - binary data (and folders) compressed
/// with LZMA2, text with PPMd or LZMA2, chosen by a trial on a sample - and
/// make **several blocks of each kind**, so the blocks can be compressed at
/// the same time on different threads. (LZMA2 and PPMd are single-threaded;
/// the format has always allowed any number of blocks - the reader walks them
/// until the end marker - so this needs no change to the archive layout.)
fn plan_blocks(items: &[Item], quiet: bool, recomp_on: bool, threads: usize) -> Result<Vec<BlockPlan>> {
    // the first 16 KiB tell text from binary; reading them for a million small files is the
    // slow part, so the files are looked at by several threads, with progress and cancel
    const HEAD: usize = 16 * 1024;
    // per item: (is_text, size counted for the engine choice)
    let classify = |it: &Item, head: &mut [u8]| -> (bool, u64) {
        if it.is_dir {
            return (false, 0);
        }
        let Ok(mut f) = File::open(&it.src) else { return (false, 0) };
        let mut size = f.metadata().map(|m| m.len()).unwrap_or(it.size);
        let n = Read::by_ref(&mut f).take(HEAD as u64).read(head).unwrap_or(0);
        let h = &head[..n];
        let container = recomp::detect_bytes(h).is_some();
        if container && recomp_on {
            // Deflate streams expand to roughly 4-8x their size
            size = size.saturating_mul(6);
        }
        let is_text = !container && looks_like_text(h) && media::detect(&mut f, size).is_none();
        (is_text, size)
    };
    PROGRESS.files_done.store(0, Relaxed);
    let n_threads = threads.clamp(1, 16).min(items.len().max(1));
    let chunk = items.len().div_ceil(n_threads).max(1);
    let mut kinds: Vec<(bool, u64)> = vec![(false, 0); items.len()];
    std::thread::scope(|sc| {
        for (part, out) in items.chunks(chunk).zip(kinds.chunks_mut(chunk)) {
            sc.spawn(move || {
                let mut head = vec![0u8; HEAD];
                for (it, k) in part.iter().zip(out.iter_mut()) {
                    if PROGRESS.cancel.load(Relaxed) {
                        return;
                    }
                    *k = classify(it, &mut head);
                    if !it.is_dir {
                        PROGRESS.files_done.fetch_add(1, Relaxed);
                    }
                }
            });
        }
    });
    check_cancel()?;
    let mut bin = Vec::new();
    let mut txt = Vec::new();
    let (mut bin_size, mut txt_size) = (0u64, 0u64);
    for (i, &(is_text, size)) in kinds.iter().enumerate() {
        if is_text {
            txt.push(i);
            txt_size += size;
        } else {
            bin.push(i);
            bin_size += size;
        }
    }
    // Like 7-Zip: order files by extension, then by file name, so that similar
    // and identical files (e.g. the same header in several versions of a
    // library) are next to each other in the solid stream. Folders first.
    let key = |i: &usize| {
        let it = &items[*i];
        let file = it.name.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        let ext = match file.rfind('.') {
            Some(p) if p > 0 => file[p + 1..].to_string(),
            _ => String::new(),
        };
        (!it.is_dir, ext, file, it.name.clone())
    };
    bin.sort_by_cached_key(key);
    txt.sort_by_cached_key(key);

    // the size each entry takes in its block (Deflate streams already counted
    // as the expanded size they become)
    let sizes: Vec<u64> = kinds.iter().map(|&(_, s)| s).collect();
    let mut plan: Vec<BlockPlan> = Vec::new();
    if !bin.is_empty() {
        let k = block_count(bin_size, bin.len(), threads);
        for part in split_by_bytes(&bin, &sizes, k) {
            let bytes: u64 = part.iter().map(|&i| sizes[i]).sum();
            let engine = lzma_block(bytes, false, k);
            if !quiet {
                outln!("Binary block: {} entries, {}", part.len(), engine.name());
            }
            plan.push(BlockPlan { items: part, engine });
        }
    }
    if !txt.is_empty() {
        // Trial on four contiguous 1 MiB runs of the (sorted) text stream, so
        // the sample contains neighbouring files exactly as the blocks will.
        let mut sample = Vec::with_capacity(4 << 20);
        for k in 0..4 {
            let start = txt.len() * k / 4;
            let target = sample.len() + (1 << 20);
            for &i in &txt[start..] {
                if sample.len() >= target {
                    break;
                }
                if let Ok(f) = File::open(&items[i].src) {
                    let _ = f.take((target - sample.len()) as u64).read_to_end(&mut sample);
                }
            }
        }
        let k = block_count(txt_size, txt.len(), threads);
        // one engine for all the text blocks, sized to one block's share
        let per_block = (txt_size / k as u64).max(1);
        let ppmd = ppmd_block(per_block, k);
        let lzma = lzma_block(per_block, true, k);
        let engine = match (engine::trial_size(&sample, ppmd), engine::trial_size(&sample, lzma)) {
            (Ok(p), Ok(l)) if l < p => lzma,
            _ => ppmd,
        };
        for part in split_by_bytes(&txt, &sizes, k) {
            if !quiet {
                outln!("Text block:   {} files, {}", part.len(), engine.name());
            }
            plan.push(BlockPlan { items: part, engine });
        }
    }
    Ok(plan)
}

// ===========================================================================
// Writing entries: the per-entry logic, and the ways of running it
// ===========================================================================
//
// The work of one entry (open, media filter, Deflate recompression, copy,
// CRC) is done by `write_item_records`, whatever writes the result:
//
// * the archive itself, one entry after the other (threads = 1, NIKPAK_SEQ, or
//   an archive that is small enough not to bother);
// * a block for -m5, on its own thread, several blocks at a time;
// * a temporary holding area (`Spill`), so the expensive per-entry work can
//   run ahead on several threads while the archive is written strictly in
//   order.
//
// Keeping one copy of that logic is deliberate: it decides how a user's file
// is stored, and two versions drifting apart would mean archives that read
// differently depending on the level.

/// What one entry cost, and the line to print for it.
#[derive(Default)]
struct ItemDone {
    bytes: u64,
    media: bool,
    deflate: bool,
    /// could not be opened or read: a warning was printed, nothing was written
    skipped: bool,
    /// the "Adding …" line, printed by whoever owns the console, in order
    log: String,
}

/// Tallies for the closing "Done." line.
#[derive(Default)]
struct Stats {
    count: u64,
    total_in: u64,
    media: u64,
    deflate: u64,
    /// u32: it is added to the collector's count, which is a u32
    warnings: u32,
    /// "Adding …" lines collected by a worker thread, printed by the thread
    /// that owns the console when the piece reaches the archive
    logs: Vec<String>,
}

impl Stats {
    fn add(&mut self, d: &ItemDone) {
        if d.skipped {
            self.warnings += 1;
            return;
        }
        self.count += 1;
        self.total_in += d.bytes;
        if d.media {
            self.media += 1;
        }
        if d.deflate {
            self.deflate += 1;
        }
    }

    fn merge(&mut self, other: &Stats) {
        self.count += other.count;
        self.total_in += other.total_in;
        self.media += other.media;
        self.deflate += other.deflate;
        self.warnings += other.warnings;
        // `logs` is not merged: the block loop prints a block's lines as it
        // copies it, and then drops them
    }
}

/// Write one entry (a folder record, or a file's header, data and CRC) and
/// return what it cost. `w` may be the archive, a block, or a holding area.
fn write_item_records<W: Write>(
    w: &mut W,
    it: &Item,
    o: &Options,
    filters_on: bool,
    recomp_on: bool,
    buf: &mut [u8],
) -> Result<ItemDone> {
    let name_bytes = it.name.as_bytes();
    let mut done = ItemDone::default();
    if it.is_dir {
        write_rec_header(w, REC_DIR, name_bytes, it.mtime, it.mode, None)
            .map_err(|e| from_io(e, "Write error"))?;
        if !o.quiet {
            done.log = format!("Adding    {}/", it.name);
        }
        return Ok(done);
    }
    let mut f = match File::open(&it.src) {
        Ok(f) => f,
        Err(e) => {
            warn(format!("Cannot open '{}': {e} - skipped", it.src.display()));
            done.skipped = true;
            return Ok(done);
        }
    };
    let size = match f.metadata() {
        Ok(m) => m.len(),
        Err(e) => {
            warn(format!("Cannot read size of '{}': {e} - skipped", it.src.display()));
            done.skipped = true;
            return Ok(done);
        }
    };
    done.bytes = size;
    // ---- media filters (WAV / AIFF / MOD / BMP) ----
    if filters_on {
        let zl = o.level.effort();
        if let Some(det) = media::detect(&mut f, size) {
            let nl = media::nac_level(zl);
            let method = if o.media == MediaMode::Force {
                Some(det.default_method())
            } else {
                match media::choose(&mut f, &det, nl) {
                    Ok(m) => m,
                    Err(_) => None,
                }
            };
            let _ = f.seek(io::SeekFrom::Start(0));
            if let Some(m) = method {
                let params = det.params_bytes();
                write_rec_header(w, REC_MEDIA, name_bytes, it.mtime, it.mode, Some(size))
                    .map_err(|e| from_io(e, "Write error"))?;
                w.write_all(&[det.filter_id(), m, params.len() as u8])
                    .and_then(|_| w.write_all(&params))
                    .map_err(|e| from_io(e, "Write error"))?;
                let mut cw = media::ChunkWriter::new(&mut *w);
                let mut rd = media::CrcRead::new(&mut f, size);
                media::encode(&mut rd, size, &det, m, nl, &mut cw)
                    .map_err(|e| from_io(e, &format!("'{}'", it.src.display())))?;
                cw.finish().map_err(|e| from_io(e, "Write error"))?;
                let (crc, left) = rd.finish();
                if left != 0 {
                    return Err(NikError::Fatal(format!(
                        "'{}' changed while it was being archived; aborting (archive not created).",
                        it.src.display()
                    )));
                }
                w.write_all(&crc.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
                done.media = true;
                PROGRESS.done.fetch_add(size, Relaxed);
                PROGRESS.files_done.fetch_add(1, Relaxed);
                if !o.quiet {
                    done.log = format!(
                        "Adding    {}   [{} {}]",
                        it.name,
                        det.kind.name(),
                        media::method_name(det.filter_id(), m)
                    );
                }
                return Ok(done);
            }
        }
    }
    // ---- Deflate recompression (ZIP containers, PDF, gzip) ----
    if recomp_on {
        if let Some(kind) = recomp::detect(&mut f, size) {
            let mut data = Vec::with_capacity(size as usize);
            if let Err(e) = Read::by_ref(&mut f).take(size).read_to_end(&mut data) {
                return Err(NikError::Fatal(format!("Read error on '{}': {e}", it.src.display())));
            }
            if data.len() as u64 != size {
                return Err(NikError::Fatal(format!(
                    "'{}' became shorter while it was being archived; aborting (archive not created).",
                    it.src.display()
                )));
            }
            let crc = crc32fast::hash(&data);
            let mut label = String::new();
            match recomp::encode(&data, kind).filter(|(t, _)| recomp_verified(t, size, crc)) {
                Some((t, st)) => {
                    write_rec_header(w, REC_MEDIA, name_bytes, it.mtime, it.mode, Some(size))
                        .and_then(|_| w.write_all(&[recomp::FILTER_DEFLATE, recomp::M_PREFLATE, 0]))
                        .map_err(|e| from_io(e, "Write error"))?;
                    let mut cw = media::ChunkWriter::new(&mut *w);
                    cw.write_all(&t).and_then(|_| cw.finish()).map_err(|e| from_io(e, "Write error"))?;
                    done.deflate = true;
                    label = format!(
                        "   [{} {} stream{} recompressed]",
                        kind.name(),
                        st.streams,
                        if st.streams == 1 { "" } else { "s" }
                    );
                }
                None => {
                    write_rec_header(w, REC_FILE, name_bytes, it.mtime, it.mode, Some(size))
                        .and_then(|_| w.write_all(&data))
                        .map_err(|e| from_io(e, "Write error"))?;
                }
            }
            w.write_all(&crc.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
            PROGRESS.done.fetch_add(size, Relaxed);
            PROGRESS.files_done.fetch_add(1, Relaxed);
            if !o.quiet {
                done.log = format!("Adding    {}{}", it.name, label);
            }
            return Ok(done);
        }
    }
    write_rec_header(w, REC_FILE, name_bytes, it.mtime, it.mode, Some(size))
        .map_err(|e| from_io(e, "Write error"))?;
    let mut crc = crc32fast::Hasher::new();
    let mut remaining = size;
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = f
            .read(&mut buf[..want])
            .map_err(|e| NikError::Fatal(format!("Read error on '{}': {e}", it.src.display())))?;
        if n == 0 {
            return Err(NikError::Fatal(format!(
                "'{}' became shorter while it was being archived; aborting (archive not created).",
                it.src.display()
            )));
        }
        crc.update(&buf[..n]);
        w.write_all(&buf[..n]).map_err(|e| from_io(e, "Write error"))?;
        remaining -= n as u64;
        PROGRESS.done.fetch_add(n as u64, Relaxed);
        check_cancel()?;
    }
    w.write_all(&crc.finalize().to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
    PROGRESS.files_done.fetch_add(1, Relaxed);
    if !o.quiet {
        done.log = format!("Adding    {}", it.name);
    }
    Ok(done)
}

fn write_rec_header<W: Write>(
    w: &mut W,
    kind: u8,
    name: &[u8],
    mtime: i64,
    mode: u32,
    size: Option<u64>,
) -> io::Result<()> {
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
// A holding area: bytes in memory up to a limit, then a temporary file
// ---------------------------------------------------------------------------

/// Where several threads put finished work while the archive is written in
/// order: a block's compressed bytes at -m5, or one entry's records for the
/// other levels. Small results stay in memory; big ones spill into a
/// temporary file beside the archive, so memory stays bounded however large
/// the archive is. The file is removed when the `Prepared` is dropped or
/// copied into the archive.
struct Spill {
    mem: Vec<u8>,
    /// bytes held in all (memory + file)
    len: u64,
    /// spill into a file once `limit` bytes are reached
    limit: u64,
    path: PathBuf,
    file: Option<BufWriter<File>>,
    /// the temporary file exists and has to be removed on drop
    armed: bool,
}

impl Spill {
    fn new(path: PathBuf, limit: u64) -> Spill {
        Spill { mem: Vec::new(), len: 0, limit, path, file: None, armed: true }
    }

    fn finish(mut self) -> io::Result<Prepared> {
        match self.file.take() {
            Some(mut f) => {
                // flush first: if that fails the file must still be removed by
                // the drop of `self`, so `armed` stays set until it worked
                f.flush()?;
                drop(f);
                self.armed = false;
                Ok(Prepared::File { path: self.path.clone() })
            }
            None => {
                self.armed = false;
                Ok(Prepared::Mem(std::mem::take(&mut self.mem)))
            }
        }
    }
}

impl Write for Spill {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if self.file.is_none() && self.len + b.len() as u64 > self.limit {
            // over the limit: move to a file and stream the rest into it
            let f = File::create(&self.path)
                .map_err(|e| io::Error::new(e.kind(), format!("Cannot write '{}': {e}", self.path.display())))?;
            let mut bw = BufWriter::with_capacity(IO_BUF, f);
            bw.write_all(&self.mem)?;
            self.mem = Vec::new();
            self.mem.shrink_to_fit();
            self.file = Some(bw);
        }
        match self.file.as_mut() {
            Some(f) => f.write_all(b)?,
            None => self.mem.extend_from_slice(b),
        }
        self.len += b.len() as u64;
        Ok(b.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(f) => f.flush(),
            None => Ok(()),
        }
    }
}

impl Drop for Spill {
    fn drop(&mut self) {
        if self.armed && !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// A finished piece of work waiting for its turn in the archive.
enum Prepared {
    Mem(Vec<u8>),
    File { path: PathBuf },
}

impl Prepared {
    /// Add the bytes to the archive. The archive's own writer counts them for
    /// the size estimate (CountedW), so nothing is counted here.
    fn copy_to<W: Write>(&self, w: &mut W) -> Result<()> {
        match self {
            Prepared::Mem(v) => {
                w.write_all(v).map_err(|e| from_io(e, "Write error"))?;
            }
            Prepared::File { path, .. } => {
                let mut f = File::open(path)
                    .map_err(|e| NikError::Fatal(format!("Cannot read '{}': {e}", path.display())))?;
                let mut buf = vec![0u8; IO_BUF];
                loop {
                    let n = f
                        .read(&mut buf)
                        .map_err(|e| NikError::Fatal(format!("Cannot read '{}': {e}", path.display())))?;
                    if n == 0 {
                        break;
                    }
                    w.write_all(&buf[..n]).map_err(|e| from_io(e, "Write error"))?;
                    check_cancel()?;
                }
            }
        }
        Ok(())
    }
}

impl Drop for Prepared {
    fn drop(&mut self) {
        if let Prepared::File { path, .. } = self {
            let _ = fs::remove_file(path);
        }
    }
}

/// A unique temporary file name beside the archive, for a holding area.
fn spill_path(archive: &Path, tag: u64) -> PathBuf {
    let mut s = archive.as_os_str().to_owned();
    s.push(format!(".part{tag}.tmp"));
    PathBuf::from(s)
}

/// Do we run several entries/blocks at the same time? `NIKPAK_SEQ=1` forces the
/// old one-at-a-time path (for comparing, or when a machine behaves badly).
fn parallel_wanted(threads: usize) -> bool {
    threads > 1 && std::env::var_os("NIKPAK_SEQ").is_none()
}

/// Write entries one at a time, straight into `w` (the archive, a block, or a
/// holding area). `order` is the sequence of item indices to write.
fn write_items_list<W: Write>(
    w: &mut W,
    items: &[Item],
    order: &[usize],
    o: &Options,
    filters_on: bool,
    recomp_on: bool,
    stats: &mut Stats,
    print: bool,
) -> Result<()> {
    let mut buf = vec![0u8; IO_BUF];
    for &idx in order {
        check_cancel()?;
        let done = write_item_records(w, &items[idx], o, filters_on, recomp_on, &mut buf)?;
        // before the log is moved out of `done` below
        stats.add(&done);
        if !done.log.is_empty() {
            if print {
                outln!("{}", done.log);
            } else {
                // a worker thread: the console belongs to the main thread, which
                // prints these in archive order
                stats.logs.push(done.log);
            }
        }
    }
    Ok(())
}

/// Bytes all the entries held out of order may keep in memory together; each
/// one spills into a temporary file past its share, so this is the ceiling of
/// what the preparation pipeline costs in RAM.
const ITEM_SPILL_TOTAL: u64 = 256 << 20;
/// Bytes all blocks together may hold in memory at -m5.
const BLOCK_SPILL_TOTAL: u64 = 256 << 20;

/// Prepare `chunk` on several threads and write the results in order: the
/// expensive per-entry work (reading, media trials, Deflate recompression)
/// runs on all cores, while the archive itself is still written strictly in
/// the order it lists. A batch waits for its slowest entry, so its width is a
/// compromise: wide enough that the wait is cheap, small enough to stay inside
/// the memory ceiling below.
fn write_items_pipelined<W: Write>(
    w: &mut W,
    items: &[Item],
    order: &[usize],
    o: &Options,
    filters_on: bool,
    recomp_on: bool,
    threads: usize,
    archive: &Path,
    stats: &mut Stats,
) -> Result<()> {
    // A batch has to be wide enough that waiting for its slowest entry is
    // cheap, and small enough to keep the memory ceiling above: three entries
    // per thread does both.
    let batch = (threads * 3).max(2).min(order.len().max(1));
    let per_item = (ITEM_SPILL_TOTAL / batch as u64).clamp(1 << 20, 32 << 20);
    let mut tag: u64 = 0;
    for chunk in order.chunks(batch) {
        check_cancel()?;
        let base = tag;
        let prepared: Vec<(Result<ItemDone>, Spill)> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .enumerate()
                .map(|(k, &idx)| {
                    let path = spill_path(archive, base + k as u64);
                    let it = &items[idx];
                    s.spawn(move || {
                        let mut sp = Spill::new(path, per_item);
                        let mut buf = vec![0u8; IO_BUF];
                        let r = write_item_records(&mut sp, it, o, filters_on, recomp_on, &mut buf);
                        (r, sp)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| (Err(NikError::Fatal("Internal error while compressing.".into())), Spill::new(PathBuf::new(), 0))))
                .collect()
        });
        tag += chunk.len() as u64;
        for (r, sp) in prepared {
            let done = r?;
            // a skipped entry wrote nothing: its holding area is empty and
            // copying it adds no bytes
            sp.finish().map_err(|e| from_io(e, "Write error"))?.copy_to(w)?;
            if !done.log.is_empty() {
                outln!("{}", done.log);
            }
            stats.add(&done);
        }
    }
    Ok(())
}

// ===========================================================================
// Reader-side plumbing
// ===========================================================================

enum Source {
    Plain(BufReader<Chain>),
    Dec(DecryptReader<BufReader<Chain>>),
}

impl Read for Source {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Source::Plain(r) => r.read(b),
            Source::Dec(r) => r.read(b),
        }
    }
}

enum Inner {
    Store(Source),
    Zstd(zstd::stream::read::Decoder<'static, BufReader<Source>>),
    Blocks(BlockReader<Source>),
}

impl Read for Inner {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Inner::Store(r) => r.read(b),
            Inner::Zstd(r) => r.read(b),
            Inner::Blocks(r) => r.read(b),
        }
    }
}

struct Opened {
    inner: Inner,
    encrypted: bool,
    method: u8,
    /// kept to write the archive again (delete)
    password: Option<zeroize::Zeroizing<String>>,
    /// the archive's legal statement (already accepted when we got here)
    legal: Option<String>,
    /// the cover picture, kept to write the archive again (delete)
    cover: Option<Vec<u8>>,
}

fn open_archive(o: &Options) -> Result<Opened> {
    let path = Path::new(&o.archive);
    // A Chain reads the archive itself, the volumes that follow it, or the
    // archive appended to a self-extracting .exe, as one stream (split.rs).
    let chain = Chain::open(path).map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            NikError::Fatal(format!("Cannot open '{}': file not found.", o.archive))
        } else {
            NikError::Fatal(format!("Cannot open '{}': {e}", o.archive))
        }
    })?;
    PROGRESS.done.store(0, Relaxed);
    PROGRESS.total.store(chain.total(), Relaxed);
    let mut br = BufReader::with_capacity(IO_BUF, chain);

    let mut fixed = [0u8; 12];
    br.read_exact(&mut fixed).map_err(|_| {
        NikError::Corrupt(format!("'{}' is not a valid .nik archive (file too short).", o.archive))
    })?;
    if fixed[..8] != MAGIC {
        // A Nikkosynth soundbank shares the extension: say which it is.
        if fixed[..8] == *crate::soundbank::MAGIC {
            let info = crate::soundbank::read_info(path).unwrap_or_default();
            return Err(NikError::Fatal(crate::soundbank::describe(&info)));
        }
        // A NikPak package does too.
        if fixed[..8] == *crate::package::MAGIC {
            let info = crate::package::read_info(path).unwrap_or_default();
            return Err(NikError::Fatal(crate::package::describe(&info)));
        }
        return Err(NikError::Corrupt(format!("'{}' is not a .nik archive (bad signature).", o.archive)));
    }
    if fixed[8] == 0 || fixed[8] > VERSION {
        return Err(NikError::Fatal(format!(
            "Unsupported archive version {}. This nikpak understands version {VERSION}; please update nikpak.",
            fixed[8]
        )));
    }
    let flags = fixed[9];
    let method = fixed[10];
    if flags & !(FLAG_ENCRYPTED | FLAG_LEGAL | FLAG_COVER | FLAG_SPLIT) != 0 || fixed[11] != 0 {
        return Err(NikError::Fatal("Archive uses unknown features; please update nikpak.".into()));
    }
    if method > METHOD_BLOCKS || (method == METHOD_BLOCKS && fixed[8] < 3) {
        return Err(NikError::Fatal("Archive uses an unknown compression method; please update nikpak.".into()));
    }
    let encrypted = flags & FLAG_ENCRYPTED != 0;

    // the whole header, as the writer bound it to every encrypted chunk
    let mut header_bytes = fixed.to_vec();
    let mut kdf = None;
    if encrypted {
        let mut ext = [0u8; 12 + SALT_LEN + NONCE_PREFIX_LEN];
        br.read_exact(&mut ext)
            .map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
        let mem = u32::from_le_bytes(ext[0..4].try_into().unwrap());
        let time = u32::from_le_bytes(ext[4..8].try_into().unwrap());
        let lanes = u32::from_le_bytes(ext[8..12].try_into().unwrap());
        crypto::check_kdf_params(mem, time, lanes).map_err(NikError::Corrupt)?;
        let mut salt = [0u8; SALT_LEN];
        salt.copy_from_slice(&ext[12..12 + SALT_LEN]);
        let mut nonce = [0u8; NONCE_PREFIX_LEN];
        nonce.copy_from_slice(&ext[12 + SALT_LEN..]);
        header_bytes.extend_from_slice(&ext);
        kdf = Some((mem, time, lanes, salt, nonce));
    }
    let legal = if flags & FLAG_LEGAL != 0 {
        let text = read_legal(&mut br)?;
        header_bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
        header_bytes.extend_from_slice(text.as_bytes());
        Some(text)
    } else {
        None
    };
    let cover = if flags & FLAG_COVER != 0 {
        let pic = read_cover(&mut br)?;
        header_bytes.extend_from_slice(&(pic.len() as u32).to_le_bytes());
        header_bytes.extend_from_slice(&pic);
        Some(pic)
    } else {
        None
    };
    // A split archive says how big a volume is: a full volume then means that
    // another one follows, and a missing one is named as such.
    if flags & FLAG_SPLIT != 0 {
        let mut size = [0u8; 8];
        br.read_exact(&mut size).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
        header_bytes.extend_from_slice(&size);
        let size = u64::from_le_bytes(size);
        if size == 0 {
            return Err(NikError::Corrupt("Archive header is damaged: its volume size is 0.".into()));
        }
        br.get_mut().set_volume_size(size);
    }
    // **The legal statement comes first**: nothing is decrypted or read until it
    // has been accepted - in a window (accept_legal), with -accept, or here.
    if let Some(text) = &legal {
        require_legal(path, text, o)?;
    }

    let mut password = None;
    let source = if let Some((mem, time, lanes, salt, nonce)) = kdf {
        let pw = password_for_open(&o.password)?;
        if !o.quiet {
            outln!("Deriving decryption key (Argon2id)...");
        }
        let key = crypto::derive_key(&pw, &salt, mem, time, lanes).map_err(NikError::Fatal)?;
        password = Some(pw);
        Source::Dec(DecryptReader::new(br, &key, &nonce, header_bytes))
    } else {
        Source::Plain(br)
    };

    let inner = match method {
        METHOD_ZSTD => Inner::Zstd(
            zstd::stream::read::Decoder::new(source).map_err(|e| from_io(e, "Cannot start decompressor"))?,
        ),
        METHOD_BLOCKS => Inner::Blocks(BlockReader::new(source)),
        _ => Inner::Store(source),
    };
    Ok(Opened { inner, encrypted, method, password, legal, cover })
}

// ---------------------------------------------------------------------------
// Legal statement
// ---------------------------------------------------------------------------

/// Statements accepted in this process: (archive, the exact text accepted).
/// A window asks once, then every operation on that archive goes through; a
/// statement that changed is asked again.
static ACCEPTED: std::sync::Mutex<Vec<(PathBuf, String)>> = std::sync::Mutex::new(Vec::new());

fn accept_key(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Record that the user accepted `text`, the legal statement of `path`.
/// Call it after the user said "I approve"; later opens of that archive pass.
pub fn accept_legal(path: &Path, text: &str) {
    let key = accept_key(path);
    if let Ok(mut v) = ACCEPTED.lock() {
        v.retain(|(p, _)| *p != key);
        v.push((key, text.to_string()));
    }
}

/// Was `text`, the legal statement of `path`, accepted in this process?
pub fn legal_is_accepted(path: &Path, text: &str) -> bool {
    legal_accepted(path, text)
}

fn legal_accepted(path: &Path, text: &str) -> bool {
    let key = accept_key(path);
    ACCEPTED.lock().map(|v| v.iter().any(|(p, t)| *p == key && t == text)).unwrap_or(false)
}

/// Normalise the statement to attach: trimmed, `None` when empty, refused when long.
fn checked_legal(text: Option<&str>) -> Result<Option<String>> {
    let Some(t) = text.map(str::trim).filter(|t| !t.is_empty()) else { return Ok(None) };
    let t = t.replace("\r\n", "\n");
    if t.len() > MAX_LEGAL_LEN {
        return Err(NikError::Usage(format!(
            "The legal statement is too long ({} bytes; at most {MAX_LEGAL_LEN}).",
            t.len()
        )));
    }
    Ok(Some(t))
}

fn read_legal<R: Read>(r: &mut R) -> Result<String> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    let n = u32::from_le_bytes(n) as usize;
    if n == 0 || n > MAX_LEGAL_LEN {
        return Err(NikError::Corrupt("The archive's legal statement is damaged.".into()));
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    String::from_utf8(b).map_err(|_| NikError::Corrupt("The archive's legal statement is damaged.".into()))
}

/// Check a cover handed to `create`: a JPEG or PNG, not too large.
fn checked_cover(pic: Option<&[u8]>) -> Result<Option<Vec<u8>>> {
    let Some(pic) = pic.filter(|p| !p.is_empty()) else { return Ok(None) };
    if crate::cover::kind(pic).is_none() {
        return Err(NikError::Usage("The cover must be a JPEG or PNG picture.".into()));
    }
    if pic.len() > crate::cover::MAX_COVER_BYTES {
        return Err(NikError::Usage("The cover picture is too large (at most 4 MB).".into()));
    }
    Ok(Some(pic.to_vec()))
}

fn read_cover<R: Read>(r: &mut R) -> Result<Vec<u8>> {
    let mut n = [0u8; 4];
    r.read_exact(&mut n).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    let n = u32::from_le_bytes(n) as usize;
    if n == 0 || n > crate::cover::MAX_COVER_BYTES {
        return Err(NikError::Corrupt("The archive's cover is damaged.".into()));
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    if crate::cover::kind(&b).is_none() {
        return Err(NikError::Corrupt("The archive's cover is damaged.".into()));
    }
    Ok(b)
}

/// The archive's cover picture (JPEG or PNG bytes), if it has one. Reads only the
/// header, like [`legal_statement`]: no password, and no need to accept the terms
/// first, so a window can show the cover beside them.
/// The archive's bytes: its first volume, the volumes that follow, or the
/// archive appended to a self-extracting .exe.
fn open_bytes(path: &Path) -> Result<Chain> {
    Chain::open(path).map_err(|e| NikError::Fatal(format!("Cannot open '{}': {e}", path.display())))
}

pub fn cover_image(path: &Path) -> Result<Option<Vec<u8>>> {
    let mut f = BufReader::with_capacity(IO_BUF, open_bytes(path)?);
    let mut fixed = [0u8; 12];
    f.read_exact(&mut fixed)
        .map_err(|_| NikError::Corrupt(format!("'{}' is not a valid .nik archive (file too short).", path.display())))?;
    if fixed[..8] != MAGIC {
        return Err(NikError::Corrupt(format!("'{}' is not a .nik archive.", path.display())));
    }
    if fixed[9] & FLAG_COVER == 0 {
        return Ok(None);
    }
    if fixed[9] & FLAG_ENCRYPTED != 0 {
        let mut ext = [0u8; 12 + SALT_LEN + NONCE_PREFIX_LEN];
        f.read_exact(&mut ext).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    }
    if fixed[9] & FLAG_LEGAL != 0 {
        read_legal(&mut f)?;
    }
    read_cover(&mut f).map(Some)
}

/// The archive's legal statement, if it has one. Reads only the header: no
/// password is needed, so a window can show it before asking for one.
pub fn legal_statement(path: &Path) -> Result<Option<String>> {
    let mut f = BufReader::with_capacity(IO_BUF, open_bytes(path)?);
    let mut fixed = [0u8; 12];
    f.read_exact(&mut fixed)
        .map_err(|_| NikError::Corrupt(format!("'{}' is not a valid .nik archive (file too short).", path.display())))?;
    if fixed[..8] != MAGIC {
        return Err(NikError::Corrupt(format!("'{}' is not a .nik archive.", path.display())));
    }
    if fixed[9] & FLAG_LEGAL == 0 {
        return Ok(None);
    }
    if fixed[9] & FLAG_ENCRYPTED != 0 {
        let mut ext = [0u8; 12 + SALT_LEN + NONCE_PREFIX_LEN];
        f.read_exact(&mut ext).map_err(|_| NikError::Corrupt("Archive header is truncated.".into()))?;
    }
    read_legal(&mut f).map(Some)
}

/// Stop unless the statement was accepted. On a console (not -q) it is shown
/// and asked; a window must call [`accept_legal`] first.
fn require_legal(path: &Path, text: &str, o: &Options) -> Result<()> {
    if o.accept_legal || legal_accepted(path, text) {
        return Ok(());
    }
    use std::io::IsTerminal;
    if !o.quiet && io::stdin().is_terminal() {
        eprintln!("\nThis archive comes with a legal statement:\n");
        eprintln!("{text}\n");
        loop {
            eprint!("Do you accept it? Type YES to accept, NO to stop: ");
            let _ = io::stderr().flush();
            let mut s = String::new();
            if io::stdin().read_line(&mut s).unwrap_or(0) == 0 {
                break;
            }
            match s.trim().to_ascii_lowercase().as_str() {
                "yes" | "y" => {
                    accept_legal(path, text);
                    return Ok(());
                }
                "no" | "n" | "" => break,
                _ => continue,
            }
        }
        return Err(NikError::Legal("The legal statement was not accepted; nothing was done.".into()));
    }
    Err(NikError::Legal(
        "This archive carries a legal statement that must be accepted before it is opened.\n\
         Run the command in a console to read it, or add -accept to accept it."
            .into(),
    ))
}

// ---------------------------------------------------------------------------
// Record parsing
// ---------------------------------------------------------------------------

struct MediaHdr {
    filter: u8,
    method: u8,
    params: Vec<u8>,
}

struct Rec {
    is_dir: bool,
    name: String,
    mtime: i64,
    mode: u32,
    size: u64,
    media: Option<MediaHdr>,
}

enum Next {
    Rec(Rec),
    End(u64),
}

fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}
fn read_u16<R: Read>(r: &mut R) -> io::Result<u16> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b)?;
    Ok(u16::from_le_bytes(b))
}
fn read_u32<R: Read>(r: &mut R) -> io::Result<u32> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b)?;
    Ok(u32::from_le_bytes(b))
}
fn read_u64<R: Read>(r: &mut R) -> io::Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn next_record<R: Read>(r: &mut R) -> Result<Next> {
    let ctx = "Reading archive";
    let kind = read_u8(r).map_err(|e| from_io(e, ctx))?;
    match kind {
        REC_END => {
            let n = read_u64(r).map_err(|e| from_io(e, ctx))?;
            Ok(Next::End(n))
        }
        REC_FILE | REC_DIR | REC_MEDIA => {
            let nlen = read_u16(r).map_err(|e| from_io(e, ctx))? as usize;
            if nlen > MAX_NAME_LEN {
                return Err(NikError::Corrupt("Archive is damaged (bad name length).".into()));
            }
            let mut nb = vec![0u8; nlen];
            r.read_exact(&mut nb).map_err(|e| from_io(e, ctx))?;
            let name = String::from_utf8(nb)
                .map_err(|_| NikError::Corrupt("Archive is damaged (invalid file name encoding).".into()))?;
            let mtime = read_u64(r).map_err(|e| from_io(e, ctx))? as i64;
            let mode = read_u32(r).map_err(|e| from_io(e, ctx))?;
            let size = if kind != REC_DIR { read_u64(r).map_err(|e| from_io(e, ctx))? } else { 0 };
            let media = if kind == REC_MEDIA {
                let filter = read_u8(r).map_err(|e| from_io(e, ctx))?;
                let method = read_u8(r).map_err(|e| from_io(e, ctx))?;
                let plen = read_u8(r).map_err(|e| from_io(e, ctx))? as usize;
                let mut params = vec![0u8; plen];
                r.read_exact(&mut params).map_err(|e| from_io(e, ctx))?;
                Some(MediaHdr { filter, method, params })
            } else {
                None
            };
            Ok(Next::Rec(Rec { is_dir: kind == REC_DIR, name, mtime, mode, size, media }))
        }
        _ => Err(NikError::Corrupt("Archive is damaged (unknown record type).".into())),
    }
}

/// Copy exactly `size` bytes from r to w (w may be io::sink()), return CRC32.
fn copy_exact<R: Read, W: Write>(r: &mut R, w: &mut W, size: u64, buf: &mut [u8]) -> Result<u32> {
    let mut crc = crc32fast::Hasher::new();
    let mut remaining = size;
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = r.read(&mut buf[..want]).map_err(|e| from_io(e, "Reading archive"))?;
        if n == 0 {
            return Err(NikError::Corrupt("Unexpected end of archive (file truncated?).".into()));
        }
        crc.update(&buf[..n]);
        w.write_all(&buf[..n])
            .map_err(|e| NikError::Fatal(format!("Write error: {e}")))?;
        remaining -= n as u64;
    }
    Ok(crc.finalize())
}

/// Read the data of one file entry (plain or media-filtered) into `w`; returns
/// the CRC32 of the reconstructed original bytes.
fn entry_data<W: Write>(inner: &mut Inner, rec: &Rec, w: &mut W, buf: &mut [u8]) -> Result<u32> {
    match &rec.media {
        None => copy_exact(inner, w, rec.size, buf),
        Some(m) => {
            let mut cr = media::ChunkReader::new(inner);
            let mut cw = media::CrcWrite::new(&mut *w);
            media::decode(&mut cr, rec.size, m.filter, m.method, &m.params, &mut cw)
                .map_err(|e| from_io(e, "Reading media data"))?;
            let extra = cr.drain().map_err(|e| from_io(e, "Reading media data"))?;
            if extra != 0 {
                return Err(NikError::Corrupt(format!("Archive is damaged (extra data after '{}').", rec.name)));
            }
            let (crc, count) = cw.finish();
            if count != rec.size {
                return Err(NikError::Corrupt(format!("Archive is damaged ('{}' has the wrong length).", rec.name)));
            }
            Ok(crc)
        }
    }
}

/// Skip a file entry. Plain entries are CRC-checked, media entries are only
/// skipped over (use the `t` command to fully verify them).
fn skip_entry(inner: &mut Inner, rec: &Rec, buf: &mut [u8]) -> Result<()> {
    if rec.media.is_some() {
        media::ChunkReader::new(inner).drain().map_err(|e| from_io(e, "Reading archive"))?;
        read_u32(inner).map_err(|e| from_io(e, "Reading archive"))?;
        return Ok(());
    }
    let c = copy_exact(inner, &mut io::sink(), rec.size, buf)?;
    let stored = read_u32(inner).map_err(|e| from_io(e, "Reading archive"))?;
    if stored != c {
        return Err(NikError::Corrupt(format!("CRC mismatch in '{}': archive is damaged.", rec.name)));
    }
    Ok(())
}

/// After REC_END: make sure the stream really ends here and (if encrypted)
/// that the final authenticated chunk has been verified.
fn verify_end(inner: &mut Inner) -> Result<()> {
    let mut sink = io::sink();
    io::copy(inner, &mut sink).map_err(|e| from_io(e, "Verifying end of archive"))?;
    Ok(())
}

// ===========================================================================
// LIST
// ===========================================================================

pub fn list(o: &Options) -> Result<u8> {
    let mut op = open_archive(o)?;
    let verbose = o.command == Command::ListVerbose;
    let bare = o.command == Command::ListBare;
    let mut buf = vec![0u8; IO_BUF];
    let (mut nfiles, mut ndirs, mut total) = (0u64, 0u64, 0u64);
    let mut matched = 0u64;

    if !bare {
        outln!(
            "Archive: {}{}{}",
            o.archive,
            if op.encrypted { "   [encrypted]" } else { "" },
            if op.legal.is_some() { "   [legal statement accepted]" } else { "" }
        );
        if let Some(c) = &op.cover {
            outln!("Cover:   {} KB picture", c.len().div_ceil(1024));
        }
        outln!();
        if verbose {
            outln!("{:<10} {:>14}  {:<16} {:<8}  Name", "Attr", "Size", "Date (UTC)", "CRC32");
        } else {
            outln!("{:<10} {:>14}  {:<16}  Name", "Attr", "Size", "Date (UTC)");
        }
        outln!("{}", "-".repeat(70));
    }

    let count = loop {
        match next_record(&mut op.inner)? {
            Next::End(c) => break c,
            Next::Rec(r) => {
                let crc = if r.is_dir {
                    0
                } else if r.media.is_some() {
                    media::ChunkReader::new(&mut op.inner).drain().map_err(|e| from_io(e, "Reading archive"))?;
                    read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?
                } else {
                    let c = copy_exact(&mut op.inner, &mut io::sink(), r.size, &mut buf)?;
                    let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                    if stored != c {
                        eprintln!("WARNING: CRC mismatch for '{}'", r.name);
                    }
                    stored
                };
                if !passes_filters(&r.name, o) {
                    continue;
                }
                matched += 1;
                if r.is_dir {
                    ndirs += 1;
                } else {
                    nfiles += 1;
                    total += r.size;
                }
                if bare {
                    outln!("{}{}", r.name, if r.is_dir { "/" } else { "" });
                } else {
                    let attr = attr_string(r.is_dir, r.mode);
                    if verbose {
                        outln!(
                            "{:<10} {:>14}  {:<16} {:08X}  {}{}",
                            attr, r.size, format_time(r.mtime), crc, r.name,
                            if r.is_dir { "/" } else { "" }
                        );
                    } else {
                        outln!(
                            "{:<10} {:>14}  {:<16}  {}{}",
                            attr, r.size, format_time(r.mtime), r.name,
                            if r.is_dir { "/" } else { "" }
                        );
                    }
                }
            }
        }
    };
    verify_end(&mut op.inner)?;

    if !bare {
        outln!("{}", "-".repeat(70));
        outln!("{} files, {} folders, {} bytes ({})", nfiles, ndirs, total, human_size(total));
    }
    let _ = count;
    if matched == 0 && !o.files.is_empty() {
        return Err(NikError::NoFiles("No files in the archive match the given names.".into()));
    }
    Ok(0)
}

fn attr_string(is_dir: bool, mode: u32) -> String {
    let mut s = String::with_capacity(10);
    s.push(if is_dir { 'd' } else { '-' });
    let bits = ['r', 'w', 'x'];
    for i in (0..9).rev() {
        let bit = (mode >> i) & 1;
        s.push(if bit == 1 { bits[(8 - i) % 3] } else { '-' });
    }
    s
}

fn passes_filters(name: &str, o: &Options) -> bool {
    if o.excludes.iter().any(|x| filter_matches(x, name)) {
        return false;
    }
    if o.files.is_empty() {
        return true;
    }
    o.files.iter().any(|f| filter_matches(f, name))
}

// ===========================================================================
// TEST
// ===========================================================================

pub fn test(o: &Options) -> Result<u8> {
    let mut op = open_archive(o)?;
    let mut buf = vec![0u8; IO_BUF];
    let mut bad = 0u64;
    let mut n = 0u64;
    let declared = loop {
        match next_record(&mut op.inner)? {
            Next::End(c) => break c,
            Next::Rec(r) => {
                n += 1;
                if r.is_dir {
                    continue;
                }
                let c = entry_data(&mut op.inner, &r, &mut io::sink(), &mut buf)?;
                let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                if stored == c {
                    if !o.quiet {
                        outln!("Testing   {}   OK", r.name);
                    }
                } else {
                    bad += 1;
                    eprintln!("Testing   {}   FAILED (CRC mismatch)", r.name);
                }
            }
        }
    };
    verify_end(&mut op.inner)?;
    if declared != n {
        return Err(NikError::Corrupt("Entry count mismatch: the archive is damaged.".into()));
    }
    if bad > 0 {
        return Err(NikError::Corrupt(format!("{bad} file(s) failed the CRC check.")));
    }
    if !o.quiet {
        outln!();
        outln!(
            "All OK ({} entries){}",
            n,
            if op.encrypted { " - authentication verified" } else { "" }
        );
    }
    Ok(0)
}

// ===========================================================================
// EXTRACT
// ===========================================================================

fn unique_path(p: &Path) -> PathBuf {
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    for i in 1..10_000 {
        let c = dir.join(format!("{stem} ({i}){ext}"));
        if !c.exists() && fs::symlink_metadata(&c).is_err() {
            return c;
        }
    }
    p.to_path_buf()
}

/// An exact choice of entries (the window): the names of the files and
/// folders to extract, and a folder prefix that is removed from the stored
/// names (extracting `docs/a.txt` while browsing `docs/` creates `a.txt`).
#[derive(Clone, Debug, Default)]
pub struct Pick {
    pub names: std::collections::HashSet<String>,
    pub strip: String,
}

impl Pick {
    fn wants(&self, name: &str) -> bool {
        self.names.contains(name)
    }
    /// stored name -> name below the destination
    fn relative<'a>(&self, name: &'a str) -> &'a str {
        if !self.strip.is_empty() && name.len() > self.strip.len() && name[..self.strip.len()].eq_ignore_ascii_case(&self.strip) {
            &name[self.strip.len()..]
        } else {
            name
        }
    }
}

/// The file written last by `extract_pick` (the window opens it).
pub static LAST_EXTRACTED: std::sync::Mutex<Option<PathBuf>> = std::sync::Mutex::new(None);

pub fn extract(o: &Options) -> Result<u8> {
    extract_pick(o, None)
}

/// Extract the entries chosen by the command line filters, or exactly the
/// entries of `pick`. With a pick, reading stops after its last entry.
pub fn extract_pick(o: &Options, pick: Option<&Pick>) -> Result<u8> {
    let started = Instant::now();
    let dest = o.dest.clone().unwrap_or_else(|| PathBuf::from("."));
    if dest.exists() && !dest.is_dir() {
        return Err(NikError::Fatal(format!("Destination '{}' is not a folder.", dest.display())));
    }
    let mut op = open_archive(o)?;
    fs::create_dir_all(&dest)
        .map_err(|e| NikError::Fatal(format!("Cannot create destination '{}': {e}", dest.display())))?;

    let flat = o.command == Command::ExtractFlat;
    let mut overwrite = o.overwrite;
    let mut buf = vec![0u8; IO_BUF];
    let mut warnings = 0u32;
    // how many files are out: the window's Extract box reports it when the
    // extraction ends (bytes read are counted by `Chain` in PROGRESS.done)
    PROGRESS.files_done.store(0, Relaxed);
    let mut extracted = 0u64;
    let mut total = 0u64;
    let mut matched = 0u64;
    let mut dir_times: Vec<(PathBuf, i64)> = Vec::new();

    let declared = loop {
        let rec = match next_record(&mut op.inner)? {
            Next::End(c) => break Some(c),
            Next::Rec(r) => r,
        };

        check_cancel()?;
        let want = match pick {
            Some(p) => p.wants(&rec.name),
            None => passes_filters(&rec.name, o),
        };
        let shown_name = pick.map_or(rec.name.as_str(), |p| p.relative(&rec.name));
        // Validate and map the name
        let comps = match sanitize_entry_name(shown_name) {
            Ok(c) if !c.is_empty() || !want => Some(c),
            Ok(_) => None,
            Err(why) if want => {
                warnings += 1;
                warn(format!("Skipping unsafe entry '{}': {why}", rec.name));
                None
            }
            Err(_) => None,
        };
        let comps = if want { comps } else { None };

        if rec.is_dir {
            if let Some(c) = comps {
                matched += 1;
                if flat || c.is_empty() {
                    if pick.is_some_and(|p| matched as usize >= p.names.len()) {
                        break None;
                    }
                    continue;
                }
                if let Err(why) = check_no_symlinks(&dest, &c) {
                    warnings += 1;
                    warn(format!("Skipping '{}': {why}", rec.name));
                    continue;
                }
                let p = join_components(&dest, &c);
                if let Err(e) = fs::create_dir_all(&p) {
                    warnings += 1;
                    warn(format!("Cannot create folder '{}': {e}", p.display()));
                    continue;
                }
                dir_times.push((p, rec.mtime));
            }
            if pick.is_some_and(|p| matched as usize >= p.names.len()) {
                break None;
            }
            continue;
        }

        // ---- file entry ----
        let target_comps: Option<Vec<String>> = comps.map(|c| {
            if flat { vec![c.last().cloned().unwrap_or_default()] } else { c }
        });
        let Some(tc) = target_comps else {
            // skipped: consume the data
            skip_entry(&mut op.inner, &rec, &mut buf)?;
            continue;
        };
        matched += 1;

        let mut target = join_components(&dest, &tc);
        let mut skip_reason: Option<String> = None;

        if let Err(why) = check_no_symlinks(&dest, &tc) {
            skip_reason = Some(why);
        } else if let Some(parent) = target.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                skip_reason = Some(format!("cannot create folder '{}': {e}", parent.display()));
            }
        }

        if skip_reason.is_none() && fs::symlink_metadata(&target).is_ok() {
            let mut choice = overwrite;
            if choice == Overwrite::Ask {
                match ask_yes_no(&format!("'{}' already exists. Overwrite?", target.display())) {
                    Some('y') => choice = Overwrite::All,
                    Some('a') => {
                        overwrite = Overwrite::All;
                        choice = Overwrite::All;
                    }
                    Some('s') => {
                        overwrite = Overwrite::Skip;
                        choice = Overwrite::Skip;
                    }
                    _ => choice = Overwrite::Skip,
                }
            }
            match choice {
                Overwrite::All => {
                    // remove first so we never write THROUGH an existing link
                    let md = fs::symlink_metadata(&target);
                    let res = match md {
                        Ok(m) if m.is_dir() => Err(io::Error::new(io::ErrorKind::Other, "a folder with that name exists")),
                        _ => fs::remove_file(&target),
                    };
                    if let Err(e) = res {
                        skip_reason = Some(format!("cannot replace existing file: {e}"));
                    }
                }
                Overwrite::Rename => target = unique_path(&target),
                _ => skip_reason = Some("already exists (skipped)".into()),
            }
        }

        if let Some(why) = skip_reason {
            warnings += 1;
            warn(format!("'{}': {why}", rec.name));
            skip_entry(&mut op.inner, &rec, &mut buf)?;
            continue;
        }

        // ---- write the file (create_new: never follows a pre-existing link)
        let mut f = match OpenOptions::new().write(true).create_new(true).open(&target) {
            Ok(f) => f,
            Err(e) => {
                warnings += 1;
                warn(format!("Cannot create '{}': {e} - skipped", target.display()));
                skip_entry(&mut op.inner, &rec, &mut buf)?;
                continue;
            }
        };
        let res = {
            let mut w = BufWriter::with_capacity(IO_BUF, &mut f);
            let r = entry_data(&mut op.inner, &rec, &mut w, &mut buf);
            match r {
                Ok(c) => w.flush().map(|_| c).map_err(|e| NikError::Fatal(format!("Write error on '{}': {e}", target.display()))),
                Err(e) => Err(e),
            }
        };
        let crc = match res {
            Ok(c) => c,
            Err(e) => {
                drop(f);
                let _ = fs::remove_file(&target);
                return Err(e);
            }
        };
        let stored = read_u32(&mut op.inner).map_err(|e| {
            let _ = fs::remove_file(&target);
            from_io(e, "Reading archive")
        })?;
        if stored != crc {
            drop(f);
            let _ = fs::remove_file(&target);
            return Err(NikError::Corrupt(format!(
                "CRC mismatch in '{}': the archive is damaged. The file was not kept.",
                rec.name
            )));
        }
        if rec.mtime >= 0 {
            let t = UNIX_EPOCH + std::time::Duration::from_secs(rec.mtime as u64);
            let _ = f.set_modified(t);
        }
        drop(f);
        set_file_mode(&target, rec.mode);

        extracted += 1;
        PROGRESS.files_done.fetch_add(1, Relaxed);
        total += rec.size;
        if !o.quiet {
            outln!("Extracting  {}   OK", target.display());
        }
        if let Some(p) = pick {
            LAST_EXTRACTED.lock().map(|mut v| *v = Some(target.clone())).ok();
            if matched as usize >= p.names.len() {
                break None; // everything chosen is out: no need to read further
            }
        }
    };
    if declared.is_some() {
        verify_end(&mut op.inner)?;
    }

    // folder timestamps last (adding files changes them)
    #[cfg(unix)]
    for (p, t) in dir_times.iter().rev() {
        if *t >= 0 {
            if let Ok(d) = File::open(p) {
                let _ = d.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(*t as u64));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = &dir_times;

    let _ = declared;
    if matched == 0 && !o.files.is_empty() {
        return Err(NikError::NoFiles("No files in the archive match the given names.".into()));
    }
    if !o.quiet {
        outln!();
        outln!(
            "Done. {} files extracted, {}, {:.2}s",
            extracted,
            human_size(total),
            started.elapsed().as_secs_f64()
        );
    }
    let _ = SystemTime::now;
    Ok(if warnings > 0 { 1 } else { 0 })
}

#[cfg(unix)]
fn set_file_mode(p: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    // strip setuid / setgid / sticky: never restore privileged bits from an archive
    let m = (mode & 0o777) | 0o400;
    let _ = fs::set_permissions(p, fs::Permissions::from_mode(m));
}

#[cfg(not(unix))]
fn set_file_mode(p: &Path, mode: u32) {
    if mode & 0o200 == 0 {
        if let Ok(md) = fs::metadata(p) {
            let mut perms = md.permissions();
            perms.set_readonly(true);
            let _ = fs::set_permissions(p, perms);
        }
    }
}


// ===========================================================================
// PRINT (nikpak p): file contents to standard output
// ===========================================================================

pub fn print(o: &Options) -> Result<u8> {
    let mut op = open_archive(o)?;
    let mut buf = vec![0u8; IO_BUF];
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut matched = 0u64;
    loop {
        match next_record(&mut op.inner)? {
            Next::End(_) => break,
            Next::Rec(r) if r.is_dir => {}
            Next::Rec(r) if passes_filters(&r.name, o) => {
                matched += 1;
                let c = entry_data(&mut op.inner, &r, &mut out, &mut buf)?;
                let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                if stored != c {
                    return Err(NikError::Corrupt(format!("CRC mismatch in '{}': the archive is damaged.", r.name)));
                }
            }
            Next::Rec(r) => skip_entry(&mut op.inner, &r, &mut buf)?,
        }
    }
    verify_end(&mut op.inner)?;
    let _ = out.flush();
    if matched == 0 {
        return Err(NikError::NoFiles("No files in the archive match the given names.".into()));
    }
    Ok(0)
}

// ===========================================================================
// DELETE (nikpak d): write the archive again without the chosen entries
// ===========================================================================

/// Copy the chunk stream of a media / recompressed entry unchanged.
fn copy_chunks<R: Read, W: Write>(r: &mut R, w: &mut W, buf: &mut [u8]) -> Result<()> {
    loop {
        let len = read_u32(r).map_err(|e| from_io(e, "Reading archive"))?;
        w.write_all(&len.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
        if len == 0 {
            return Ok(());
        }
        if len > (64 << 20) {
            return Err(NikError::Corrupt("Archive is damaged (bad chunk length).".into()));
        }
        copy_exact(r, w, len as u64, buf)?;
        check_cancel()?;
    }
}

/// Remove entries from an archive. A folder that is deleted takes its
/// contents with it. The archive is solid, so the kept entries are
/// compressed again (they are copied exactly as stored: media and Deflate
/// recompression are kept) into a temporary file that replaces the archive
/// only when everything went well. Encrypted archives keep their password.
/// Returns the number of entries deleted.
pub fn delete(o: &Options, pick: Option<&Pick>) -> Result<u64> {
    if pick.is_none() && o.files.is_empty() {
        return Err(NikError::Usage("Name the files or folders to delete, e.g.  nikpak d archive.nik old\\*.log".into()));
    }
    let archive = PathBuf::from(&o.archive);
    let mut op = open_archive(o)?;

    // folders chosen in the window take their contents with them
    let dir_prefixes: Vec<String> = pick
        .map(|p| p.names.iter().map(|n| format!("{}/", n.trim_end_matches('/'))).collect())
        .unwrap_or_default();
    let doomed = |name: &str| match pick {
        Some(p) => p.wants(name) || dir_prefixes.iter().any(|d| name.starts_with(d.as_str())),
        None => passes_filters(name, o),
    };

    // ---- keep the shape of the archive being rewritten -------------------
    // A split archive stays split, and a self-extracting one keeps its .exe
    // stub (the bytes in front of the archive), so neither is lost by a delete.
    let hidden = split::embedded(&archive);
    let split_size = o.split.or_else(|| {
        let first = split::volume_name(&archive, 1);
        let len = match hidden {
            Some(e) => e.len,
            None => fs::metadata(&first).map(|m| m.len()).unwrap_or(0),
        };
        (split::is_split(&archive) && len > 0).then_some(len)
    });
    let stub = hidden.map(|e| (archive.clone(), e.offset));

    // ---- output: same method and password, new salt and nonce
    let header = Header {
        encrypted: op.encrypted,
        method: op.method,
        kdf_mem: KDF_MEM_KIB,
        kdf_time: KDF_TIME,
        kdf_lanes: KDF_LANES,
        salt: crypto::random_bytes::<SALT_LEN>(),
        nonce_prefix: crypto::random_bytes::<NONCE_PREFIX_LEN>(),
        // the legal statement travels with the archive: a delete never drops it
        legal: op.legal.clone(),
        cover: op.cover.clone(),
        split: split_size,
    };
    let header_bytes = header.to_bytes();
    let out = Out::create(&archive, split_size, stub)
        .map_err(|e| NikError::Fatal(format!("Cannot write '{}': {e}", archive.display())))?;
    let mut bw = BufWriter::with_capacity(IO_BUF, CountedW(out));
    bw.write_all(&header_bytes).map_err(|e| from_io(e, "Write error"))?;
    let sink = match &op.password {
        Some(pw) => {
            let key = crypto::derive_key(pw, &header.salt, header.kdf_mem, header.kdf_time, header.kdf_lanes)
                .map_err(NikError::Fatal)?;
            Sink::Enc(EncryptWriter::new(bw, &key, &header.nonce_prefix, header_bytes.clone()))
        }
        None => Sink::Plain(bw),
    };
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as u32;
    let mut comp = match op.method {
        METHOD_STORE => Compressor::Store(sink),
        METHOD_BLOCKS => Compressor::Blocks(BlockWriter::new(sink)),
        _ => {
            // the level is not stored in the archive: use -m4 (zstd 15, long matching)
            let mut enc = zstd::stream::write::Encoder::new(sink, 15).map_err(|e| from_io(e, "Cannot start compressor"))?;
            if threads > 1 {
                let _ = enc.multithread(threads);
            }
            let _ = enc.include_checksum(true);
            let _ = enc.window_log(27);
            let _ = enc.long_distance_matching(true);
            Compressor::Zstd(enc)
        }
    };

    let mut buf = vec![0u8; IO_BUF];
    let (mut kept, mut deleted) = (0u64, 0u64);
    let mut written_block = 0u32;
    loop {
        check_cancel()?;
        let rec = match next_record(&mut op.inner)? {
            Next::End(_) => break,
            Next::Rec(r) => r,
        };
        if doomed(&rec.name) {
            deleted += 1;
            if !o.quiet {
                outln!("Deleting  {}{}", rec.name, if rec.is_dir { "/" } else { "" });
            }
            if !rec.is_dir {
                skip_entry(&mut op.inner, &rec, &mut buf)?;
            }
            continue;
        }
        // -m5: keep the entry in a block made with the same engine as before
        if let (Inner::Blocks(r), Compressor::Blocks(w)) = (&op.inner, &mut comp) {
            if r.blocks != written_block {
                written_block = r.blocks;
                let e = r.engine.unwrap_or(Engine::lzma2_for(1 << 20, false));
                w.start(e, threads).map_err(|e| from_io(e, "Cannot start compressor"))?;
            }
        }
        let kind = if rec.is_dir { REC_DIR } else if rec.media.is_some() { REC_MEDIA } else { REC_FILE };
        write_rec_header(&mut comp, kind, rec.name.as_bytes(), rec.mtime, rec.mode, (!rec.is_dir).then_some(rec.size))
            .map_err(|e| from_io(e, "Write error"))?;
        kept += 1;
        if rec.is_dir {
            continue;
        }
        match &rec.media {
            Some(m) => {
                comp.write_all(&[m.filter, m.method, m.params.len() as u8])
                    .and_then(|_| comp.write_all(&m.params))
                    .map_err(|e| from_io(e, "Write error"))?;
                copy_chunks(&mut op.inner, &mut comp, &mut buf)?;
                let crc = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                comp.write_all(&crc.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
            }
            None => {
                let c = copy_exact(&mut op.inner, &mut comp, rec.size, &mut buf)?;
                let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                if stored != c {
                    return Err(NikError::Corrupt(format!("CRC mismatch in '{}': the archive is damaged. Nothing was deleted.", rec.name)));
                }
                comp.write_all(&c.to_le_bytes()).map_err(|e| from_io(e, "Write error"))?;
            }
        }
    }
    verify_end(&mut op.inner)?;
    drop(op); // close the archive before replacing it
    if deleted == 0 {
        return Err(NikError::NoFiles("No entries in the archive match the given names; nothing was deleted.".into()));
    }
    if let Compressor::Blocks(w) = &mut comp {
        if written_block == 0 {
            // every entry was deleted: an empty block holds the end marker
            w.start(Engine::lzma2_for(1 << 20, false), 1).map_err(|e| from_io(e, "Cannot start compressor"))?;
        }
    }
    comp.write_all(&[REC_END]).and_then(|_| comp.write_all(&kept.to_le_bytes())).map_err(|e| from_io(e, "Write error"))?;
    let out = comp.finish().map_err(|e| from_io(e, "Cannot finish archive"))?;
    // fs::rename replaces the old archive (MoveFileEx with REPLACE_EXISTING on Windows)
    let _ = out.finish().map_err(|e| NikError::Fatal(format!("Cannot replace '{}': {e}", archive.display())))?;
    if !o.quiet {
        outln!();
        outln!("Done. {} entries deleted, {} kept, archive is now {}.", deleted, kept, human_size(fs::metadata(&archive).map(|m| m.len()).unwrap_or(0)));
    }
    Ok(deleted)
}

// ===========================================================================
// For the window (unNik): header, index, one entry
// ===========================================================================

/// One entry of an archive, as shown by the browser.
#[derive(Clone, Debug)]
pub struct Entry {
    /// stored path, '/' separated, without a trailing '/'
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
    pub mode: u32,
    pub crc: u32,
    /// how the data is stored: "" (plain), or a media / Deflate filter name
    pub filter: String,
}

/// Everything the browser needs to know about an archive.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub entries: Vec<Entry>,
    pub encrypted: bool,
    /// METHOD_STORE / METHOD_ZSTD / METHOD_BLOCKS
    pub method: u8,
    /// the engines of the -m5 blocks ("LZMA2, 64 MiB dictionary" ...)
    pub engines: Vec<String>,
    pub archive_size: u64,
    /// the legal statement the archive carries (accepted before it was read)
    pub legal: Option<String>,
    /// the cover picture (JPEG or PNG bytes)
    pub cover: Option<Vec<u8>>,
}

/// Options for the window's operations (never asks anything on a console).
pub fn gui_options(archive: &Path, password: Option<&str>) -> Options {
    Options {
        command: Command::ExtractFull,
        archive: archive.display().to_string(),
        files: Vec::new(),
        // an empty given password fails cleanly instead of prompting
        password: crate::util::PwdSpec::Given(password.unwrap_or("").to_string()),
        level: Level::Max,
        ep: EpMode::Keep,
        overwrite: Overwrite::Rename,
        quiet: true,
        dest: None,
        excludes: Vec::new(),
        threads: 0,
        media: MediaMode::Auto,
        legal: None,
        cover: None,
        // a window shows the statement itself and calls accept_legal
        accept_legal: false,
        // the archive keeps whatever split it has (see delete)
        split: None,
        sfx: false,
    }
}

/// Is this a .nik archive, and is it encrypted? Reads only the header.
///
/// **A Nikkosynth soundbank is named as such.** Both products write `.nik`, so a
/// player who exports a bank and then opens it here used to get "is not a .nik
/// archive (bad signature)" — true, and no help at all. See `soundbank.rs`.
pub fn peek(path: &Path) -> Result<bool> {
    let mut f = open_bytes(path)?;
    let mut fixed = [0u8; 12];
    f.read_exact(&mut fixed)
        .map_err(|_| NikError::Corrupt(format!("'{}' is not a valid .nik archive (file too short).", path.display())))?;
    if fixed[..8] != MAGIC {
        if fixed[..8] == *crate::soundbank::MAGIC {
            let info = crate::soundbank::read_info(path).unwrap_or_default();
            return Err(NikError::Fatal(crate::soundbank::describe(&info)));
        }
        if fixed[..8] == *crate::package::MAGIC {
            let info = crate::package::read_info(path).unwrap_or_default();
            return Err(NikError::Fatal(crate::package::describe(&info)));
        }
        return Err(NikError::Corrupt(format!("'{}' is not a .nik archive.", path.display())));
    }
    Ok(fixed[9] & FLAG_ENCRYPTED != 0)
}

/// Read the whole archive once (it is solid) and return its entries.
/// Data is checked as it goes by; PROGRESS shows how far the reading is.
pub fn read_index(path: &Path, password: Option<&str>) -> Result<Index> {
    let o = gui_options(path, password);
    let mut op = open_archive(&o)?;
    let mut buf = vec![0u8; IO_BUF];
    let mut idx = Index {
        encrypted: op.encrypted,
        method: op.method,
        archive_size: PROGRESS.total.load(Relaxed),
        legal: op.legal.clone(),
        cover: op.cover.clone(),
        ..Default::default()
    };
    let mut last_block = 0u32;
    loop {
        check_cancel()?;
        let r = match next_record(&mut op.inner)? {
            Next::End(_) => break,
            Next::Rec(r) => r,
        };
        if let Inner::Blocks(b) = &op.inner {
            if b.blocks != last_block {
                last_block = b.blocks;
                if let Some(e) = b.engine {
                    idx.engines.push(e.name());
                }
            }
        }
        let crc = if r.is_dir {
            0
        } else if r.media.is_some() {
            media::ChunkReader::new(&mut op.inner).drain().map_err(|e| from_io(e, "Reading archive"))?;
            read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?
        } else {
            let c = copy_exact(&mut op.inner, &mut io::sink(), r.size, &mut buf)?;
            let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
            if stored != c {
                return Err(NikError::Corrupt(format!("CRC mismatch in '{}': the archive is damaged.", r.name)));
            }
            stored
        };
        idx.entries.push(Entry {
            name: r.name.trim_end_matches('/').to_string(),
            is_dir: r.is_dir,
            size: r.size,
            mtime: r.mtime,
            mode: r.mode,
            crc,
            filter: r.media.as_ref().map(|m| media::method_name(m.filter, m.method).to_string()).unwrap_or_default(),
        });
    }
    verify_end(&mut op.inner)?;
    Ok(idx)
}

/// The first `limit` bytes of one file (the viewer). Reading stops right
/// after that entry; the CRC is checked when the whole file was read.
/// Returns the bytes and whether the file is longer than `limit`.
pub fn read_entry(path: &Path, password: Option<&str>, name: &str, limit: usize) -> Result<(Vec<u8>, bool)> {
    struct Head {
        data: Vec<u8>,
        limit: usize,
        more: bool,
    }
    impl Write for Head {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            let room = self.limit - self.data.len();
            if b.len() > room {
                self.more = true;
            }
            self.data.extend_from_slice(&b[..b.len().min(room)]);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let o = gui_options(path, password);
    let mut op = open_archive(&o)?;
    let mut buf = vec![0u8; IO_BUF];
    loop {
        check_cancel()?;
        match next_record(&mut op.inner)? {
            Next::End(_) => return Err(NikError::NoFiles(format!("'{name}' is not in the archive."))),
            Next::Rec(r) if r.is_dir => {}
            Next::Rec(r) if r.name.trim_end_matches('/') == name => {
                let mut h = Head { data: Vec::new(), limit, more: false };
                let c = entry_data(&mut op.inner, &r, &mut h, &mut buf)?;
                let stored = read_u32(&mut op.inner).map_err(|e| from_io(e, "Reading archive"))?;
                if stored != c {
                    return Err(NikError::Corrupt(format!("CRC mismatch in '{}': the archive is damaged.", r.name)));
                }
                return Ok((h.data, h.more));
            }
            Next::Rec(r) => skip_entry(&mut op.inner, &r, &mut buf)?,
        }
    }
}

// ===========================================================================
// MEDIA TEST  (nikpak mt <files>)
// ===========================================================================

pub fn media_test(o: &Options) -> Result<u8> {
    let lvl = match o.level {
        Level::Store => 0,
        l => media::nac_level(l.effort()),
    };
    let mut warnings = 0u32;
    let (mut tot_raw, mut tot_best, mut tot_files) = (0u64, 0u64, 0u32);
    outln!("{:<28} {:<5} {:>11} {:>11}  {:<28} {}", "File", "Type", "Samples", "zstd-3", "Filters (size)", "Best");
    outln!("{}", "-".repeat(110));
    for name in &o.files {
        let mut f = match File::open(name) {
            Ok(f) => f,
            Err(e) => {
                warnings += 1;
                eprintln!("WARNING: cannot open '{name}': {e}");
                continue;
            }
        };
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        let short: String = if name.chars().count() > 28 {
            format!("...{}", name.chars().rev().take(25).collect::<Vec<_>>().into_iter().rev().collect::<String>())
        } else {
            name.clone()
        };
        let Some(det) = media::detect(&mut f, len) else {
            outln!("{:<28} {:<5} {:>11}  not a supported uncompressed media file (WAV/AIFF/MOD/BMP)", short, "-", len);
            continue;
        };
        let ev = match media::evaluate(&mut f, &det, lvl, true) {
            Ok(e) => e,
            Err(e) => {
                warnings += 1;
                eprintln!("WARNING: '{name}': {e}");
                continue;
            }
        };
        let fid = det.filter_id();
        let list: Vec<String> = ev.cands.iter().map(|(m, s)| format!("{} {}", media::method_name(fid, *m), s)).collect();
        let best = ev.cands.iter().min_by_key(|c| c.1).copied();
        let verdict = match best {
            Some((m, s)) if (s as f64) < ev.raw as f64 * 0.98 => {
                tot_best += s;
                format!("{}  ({:.1}% of zstd-3)", media::method_name(fid, m), s as f64 * 100.0 / ev.raw.max(1) as f64)
            }
            _ => {
                tot_best += ev.raw;
                "none (plain compression)".to_string()
            }
        };
        tot_raw += ev.raw;
        tot_files += 1;
        outln!("{:<28} {:<5} {:>11} {:>11}  {:<28} {}", short, det.kind.name(), ev.sample, ev.raw, list.join(", "), verdict);
    }
    if tot_files > 0 {
        outln!("{}", "-".repeat(110));
        outln!(
            "{} media file(s): plain zstd-3 {} bytes -> best filters {} bytes ({:.1}%)",
            tot_files, tot_raw, tot_best, tot_best as f64 * 100.0 / tot_raw.max(1) as f64
        );
    }
    Ok(if warnings > 0 { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::PwdSpec;

    fn make(dir: &Path, level: Level, password: Option<&str>) -> PathBuf {
        let src = dir.join("src");
        fs::create_dir_all(src.join("docs/sub")).unwrap();
        fs::write(src.join("docs/a.txt"), "alpha\n".repeat(400)).unwrap();
        fs::write(src.join("docs/sub/b.txt"), "beta\n".repeat(300)).unwrap();
        fs::write(src.join("bin.dat"), (0..5000u32).map(|i| (i * 7 % 251) as u8).collect::<Vec<_>>()).unwrap();
        let arc = dir.join("t.nik");
        let mut o = gui_options(&arc, password);
        o.command = Command::Add;
        o.level = level;
        o.overwrite = Overwrite::All;
        o.password = password.map_or(PwdSpec::None, |p| PwdSpec::Given(p.into()));
        o.files = vec![src.join("docs").display().to_string(), src.join("bin.dat").display().to_string()];
        o.ep = EpMode::Names;
        create(&o).unwrap();
        arc
    }

    #[test]
    fn index_pick_extract_and_delete() {
        for (k, (level, pw)) in [(Level::Max, None), (Level::Zstd(3), Some("password123")), (Level::Store, None)].into_iter().enumerate() {
            let dir = std::env::temp_dir().join(format!("nikpak-api-test-{}-{k}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            let arc = make(&dir, level, pw);
            assert_eq!(peek(&arc).unwrap(), pw.is_some());

            let idx = read_index(&arc, pw).unwrap();
            let names: Vec<&str> = idx.entries.iter().map(|e| e.name.as_str()).collect();
            for n in ["docs", "docs/a.txt", "docs/sub", "docs/sub/b.txt", "bin.dat"] {
                assert!(names.contains(&n), "{n} missing from {names:?}");
            }

            let (head, more) = read_entry(&arc, pw, "docs/sub/b.txt", 10).unwrap();
            assert_eq!(head, b"beta\nbeta\n");
            assert!(more);

            // extract one file from inside docs/: lands without the docs/ prefix
            let out = dir.join("out");
            let mut o = gui_options(&arc, pw);
            o.dest = Some(out.clone());
            let pick = Pick { names: ["docs/sub/b.txt".to_string()].into_iter().collect(), strip: "docs/".into() };
            extract_pick(&o, Some(&pick)).unwrap();
            assert_eq!(fs::read_to_string(out.join("sub/b.txt")).unwrap(), "beta\n".repeat(300));
            assert!(!out.join("a.txt").exists());

            // delete a folder (with its contents) and keep the rest
            let pick = Pick { names: ["docs/sub".to_string()].into_iter().collect(), strip: String::new() };
            assert_eq!(delete(&o, Some(&pick)).unwrap(), 2);
            let idx = read_index(&arc, pw).unwrap();
            let names: Vec<&str> = idx.entries.iter().map(|e| e.name.as_str()).collect();
            assert_eq!(names.len(), 3, "{names:?}");
            assert!(!names.iter().any(|n| n.starts_with("docs/sub")));
            let (a, _) = read_entry(&arc, pw, "docs/a.txt", 1 << 20).unwrap();
            assert_eq!(a, "alpha\n".repeat(400).as_bytes());

            // deleting everything leaves a valid, empty archive
            let pick = Pick { names: names.iter().map(|s| s.to_string()).collect(), strip: String::new() };
            delete(&o, Some(&pick)).unwrap();
            assert!(read_index(&arc, pw).unwrap().entries.is_empty());
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_legal_statement_must_be_accepted_and_cannot_be_changed() {
        const TERMS: &str = "These sounds are licensed to one user.\nNo resale.";
        for pw in [None, Some("password123")] {
            let dir = std::env::temp_dir().join(format!("nikpak-legal-{}-{}", std::process::id(), pw.is_some()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("src")).unwrap();
            fs::write(dir.join("src/a.txt"), "alpha\n".repeat(100)).unwrap();
            let arc = dir.join("t.nik");
            let mut o = gui_options(&arc, pw);
            o.command = Command::Add;
            o.level = Level::Zstd(3);
            o.overwrite = Overwrite::All;
            o.password = pw.map_or(PwdSpec::None, |p| PwdSpec::Given(p.into()));
            o.files = vec![dir.join("src/a.txt").display().to_string()];
            o.ep = EpMode::Names;
            o.legal = Some(format!("  {TERMS}\r\n"));
            create(&o).unwrap();

            // readable without the password, trimmed
            assert_eq!(legal_statement(&arc).unwrap().as_deref(), Some(TERMS));
            assert_eq!(peek(&arc).unwrap(), pw.is_some());
            // refused until accepted, by every path
            assert!(matches!(read_index(&arc, pw), Err(NikError::Legal(_))));
            assert!(matches!(read_entry(&arc, pw, "a.txt", 10), Err(NikError::Legal(_))));
            let mut x = gui_options(&arc, pw);
            x.dest = Some(dir.join("out"));
            assert!(matches!(extract(&x), Err(NikError::Legal(_))));
            assert!(!dir.join("out/a.txt").exists());
            // -accept lets one operation through; accept_legal lets them all
            x.accept_legal = true;
            extract(&x).unwrap();
            assert!(dir.join("out/a.txt").exists());
            accept_legal(&arc, TERMS);
            let idx = read_index(&arc, pw).unwrap();
            assert_eq!(idx.legal.as_deref(), Some(TERMS));
            assert_eq!(idx.entries.len(), 1);

            // a delete keeps the statement
            let pick = Pick { names: ["a.txt".to_string()].into_iter().collect(), strip: String::new() };
            delete(&gui_options(&arc, pw), Some(&pick)).unwrap();
            assert_eq!(legal_statement(&arc).unwrap().as_deref(), Some(TERMS));

            // changing the statement on disk: asked again; and, encrypted, the archive no longer opens
            let mut bytes = fs::read(&arc).unwrap();
            let at = bytes.windows(4).position(|w| w == b"No r").unwrap();
            bytes[at] = b'n';
            fs::write(&arc, &bytes).unwrap();
            let changed = legal_statement(&arc).unwrap().unwrap();
            assert!(matches!(read_index(&arc, pw), Err(NikError::Legal(_))));
            accept_legal(&arc, &changed);
            let r = read_index(&arc, pw);
            if pw.is_some() {
                assert!(r.is_err(), "a changed statement must break an encrypted archive");
            } else {
                r.unwrap();
            }
            let _ = fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn a_cover_travels_with_the_archive_and_is_authenticated() {
        let dir = std::env::temp_dir().join(format!("nikpak-cover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "cover test").unwrap();
        let img = image::RgbImage::from_fn(640, 480, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 40]));
        let mut png = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let pic = crate::cover::from_picture(png.get_ref()).unwrap();
        for pw in [None, Some("password123")] {
            let arc = dir.join(format!("c{}.nik", pw.is_some()));
            let mut o = gui_options(&arc, pw);
            o.command = Command::Add;
            o.level = Level::Zstd(3);
            o.overwrite = Overwrite::All;
            o.password = pw.map_or(PwdSpec::None, |p| PwdSpec::Given(p.into()));
            o.files = vec![dir.join("a.txt").display().to_string()];
            o.ep = EpMode::Names;
            o.cover = Some(pic.clone());
            o.legal = Some("Terms.".into());
            create(&o).unwrap();
            // read without the password and before the terms are accepted
            assert_eq!(cover_image(&arc).unwrap().as_deref(), Some(pic.as_slice()));
            assert_eq!(legal_statement(&arc).unwrap().as_deref(), Some("Terms."));
            accept_legal(&arc, "Terms.");
            let idx = read_index(&arc, pw).unwrap();
            assert_eq!(idx.cover.as_deref(), Some(pic.as_slice()));
            // a delete keeps it
            let pick = Pick { names: ["a.txt".to_string()].into_iter().collect(), strip: String::new() };
            delete(&gui_options(&arc, pw), Some(&pick)).unwrap();
            assert_eq!(cover_image(&arc).unwrap().as_deref(), Some(pic.as_slice()));
            // an encrypted archive whose cover was swapped no longer opens
            if pw.is_some() {
                let mut bytes = fs::read(&arc).unwrap();
                let at = bytes.windows(pic.len().min(64)).position(|w| w == &pic[..pic.len().min(64)]).unwrap();
                bytes[at + 40] ^= 0x55;
                fs::write(&arc, &bytes).unwrap();
                assert!(read_index(&arc, pw).is_err());
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_statement_is_not_attached() {
        let dir = std::env::temp_dir().join(format!("nikpak-legal-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), "x").unwrap();
        let arc = dir.join("t.nik");
        let mut o = gui_options(&arc, None);
        o.command = Command::Add;
        o.overwrite = Overwrite::All;
        o.password = PwdSpec::None;
        o.files = vec![dir.join("a.txt").display().to_string()];
        o.legal = Some("   \n ".into());
        create(&o).unwrap();
        assert_eq!(legal_statement(&arc).unwrap(), None);
        // and the header is the plain version-3 one, readable by older nikpak
        assert_eq!(fs::read(&arc).unwrap()[9], 0);
        read_index(&arc, None).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrong_password_is_reported() {
        let dir = std::env::temp_dir().join(format!("nikpak-api-pw-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let arc = make(&dir, Level::Zstd(1), Some("right-password"));
        assert!(matches!(read_index(&arc, Some("wrong-password")), Err(NikError::Password(_))));
        let _ = fs::remove_dir_all(&dir);
    }

    /// A split archive is written as a set of volumes and read back from its
    /// first volume alone (split.rs: Out, Chain).
    #[test]
    fn a_split_archive_is_read_and_extracted_from_its_first_volume() {
        let dir = std::env::temp_dir().join(format!("nikpak-api-split-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        let text = "alpha\n".repeat(4000);
        fs::write(src.join("a.txt"), &text).unwrap();
        fs::write(src.join("b.bin"), vec![7u8; 200_000]).unwrap();
        let arc = dir.join("t.nik");
        let mut o = gui_options(&arc, None);
        o.command = Command::Add;
        o.overwrite = Overwrite::All;
        // stored, so the parser reads the chain itself and the error below is
        // the one the reader makes (a compressor would wrap it)
        o.level = Level::Store;
        o.files = vec![src.display().to_string()];
        o.ep = EpMode::Names;
        o.split = Some(crate::split::MIN_VOLUME);
        create(&o).unwrap();
        assert!(crate::split::is_split(&arc), "a .n01 should sit beside the archive");
        assert!(crate::split::volume_count(&arc) > 1);

        let idx = read_index(&arc, None).unwrap();
        assert!(idx.entries.iter().any(|e| e.name == "src/a.txt"));
        assert_eq!(idx.archive_size, crate::split::total_size(&arc));

        let out = dir.join("out");
        let mut x = gui_options(&arc, None);
        x.dest = Some(out.clone());
        extract(&x).unwrap();
        // the original files come out, not a copy of the archive: a split set
        // is read as the one stream its volumes hold (and the files span the
        // volume boundary - b.bin is bigger than the 64 KiB volume here)
        assert_eq!(fs::read(out.join("src/a.txt")).unwrap(), text.into_bytes());
        assert_eq!(fs::read(out.join("src/b.bin")).unwrap(), vec![7u8; 200_000]);
        let niks = fs::read_dir(out.join("src"))
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("nik")))
            .count();
        assert_eq!(niks, 0, "the archive itself must not be extracted");

        // a volume that is missing says so, instead of "unexpected end of file"
        let missing = crate::split::volume_name(&arc, 2);
        fs::remove_file(&missing).unwrap();
        let e = read_index(&arc, None).unwrap_err().to_string();
        assert!(e.contains("missing"), "{e}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// The -m5 body cut into several blocks, compressed at the same time, must
    /// read back exactly like the one-block-at-a-time version (plan_blocks,
    /// write_body_blocks).
    #[test]
    fn blocks_are_split_balanced_and_read_back() {
        // the splitter alone: no empty chunk, order kept, sizes balanced
        let sizes = [10u64, 9, 8, 7, 6, 5, 4, 3, 2, 1];
        let list: Vec<usize> = (0..sizes.len()).collect();
        let parts = split_by_bytes(&list, &sizes, 3);
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| !p.is_empty()), "{parts:?}");
        // every entry exactly once, in the original order
        assert_eq!(parts.concat(), list);
        let total: u64 = sizes.iter().sum();
        for p in &parts {
            let bytes: u64 = p.iter().map(|&i| sizes[i]).sum();
            assert!(bytes * 2 >= total / 3, "chunk of {bytes} of {total} is starved: {parts:?}");
        }
        // more chunks than entries is refused rather than making empty ones
        assert_eq!(split_by_bytes(&[0usize], &sizes, 8).len(), 1);
        assert!(split_by_bytes(&[], &sizes, 4).is_empty());
        // and the block count follows the bytes, not the thread count alone
        assert_eq!(block_count(1 << 20, 100, 8), 1);
        assert_eq!(block_count(40 << 20, 100, 8), 5);
        assert_eq!(block_count(40 << 20, 2, 8), 2);
    }

    /// A real archive over several blocks: written with the parallel block
    /// path, read back entry by entry, and compared with the same archive made
    /// one block at a time.
    #[test]
    fn a_multi_block_archive_round_trips() {
        let dir = std::env::temp_dir().join(format!("nikpak-blocks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        // easy data (LZMA2 flies through it) but well over MIN_BLOCK_BYTES, so
        // the binary part is cut into several blocks; a text file makes the
        // text block (kept small: PPMd and the engine trial are slow)
        let binary: Vec<u8> = (0..17_000_000u32).map(|i| (i / 64 % 251) as u8).collect();
        fs::write(src.join("big.bin"), &binary).unwrap();
        let text = "the quick brown fox jumps over the lazy dog\n".repeat(12_000);
        fs::write(src.join("notes.txt"), &text).unwrap();
        for k in 0..6 {
            fs::write(src.join(format!("small{k}.bin")), vec![k as u8; 200_000]).unwrap();
        }

        let make = |arc: &Path, threads: usize| {
            let mut o = gui_options(arc, None);
            o.command = Command::Add;
            o.overwrite = Overwrite::All;
            o.level = Level::Max;
            o.threads = threads;
            o.files = vec![src.display().to_string()];
            o.ep = EpMode::Names;
            create(&o).unwrap();
        };
        let seq = dir.join("seq.nik");
        let par = dir.join("par.nik");
        make(&seq, 1);
        make(&par, 4);

        let idx = read_index(&par, None).unwrap();
        let names: Vec<&str> = idx.entries.iter().map(|e| e.name.as_str()).collect();
        for n in ["src", "src/big.bin", "src/notes.txt", "src/small0.bin", "src/small5.bin"] {
            assert!(names.contains(&n), "{n} missing from {names:?}");
        }
        assert!(!idx.engines.is_empty(), "the index should name the block engines");

        // both archives hold exactly the same files, byte for byte
        let out = dir.join("out");
        for (k, arc) in [seq, par].into_iter().enumerate() {
            let d = out.join(k.to_string());
            let mut x = gui_options(&arc, None);
            x.dest = Some(d.clone());
            extract(&x).unwrap();
            assert_eq!(fs::read(d.join("src/big.bin")).unwrap(), binary, "block {k}");
            assert_eq!(fs::read(d.join("src/notes.txt")).unwrap(), text.clone().into_bytes());
            assert_eq!(fs::read(d.join("src/small3.bin")).unwrap(), vec![3u8; 200_000]);
        }
        // no holding area is left behind
        let leftovers: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".part") || n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    /// The preparation pipeline (zstd levels): entries are read and filtered on
    /// several threads and written in archive order, so the result must be the
    /// same archive as the one-at-a-time path.
    #[test]
    fn the_preparation_pipeline_keeps_the_archive_order() {
        let dir = std::env::temp_dir().join(format!("nikpak-pipe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        for k in 0..12 {
            fs::write(src.join(format!("f{k:02}.txt")), format!("file {k}\n").repeat(1000 + k)).unwrap();
        }
        let make = |arc: &Path, threads: usize| {
            let mut o = gui_options(arc, None);
            o.command = Command::Add;
            o.overwrite = Overwrite::All;
            o.level = Level::Zstd(3);
            o.threads = threads;
            o.files = vec![src.display().to_string()];
            o.ep = EpMode::Names;
            create(&o).unwrap();
            read_index(arc, None).unwrap()
        };
        let a = make(&dir.join("one.nik"), 1);
        let b = make(&dir.join("many.nik"), 4);
        let names = |i: &Index| i.entries.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&a), names(&b), "the pipeline must not reorder entries");
        let sizes = |i: &Index| i.entries.iter().map(|e| e.size).collect::<Vec<_>>();
        assert_eq!(sizes(&a), sizes(&b));
        let _ = fs::remove_dir_all(&dir);
    }
}
