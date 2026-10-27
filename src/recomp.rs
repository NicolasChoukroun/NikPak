//! Deflate recompression ("precompression") for files that are themselves
//! Deflate-compressed containers: ZIP-based formats (docx, xlsx, pptx, odt,
//! ods, epub, jar, zip ...), PDF (FlateDecode streams) and gzip.
//!
//! Such files are nearly incompressible for any archiver because their content
//! is already Deflate-coded. nikpak finds every Deflate stream inside the file,
//! decodes it to its plain text and records - with preflate - the small amount
//! of extra information needed to re-create the ORIGINAL Deflate bit-stream
//! exactly. The plain text (XML, PDF page descriptions ...) then goes into the
//! archive's solid stream, where LZMA2/zstd compress it far better than
//! Deflate did. On extraction the Deflate streams are rebuilt bit for bit, so
//! the extracted file is byte-identical (and the CRC32 of the original bytes
//! is checked as for every entry).
//!
//! Transformed stream (inside the media record's chunk framing):
//!
//! ```text
//! segment := 0x00 len:u64 bytes[len]                      raw bytes, copied as is
//!          | 0x01 plain_len:u64 corr_len:u32 plain corr    one Deflate stream
//!          | 0xFF                                          end of file
//! ```

use preflate_rs::{preflate_whole_deflate_stream, recreate_whole_deflate_stream, PreflateConfig};
use std::cell::Cell;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::panic::{self, AssertUnwindSafe};

thread_local! {
    static IN_PREFLATE: Cell<bool> = const { Cell::new(false) };
}

/// preflate-rs can panic (e.g. index out of bounds) on malformed input instead
/// of returning an error. Run it under catch_unwind so that a damaged archive
/// or a hostile input file gives a clean error / fallback, never a crash.
fn guarded<T>(f: impl FnOnce() -> T) -> Option<T> {
    IN_PREFLATE.with(|c| c.set(true));
    let r = panic::catch_unwind(AssertUnwindSafe(f));
    IN_PREFLATE.with(|c| c.set(false));
    r.ok()
}

/// Silence the default panic message for panics caught by `guarded`; all
/// other panics are reported as usual. Call once at program start.
pub fn install_panic_hook() {
    let default = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if !IN_PREFLATE.with(|c| c.get()) {
            default(info);
        }
    }));
}

pub const FILTER_DEFLATE: u8 = 3;
/// Method 2: preflate corrections stored with nikpak's own range coder (1.4+).
/// Method 1 was used by the 1.2.x-1.3.x test builds (LGPL coder, no longer included).
pub const M_PREFLATE: u8 = 2;
pub const M_PREFLATE_V1: u8 = 1;

/// Files below this size are not worth the analysis.
pub const MIN_SIZE: u64 = 1024;
/// The whole file is held in memory while it is analysed.
pub const MAX_FILE: u64 = 512 << 20;
/// Limits that protect the decoder against hostile archives.
const MAX_PLAIN: u64 = 512 << 20;
const MAX_CORR: u32 = 64 << 20;
/// Streams smaller than this are left as they are.
const MIN_STREAM: usize = 64;

const SEG_RAW: u8 = 0x00;
const SEG_DEFLATE: u8 = 0x01;
const SEG_END: u8 = 0xFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Zip,
    Pdf,
    Gzip,
}

impl Container {
    pub fn name(&self) -> &'static str {
        match self {
            Container::Zip => "ZIP",
            Container::Pdf => "PDF",
            Container::Gzip => "GZIP",
        }
    }
}

/// Recognise a container by its content (never by the file extension).
pub fn detect(f: &mut File, len: u64) -> Option<Container> {
    if !(MIN_SIZE..=MAX_FILE).contains(&len) {
        return None;
    }
    let mut head = [0u8; 8];
    f.seek(SeekFrom::Start(0)).ok()?;
    f.read_exact(&mut head).ok()?;
    f.seek(SeekFrom::Start(0)).ok()?;
    detect_bytes(&head)
}

pub fn detect_bytes(head: &[u8]) -> Option<Container> {
    if head.len() >= 4 && &head[..4] == b"PK\x03\x04" {
        Some(Container::Zip)
    } else if head.len() >= 5 && &head[..5] == b"%PDF-" {
        Some(Container::Pdf)
    } else if head.len() >= 3 && head[0] == 0x1f && head[1] == 0x8b && head[2] == 8 {
        Some(Container::Gzip)
    } else {
        None
    }
}

fn u16le(b: &[u8], o: usize) -> usize {
    b[o] as usize | (b[o + 1] as usize) << 8
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

/// Offsets where a raw Deflate stream may begin, in increasing order.
fn candidates(d: &[u8], c: Container) -> Vec<usize> {
    let mut v = Vec::new();
    match c {
        Container::Zip => {
            // Every local file header that announces method 8 (Deflate).
            let mut i = 0;
            while i + 30 <= d.len() {
                if &d[i..i + 4] == b"PK\x03\x04" {
                    let method = u16le(d, i + 8);
                    let start = i + 30 + u16le(d, i + 26) + u16le(d, i + 28);
                    if method == 8 && start < d.len() {
                        v.push(start);
                    }
                    i += 4;
                } else {
                    i += 1;
                }
            }
        }
        Container::Pdf => {
            // "stream" EOL, then a zlib header (CM = 8, valid FCHECK).
            let mut i = 0;
            while let Some(p) = find(&d[i..], b"stream") {
                let mut t = i + p + 6;
                if t < d.len() && d[t] == b'\r' {
                    t += 1;
                }
                if t < d.len() && d[t] == b'\n' {
                    t += 1;
                }
                if t + 2 < d.len() && d[t] & 0x0f == 8 && d[t] >> 4 <= 7 && ((d[t] as u16) << 8 | d[t + 1] as u16) % 31 == 0 && d[t + 1] & 0x20 == 0 {
                    v.push(t + 2);
                }
                i = i + p + 6;
            }
        }
        Container::Gzip => {
            // Every member: fixed 10-byte header plus optional fields.
            let i = 0;
            if i + 18 <= d.len() && d[i] == 0x1f && d[i + 1] == 0x8b && d[i + 2] == 8 {
                let flg = d[i + 3];
                let mut p = i + 10;
                if flg & 4 != 0 && p + 2 <= d.len() {
                    p += 2 + u16le(d, p);
                }
                for bit in [8u8, 16] {
                    if flg & bit != 0 {
                        while p < d.len() && d[p] != 0 {
                            p += 1;
                        }
                        p += 1;
                    }
                }
                if flg & 2 != 0 {
                    p += 2;
                }
                // The next member can only be found after decoding this one;
                // encode() continues the scan from where the stream ended.
                if p < d.len() {
                    v.push(p);
                }
            }
        }
    }
    v
}

fn config() -> PreflateConfig {
    // The caller decodes the result again and compares the CRC (stronger than
    // preflate's own check, because it covers our segment framing too).
    PreflateConfig { max_chain_length: 4096, plain_text_limit: MAX_PLAIN as usize, verify_compression: false }
}

pub struct Stats {
    pub streams: u32,
    pub deflate_bytes: u64,
    pub plain_bytes: u64,
}

/// Transform a whole file. Returns None when no Deflate stream could be
/// recompressed (the caller then stores the file normally).
pub fn encode(d: &[u8], c: Container) -> Option<(Vec<u8>, Stats)> {
    let cfg = config();
    let mut out = Vec::with_capacity(d.len() * 4);
    let mut st = Stats { streams: 0, deflate_bytes: 0, plain_bytes: 0 };
    let mut pos = 0usize;
    let mut cands = candidates(d, c);
    let mut k = 0;
    while k < cands.len() {
        let s = cands[k];
        k += 1;
        if s < pos {
            continue;
        }
        let Some(Ok((r, plain))) = guarded(|| preflate_whole_deflate_stream(&d[s..], &cfg)) else { continue };
        let csz = r.compressed_size;
        let plain = plain.text();
        // Only worth it when the corrections are small compared with what the
        // Deflate stream occupied.
        if csz < MIN_STREAM || csz > d.len() - s || r.corrections.len() * 2 > csz || r.corrections.len() > MAX_CORR as usize {
            continue;
        }
        if s > pos {
            out.push(SEG_RAW);
            out.extend_from_slice(&((s - pos) as u64).to_le_bytes());
            out.extend_from_slice(&d[pos..s]);
        }
        out.push(SEG_DEFLATE);
        out.extend_from_slice(&(plain.len() as u64).to_le_bytes());
        out.extend_from_slice(&(r.corrections.len() as u32).to_le_bytes());
        out.extend_from_slice(plain);
        out.extend_from_slice(&r.corrections);
        st.streams += 1;
        st.deflate_bytes += csz as u64;
        st.plain_bytes += plain.len() as u64;
        pos = s + csz;
        // gzip: a further member may follow the 8-byte trailer
        if c == Container::Gzip && pos + 8 < d.len() {
            let next = candidates(&d[pos + 8..], Container::Gzip);
            cands.extend(next.into_iter().map(|o| o + pos + 8));
        }
    }
    if st.streams == 0 {
        return None;
    }
    if pos < d.len() {
        out.push(SEG_RAW);
        out.extend_from_slice(&((d.len() - pos) as u64).to_le_bytes());
        out.extend_from_slice(&d[pos..]);
    }
    out.push(SEG_END);
    Some((out, st))
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn read_u8<R: Read>(r: &mut R) -> io::Result<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b)?;
    Ok(b[0])
}

/// Rebuild the original file from the transformed stream. `size` is the
/// original file size from the record; more output than that is an error.
pub fn decode<R: Read, W: Write>(src: &mut R, size: u64, out: &mut W) -> io::Result<()> {
    let mut written = 0u64;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        match read_u8(src)? {
            SEG_END => break,
            SEG_RAW => {
                let mut b8 = [0u8; 8];
                src.read_exact(&mut b8)?;
                let mut n = u64::from_le_bytes(b8);
                if n > size - written {
                    return Err(bad("Deflate segment is longer than the file"));
                }
                written += n;
                while n > 0 {
                    let want = n.min(buf.len() as u64) as usize;
                    src.read_exact(&mut buf[..want])?;
                    out.write_all(&buf[..want])?;
                    n -= want as u64;
                }
            }
            SEG_DEFLATE => {
                let mut b8 = [0u8; 8];
                src.read_exact(&mut b8)?;
                let plen = u64::from_le_bytes(b8);
                let mut b4 = [0u8; 4];
                src.read_exact(&mut b4)?;
                let clen = u32::from_le_bytes(b4);
                if plen > MAX_PLAIN || clen > MAX_CORR {
                    return Err(bad("Deflate segment exceeds the limits"));
                }
                let mut plain = vec![0u8; plen as usize];
                src.read_exact(&mut plain)?;
                let mut corr = vec![0u8; clen as usize];
                src.read_exact(&mut corr)?;
                let deflate = match guarded(|| recreate_whole_deflate_stream(&plain, &corr)) {
                    Some(Ok(v)) => v,
                    Some(Err(e)) => return Err(bad(&format!("cannot rebuild a Deflate stream ({e:?})"))),
                    None => return Err(bad("cannot rebuild a Deflate stream (damaged data)")),
                };
                if deflate.len() as u64 > size - written {
                    return Err(bad("Deflate segment is longer than the file"));
                }
                written += deflate.len() as u64;
                out.write_all(&deflate)?;
            }
            _ => return Err(bad("unknown Deflate segment type")),
        }
    }
    if written != size {
        return Err(bad("Deflate data has the wrong length"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::DeflateEncoder, write::GzEncoder, write::ZlibEncoder, Compression};
    use std::io::Cursor;

    fn deflate(data: &[u8], level: u32) -> Vec<u8> {
        let mut e = DeflateEncoder::new(Vec::new(), Compression::new(level));
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn text(n: usize, seed: u64) -> Vec<u8> {
        let words = ["<row r=\"", "\"><c t=\"s\"><v>", "</v></c>", "sales", "region", "North", "South", "1234", "widget", "\n"];
        let mut s = seed;
        let mut v = Vec::new();
        while v.len() < n {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            v.extend_from_slice(words[(s % words.len() as u64) as usize].as_bytes());
        }
        v.truncate(n);
        v
    }

    /// Minimal ZIP: local headers + data (sizes need not be consistent for the filter).
    fn zip(entries: &[(&str, Vec<u8>, u16)]) -> Vec<u8> {
        let mut z = Vec::new();
        for (name, data, method) in entries {
            let payload = if *method == 8 { deflate(data, 6) } else { data.clone() };
            z.extend_from_slice(b"PK\x03\x04");
            z.extend_from_slice(&20u16.to_le_bytes());
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(&method.to_le_bytes());
            z.extend_from_slice(&[0u8; 16]);
            z.extend_from_slice(&(name.len() as u16).to_le_bytes());
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(name.as_bytes());
            z.extend_from_slice(&payload);
        }
        z.extend_from_slice(b"PK\x05\x06 central directory stand-in");
        z
    }

    fn roundtrip(d: &[u8], c: Container) -> Stats {
        let (t, st) = encode(d, c).expect("nothing recompressed");
        let mut out = Vec::new();
        decode(&mut Cursor::new(&t), d.len() as u64, &mut out).unwrap();
        assert_eq!(out, d);
        st
    }

    #[test]
    fn zip_roundtrip() {
        let d = zip(&[
            ("a.xml", text(200_000, 1), 8),
            ("b.bin", (0..5000u32).map(|i| (i * 7) as u8).collect(), 0),
            ("c.xml", text(50_000, 2), 8),
            ("d.xml", text(10, 3), 8),
        ]);
        assert_eq!(detect_bytes(&d), Some(Container::Zip));
        let st = roundtrip(&d, Container::Zip);
        assert!(st.streams >= 2);
    }

    #[test]
    fn zip_levels() {
        for lvl in [1, 3, 6, 9] {
            let mut e = DeflateEncoder::new(Vec::new(), Compression::new(lvl));
            e.write_all(&text(100_000, lvl as u64)).unwrap();
            let payload = e.finish().unwrap();
            let mut z = b"PK\x03\x04".to_vec();
            z.extend_from_slice(&[20, 0, 0, 0, 8, 0]);
            z.extend_from_slice(&[0u8; 16]);
            z.extend_from_slice(&[1, 0, 0, 0, b'x']);
            z.extend_from_slice(&payload);
            roundtrip(&z, Container::Zip);
        }
    }

    #[test]
    fn pdf_roundtrip() {
        let mut d = b"%PDF-1.4\n1 0 obj\n<< /Length 99 /Filter /FlateDecode >>\nstream\r\n".to_vec();
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&text(30_000, 9)).unwrap();
        d.extend_from_slice(&z.finish().unwrap());
        d.extend_from_slice(b"\nendstream\nendobj\n2 0 obj\nstream\n");
        let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
        z.write_all(&text(20_000, 10)).unwrap();
        d.extend_from_slice(&z.finish().unwrap());
        d.extend_from_slice(b"\nendstream\nendobj\n%%EOF\n");
        assert_eq!(detect_bytes(&d), Some(Container::Pdf));
        let st = roundtrip(&d, Container::Pdf);
        assert_eq!(st.streams, 2);
    }

    #[test]
    fn gzip_multi_member() {
        let mut d = Vec::new();
        for i in 0..3 {
            let mut g = GzEncoder::new(Vec::new(), Compression::default());
            g.write_all(&text(40_000, 20 + i)).unwrap();
            d.extend_from_slice(&g.finish().unwrap());
        }
        assert_eq!(detect_bytes(&d), Some(Container::Gzip));
        let st = roundtrip(&d, Container::Gzip);
        assert_eq!(st.streams, 3);
    }

    #[test]
    fn not_containers() {
        assert_eq!(detect_bytes(b"hello world"), None);
        assert!(encode(b"PK\x03\x04 but nothing valid follows here at all..............", Container::Zip).is_none());
    }

    #[test]
    fn decoder_survives_garbage() {
        install_panic_hook();
        let d = zip(&[("a.xml", text(30_000, 5), 8)]);
        let (t, _) = encode(&d, Container::Zip).unwrap();
        let mut s = 0x9E3779B97F4A7C15u64;
        for round in 0..400 {
            let mut m = t.clone();
            if round % 2 == 0 {
                for _ in 0..4 {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    let i = (s % m.len() as u64) as usize;
                    m[i] ^= 1 << (s % 8);
                }
            } else {
                m.truncate((s % m.len() as u64) as usize);
                s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            }
            let mut out = Vec::new();
            if decode(&mut Cursor::new(&m), d.len() as u64, &mut out).is_ok() {
                // A flip that still decodes must not produce wrong data silently
                // in the archive: the entry CRC catches it. Here we only require
                // no panic and bounded output.
                assert!(out.len() as u64 <= d.len() as u64);
            }
        }
    }
}
