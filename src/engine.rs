//! Multi-engine body (archive method 2, used by the maximum level -m5).
//!
//! The inner record stream is cut - only between entries - into blocks, and
//! every block is compressed with the engine that suits its content best:
//! LZMA2 for binary data (and for media / recompressed Deflate entries),
//! PPMd variant I for text, chosen by a trial compression of a sample. Each
//! block is solid. The decompressed blocks, concatenated, are exactly the
//! inner record stream of a single-engine archive.
//!
//! ```text
//! body  := block* 0x00
//! block := engine:u8 params frame* u32(0)
//!   engine 1 = zstd   params: -
//!   engine 2 = LZMA2  params: dict_size:u32 lc:u8 lp:u8 pb:u8
//!   engine 3 = PPMd8  params: order:u8 mem_size:u32 restore:u8
//! frame := len:u32 (1 ..= 1 MiB) bytes[len]
//! ```

use liblzma::stream::{Filters, LzmaOptions, Stream};
use ppmd_rust::{Ppmd8Decoder, Ppmd8Encoder, RestoreMethod};
use std::io::{self, Read, Write};

pub const ENG_END: u8 = 0;
pub const ENG_ZSTD: u8 = 1;
pub const ENG_LZMA2: u8 = 2;
pub const ENG_PPMD: u8 = 3;

const FRAME: usize = 256 * 1024;
const MAX_FRAME: u32 = 1 << 20;
/// Decoder memory limits (protect against hostile archives).
const MAX_DICT: u32 = 1536 << 20;
const MAX_PPMD_MEM: u32 = 1024 << 20;

pub const PPMD_ORDER: u8 = 8;
pub const PPMD_MEM: u32 = 192 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    /// Part of the format (readers must handle it); -m5 itself uses LZMA2/PPMd.
    #[allow(dead_code)]
    Zstd(i32),
    Lzma2 { dict: u32, lc: u8, lp: u8, pb: u8 },
    Ppmd { order: u8, mem: u32 },
}

impl Engine {
    pub fn name(&self) -> String {
        match self {
            Engine::Zstd(l) => format!("zstd {l}"),
            Engine::Lzma2 { dict, .. } => format!("LZMA2, {} MiB dictionary", dict >> 20),
            Engine::Ppmd { order, mem } => format!("PPMd order {order}, {} MiB", mem >> 20),
        }
    }

    /// LZMA2 at maximum effort with a dictionary sized to the data (capped).
    pub fn lzma2_for(input: u64, text: bool) -> Engine {
        let mut dict: u32 = 1 << 20;
        // capped at 128 MiB: the encoder needs about 11x the dictionary in RAM
        while (dict as u64) < input && dict < (128 << 20) {
            dict <<= 1;
        }
        if text {
            Engine::Lzma2 { dict, lc: 4, lp: 0, pb: 0 }
        } else {
            Engine::Lzma2 { dict, lc: 3, lp: 0, pb: 2 }
        }
    }

    pub fn ppmd_for(input: u64) -> Engine {
        // PPMd needs about 12x the input for its model at most; no point in more.
        let mut mem: u32 = 16 << 20;
        while (mem as u64) < input.saturating_mul(12) && mem < PPMD_MEM {
            mem <<= 1;
        }
        Engine::Ppmd { order: PPMD_ORDER, mem: mem.min(PPMD_MEM) }
    }

    fn header(&self) -> Vec<u8> {
        match *self {
            Engine::Zstd(_) => vec![ENG_ZSTD],
            Engine::Lzma2 { dict, lc, lp, pb } => {
                let mut v = vec![ENG_LZMA2];
                v.extend_from_slice(&dict.to_le_bytes());
                v.extend_from_slice(&[lc, lp, pb]);
                v
            }
            Engine::Ppmd { order, mem } => {
                let mut v = vec![ENG_PPMD, order];
                v.extend_from_slice(&mem.to_le_bytes());
                v.push(0); // restore method: restart
                v
            }
        }
    }
}

fn lzma_filters(dict: u32, lc: u8, lp: u8, pb: u8, encoder: bool) -> io::Result<Filters> {
    // Preset 9 with long matches (nice 273) but a bounded match-finder depth:
    // "extreme" mode is up to 10x slower on repetitive data (spreadsheet XML)
    // and was not smaller in our measurements.
    let mut o = LzmaOptions::new_preset(9).map_err(io::Error::other)?;
    o.dict_size(dict).literal_context_bits(lc as u32).literal_position_bits(lp as u32).position_bits(pb as u32);
    if encoder {
        o.nice_len(273).depth(32);
    }
    let mut f = Filters::new();
    f.lzma2(&o);
    Ok(f)
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

// ---------------------------------------------------------------------------
// Framing of one block's compressed bytes
// ---------------------------------------------------------------------------

pub struct Framer<W: Write> {
    inner: W,
    buf: Vec<u8>,
}

impl<W: Write> Framer<W> {
    fn new(inner: W) -> Self {
        Framer { inner, buf: Vec::with_capacity(FRAME) }
    }
    fn emit(&mut self) -> io::Result<()> {
        if !self.buf.is_empty() {
            self.inner.write_all(&(self.buf.len() as u32).to_le_bytes())?;
            self.inner.write_all(&self.buf)?;
            self.buf.clear();
        }
        Ok(())
    }
    fn finish(mut self) -> io::Result<W> {
        self.emit()?;
        self.inner.write_all(&0u32.to_le_bytes())?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for Framer<W> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let room = FRAME - self.buf.len();
        let n = b.len().min(room);
        self.buf.extend_from_slice(&b[..n]);
        if self.buf.len() == FRAME {
            self.emit()?;
        }
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Deframer<R: Read> {
    inner: R,
    left: u32,
    done: bool,
}

impl<R: Read> Deframer<R> {
    fn new(inner: R) -> Self {
        Deframer { inner, left: 0, done: false }
    }
    /// Skip what is left of the block; returns the number of skipped bytes.
    fn drain(&mut self) -> io::Result<u64> {
        io::copy(self, &mut io::sink())
    }
}

impl<R: Read> Read for Deframer<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.left == 0 {
            if self.done {
                return Ok(0);
            }
            let mut b = [0u8; 4];
            self.inner.read_exact(&mut b)?;
            let len = u32::from_le_bytes(b);
            if len == 0 {
                self.done = true;
                return Ok(0);
            }
            if len > MAX_FRAME {
                return Err(bad("compressed block frame is too large (damaged archive)"));
            }
            self.left = len;
        }
        let want = buf.len().min(self.left as usize);
        let n = self.inner.read(&mut buf[..want])?;
        if n == 0 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "archive is truncated"));
        }
        self.left -= n as u32;
        Ok(n)
    }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

enum Enc<W: Write> {
    Zstd(zstd::stream::write::Encoder<'static, Framer<W>>),
    Lzma(liblzma::write::XzEncoder<Framer<W>>),
    Ppmd(Ppmd8Encoder<Framer<W>>),
}

/// Writes the multi-engine body. Call `start` before writing each block's
/// records and `end` after them; `finish` writes the terminator.
pub struct BlockWriter<W: Write> {
    idle: Option<W>,
    enc: Option<Enc<W>>,
}

impl<W: Write> BlockWriter<W> {
    pub fn new(inner: W) -> Self {
        BlockWriter { idle: Some(inner), enc: None }
    }

    pub fn start(&mut self, e: Engine, threads: u32) -> io::Result<()> {
        if self.enc.is_some() {
            self.end()?;
        }
        let mut w = self.idle.take().expect("writer");
        w.write_all(&e.header())?;
        let fr = Framer::new(w);
        self.enc = Some(match e {
            Engine::Zstd(l) => {
                let mut z = zstd::stream::write::Encoder::new(fr, l)?;
                if threads > 1 {
                    z.multithread(threads)?;
                }
                let _ = z.include_checksum(true);
                if l >= 15 {
                    let _ = z.window_log(27);
                    let _ = z.long_distance_matching(true);
                }
                Enc::Zstd(z)
            }
            Engine::Lzma2 { dict, lc, lp, pb } => {
                let s = Stream::new_raw_encoder(&lzma_filters(dict, lc, lp, pb, true)?).map_err(io::Error::other)?;
                Enc::Lzma(liblzma::write::XzEncoder::new_stream(fr, s))
            }
            Engine::Ppmd { order, mem } => Enc::Ppmd(
                Ppmd8Encoder::new(fr, order as u32, mem, RestoreMethod::Restart)
                    .map_err(|e| io::Error::other(format!("{e:?}")))?,
            ),
        });
        Ok(())
    }

    pub fn end(&mut self) -> io::Result<()> {
        let fr = match self.enc.take() {
            None => return Ok(()),
            Some(Enc::Zstd(z)) => z.finish()?,
            Some(Enc::Lzma(z)) => z.finish()?,
            Some(Enc::Ppmd(p)) => p.finish(true)?,
        };
        self.idle = Some(fr.finish()?);
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.end()?;
        let mut w = self.idle.take().expect("writer");
        w.write_all(&[ENG_END])?;
        Ok(w)
    }
}

impl<W: Write> Write for BlockWriter<W> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self.enc.as_mut() {
            Some(Enc::Zstd(z)) => z.write(b),
            Some(Enc::Lzma(z)) => z.write(b),
            Some(Enc::Ppmd(p)) => p.write(b),
            None => Err(io::Error::other("no block started")),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------

enum Dec<R: Read> {
    Zstd(zstd::stream::read::Decoder<'static, io::BufReader<Deframer<R>>>),
    Lzma(liblzma::read::XzDecoder<Deframer<R>>),
    Ppmd(Ppmd8Decoder<Deframer<R>>),
}

pub struct BlockReader<R: Read> {
    idle: Option<R>,
    dec: Option<Dec<R>>,
    ended: bool,
    /// number of blocks opened so far, and the engine of the current one
    /// (used to write the same blocks again when entries are deleted)
    pub blocks: u32,
    pub engine: Option<Engine>,
}

impl<R: Read> BlockReader<R> {
    pub fn new(inner: R) -> Self {
        BlockReader { idle: Some(inner), dec: None, ended: false, blocks: 0, engine: None }
    }

    fn open_next(&mut self) -> io::Result<()> {
        let mut r = self.idle.take().ok_or_else(|| bad("block reader in a bad state"))?;
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        match b[0] {
            ENG_END => {
                self.ended = true;
                self.idle = Some(r);
            }
            ENG_ZSTD => {
                self.dec = Some(Dec::Zstd(zstd::stream::read::Decoder::new(Deframer::new(r))?));
                self.blocks += 1;
                self.engine = Some(Engine::Zstd(19));
            }
            ENG_LZMA2 => {
                let mut p = [0u8; 7];
                r.read_exact(&mut p)?;
                let dict = u32::from_le_bytes(p[0..4].try_into().unwrap());
                let (lc, lp, pb) = (p[4], p[5], p[6]);
                if !(4096..=MAX_DICT).contains(&dict) || lc > 4 || lp > 4 || lc + lp > 4 || pb > 4 {
                    return Err(bad("invalid LZMA2 parameters (damaged archive)"));
                }
                let s = Stream::new_raw_decoder(&lzma_filters(dict, lc, lp, pb, false)?).map_err(|e| bad(&format!("{e}")))?;
                self.dec = Some(Dec::Lzma(liblzma::read::XzDecoder::new_stream(Deframer::new(r), s)));
                self.blocks += 1;
                self.engine = Some(Engine::Lzma2 { dict, lc, lp, pb });
            }
            ENG_PPMD => {
                let mut p = [0u8; 6];
                r.read_exact(&mut p)?;
                let order = p[0] as u32;
                let mem = u32::from_le_bytes(p[1..5].try_into().unwrap());
                if !(2..=16).contains(&order) || !(1 << 16..=MAX_PPMD_MEM).contains(&mem) || p[5] != 0 {
                    return Err(bad("invalid PPMd parameters (damaged archive)"));
                }
                let d = Ppmd8Decoder::new(Deframer::new(r), order, mem, RestoreMethod::Restart)
                    .map_err(|e| bad(&format!("{e:?}")))?;
                self.dec = Some(Dec::Ppmd(d));
                self.blocks += 1;
                self.engine = Some(Engine::Ppmd { order: order as u8, mem });
            }
            _ => return Err(bad("unknown compression engine (archive made by a newer nikpak?)")),
        }
        Ok(())
    }

    /// The current block's decoder reported its end: check that its frames are
    /// fully consumed and hand the raw reader back.
    fn close_block(&mut self) -> io::Result<()> {
        let mut fr = match self.dec.take() {
            Some(Dec::Zstd(d)) => d.finish().into_inner(),
            Some(Dec::Lzma(d)) => d.into_inner(),
            Some(Dec::Ppmd(d)) => d.into_inner(),
            None => return Ok(()),
        };
        // zstd reads through a BufReader that may already hold the rest; the
        // other decoders stop exactly at their end marker. Anything beyond the
        // end of the compressed data is damage.
        let rest = fr.drain()?;
        if rest > 0 {
            return Err(bad("unexpected data after the end of a compressed block"));
        }
        self.idle = Some(fr.inner);
        Ok(())
    }
}

impl<R: Read> Read for BlockReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            if self.ended {
                return Ok(0);
            }
            if self.dec.is_none() {
                self.open_next()?;
                continue;
            }
            let n = match self.dec.as_mut().unwrap() {
                Dec::Zstd(d) => d.read(buf)?,
                Dec::Lzma(d) => d.read(buf)?,
                Dec::Ppmd(d) => d.read(buf)?,
            };
            if n > 0 {
                return Ok(n);
            }
            self.close_block()?;
        }
    }
}

// ---------------------------------------------------------------------------
// Trial: which engine compresses this sample best?
// ---------------------------------------------------------------------------

struct Counter(u64);
impl Write for Counter {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.0 += b.len() as u64;
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Compressed size of `sample` with engine `e`.
pub fn trial_size(sample: &[u8], e: Engine) -> io::Result<u64> {
    let mut w = BlockWriter::new(Counter(0));
    w.start(e, 1)?;
    w.write_all(sample)?;
    Ok(w.finish()?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn data(n: usize, seed: u64, text: bool) -> Vec<u8> {
        let mut s = seed | 1;
        (0..n)
            .map(|i| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                if text {
                    b"the quick brown fox jumps over the lazy dog\n"[(s % 44) as usize]
                } else if i % 3 == 0 {
                    s as u8
                } else {
                    (i / 7) as u8
                }
            })
            .collect()
    }

    #[test]
    fn blocks_roundtrip() {
        let engines = [
            Engine::Zstd(3),
            Engine::lzma2_for(1 << 20, false),
            Engine::lzma2_for(1 << 20, true),
            Engine::Ppmd { order: 8, mem: 16 << 20 },
            Engine::Ppmd { order: 4, mem: 1 << 20 },
        ];
        for sizes in [vec![0usize, 1, 100], vec![FRAME - 1, FRAME, FRAME + 1], vec![700_001, 3]] {
            let mut w = BlockWriter::new(Vec::new());
            let mut expect = Vec::new();
            for (i, e) in engines.iter().enumerate() {
                let d = data(sizes[i % sizes.len()], i as u64 + 1, i % 2 == 0);
                w.start(*e, 1).unwrap();
                w.write_all(&d).unwrap();
                expect.extend_from_slice(&d);
            }
            let out = w.finish().unwrap();
            let mut r = BlockReader::new(Cursor::new(&out));
            let mut got = Vec::new();
            r.read_to_end(&mut got).unwrap();
            assert_eq!(got, expect);
            // reader stops exactly at the terminator
            let mut inner = r.idle.take().unwrap();
            assert_eq!(inner.position() as usize, out.len());
            assert_eq!(inner.read(&mut [0u8; 4]).unwrap(), 0);
        }
    }

    #[test]
    fn truncated_or_damaged_fails() {
        let mut w = BlockWriter::new(Vec::new());
        let d = data(300_000, 7, true);
        w.start(Engine::Ppmd { order: 8, mem: 16 << 20 }, 1).unwrap();
        w.write_all(&d).unwrap();
        w.start(Engine::lzma2_for(1 << 20, false), 1).unwrap();
        w.write_all(&data(300_000, 8, false)).unwrap();
        let out = w.finish().unwrap();
        let mut s = 12345u64;
        for k in 0..300 {
            let mut m = out.clone();
            if k % 3 == 0 {
                m.truncate((s % out.len() as u64) as usize);
            } else {
                let i = (s % out.len() as u64) as usize;
                m[i] ^= 1 << (k % 8);
            }
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mut r = BlockReader::new(Cursor::new(&m));
            let mut got = Vec::new();
            // must not panic; truncation must be reported
            let res = r.read_to_end(&mut got);
            if k % 3 == 0 {
                assert!(res.is_err());
            }
        }
    }

    #[test]
    fn hostile_parameters_rejected() {
        for hdr in [
            vec![ENG_LZMA2, 0xff, 0xff, 0xff, 0xff, 3, 0, 2],
            vec![ENG_LZMA2, 0, 0, 0x10, 0, 5, 0, 2],
            vec![ENG_PPMD, 8, 0xff, 0xff, 0xff, 0xff, 0],
            vec![ENG_PPMD, 40, 0, 0, 0, 1, 0],
            vec![9],
        ] {
            let mut r = BlockReader::new(Cursor::new(hdr));
            assert!(r.read(&mut [0u8; 16]).is_err());
        }
    }

    #[test]
    fn trial_prefers_ppmd_on_text() {
        // English-like prose with a realistic word distribution
        let words = ["the", "of", "and", "to", "in", "a", "is", "that", "it", "was", "for", "on", "are", "as", "with",
            "his", "they", "at", "be", "this", "have", "from", "or", "one", "had", "by", "word", "but", "not", "what",
            "all", "were", "we", "when", "your", "can", "said", "there", "use", "an", "each", "which", "she", "do",
            "how", "their", "if", "will", "up", "other", "about", "out", "many", "then", "them", "these", "so"];
        let mut s = 99u64;
        let mut t = Vec::new();
        while t.len() < 400_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let w = words[((s % 57) * (s % 57) / 57) as usize % words.len()];
            t.extend_from_slice(w.as_bytes());
            t.push(if s % 13 == 0 { b'\n' } else { b' ' });
        }
        let p = trial_size(&t, Engine::ppmd_for(t.len() as u64)).unwrap();
        let l = trial_size(&t, Engine::lzma2_for(t.len() as u64, true)).unwrap();
        assert!(p < l, "ppmd {p} lzma {l}");
    }
}
