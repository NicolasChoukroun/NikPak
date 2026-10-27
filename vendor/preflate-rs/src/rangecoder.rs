/*---------------------------------------------------------------------------------------------
 *  Added by Nikkoid for NikPak (2026). Licensed under the Apache License, Version 2.0,
 *  like the rest of this crate.
 *--------------------------------------------------------------------------------------------*/

//! Adaptive binary range coder used to store preflate's correction data.
//!
//! It replaces the `cabac` crate used by upstream preflate-rs (that crate is
//! LGPL-3.0, which is unsuitable for a statically linked, closed-source
//! program). This is an independent implementation of the classic
//! LZMA-style binary range coder: 11-bit probabilities adapted by 1/32 per
//! coded bit, 32-bit range with byte-wise carry propagation.
//!
//! The decoder never panics on damaged input: reading past the end yields
//! zero bytes, and the caller detects the damage through its own checks.

use std::io::{self, Read, Write};

const PROB_BITS: u32 = 11;
const PROB_ONE: u16 = 1 << PROB_BITS;
const ADAPT: u32 = 5;
const TOP: u32 = 1 << 24;

/// Adaptive probability that the next bit is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prob(u16);

impl Default for Prob {
    fn default() -> Self {
        Prob(PROB_ONE / 2)
    }
}

/// Writes adaptively coded bits. Named like the upstream trait so the
/// prediction codec needs no other changes.
pub trait CabacWriter<C> {
    fn put(&mut self, bit: bool, ctx: &mut C) -> io::Result<()>;
    fn finish(&mut self) -> io::Result<()>;

    /// `value` ones followed by a zero; the i-th bit uses ctx[min(i, N-1)].
    fn put_unary_encoded<const N: usize>(&mut self, value: usize, ctx: &mut [C; N]) -> io::Result<()> {
        for i in 0..=value {
            self.put(i < value, &mut ctx[i.min(N - 1)])?;
        }
        Ok(())
    }

    /// The low `n` bits of `value` (n <= 64), most significant first; bit k
    /// uses ctx[min(k, N-1)].
    fn put_n_bits<const N: usize>(&mut self, value: u64, n: usize, ctx: &mut [C; N]) -> io::Result<()> {
        for k in (0..n.min(64)).rev() {
            self.put((value >> k) & 1 != 0, &mut ctx[k.min(N - 1)])?;
        }
        Ok(())
    }
}

pub trait CabacReader<C> {
    fn get(&mut self, ctx: &mut C) -> io::Result<bool>;

    /// Counterpart of put_unary_encoded. Damaged input cannot make it loop
    /// for ever: values are capped at 64 (preflate never writes more than 32).
    fn get_unary_encoded<const N: usize>(&mut self, ctx: &mut [C; N]) -> io::Result<usize> {
        let mut i = 0;
        while i < 64 && self.get(&mut ctx[i.min(N - 1)])? {
            i += 1;
        }
        Ok(i)
    }

    fn get_n_bits<const N: usize>(&mut self, n: usize, ctx: &mut [C; N]) -> io::Result<u64> {
        let mut v = 0u64;
        for k in (0..n.min(64)).rev() {
            if self.get(&mut ctx[k.min(N - 1)])? {
                v |= 1 << k;
            }
        }
        Ok(v)
    }
}

pub struct RangeWriter<W: Write> {
    out: W,
    low: u64,
    range: u32,
    cache: u8,
    pending: u64,
    finished: bool,
}

impl<W: Write> RangeWriter<W> {
    pub fn new(out: W) -> io::Result<Self> {
        Ok(RangeWriter { out, low: 0, range: u32::MAX, cache: 0, pending: 1, finished: false })
    }

    fn shift_low(&mut self) -> io::Result<()> {
        // emit the cached byte (and any 0xFF run) once no carry can reach it
        if self.low < 0xFF00_0000 || self.low >= 1 << 32 {
            let carry = (self.low >> 32) as u8;
            let mut byte = self.cache;
            while self.pending > 0 {
                self.out.write_all(&[byte.wrapping_add(carry)])?;
                byte = 0xFF;
                self.pending -= 1;
            }
            self.cache = (self.low >> 24) as u8;
        }
        self.pending += 1;
        self.low = (self.low & 0x00FF_FFFF) << 8;
        Ok(())
    }
}

impl<W: Write> CabacWriter<Prob> for RangeWriter<W> {
    fn put(&mut self, bit: bool, p: &mut Prob) -> io::Result<()> {
        let bound = (self.range >> PROB_BITS) * p.0 as u32;
        if bit {
            self.low += bound as u64;
            self.range -= bound;
            p.0 -= p.0 >> ADAPT;
        } else {
            self.range = bound;
            p.0 += (PROB_ONE - p.0) >> ADAPT;
        }
        while self.range < TOP {
            self.range <<= 8;
            self.shift_low()?;
        }
        Ok(())
    }

    fn finish(&mut self) -> io::Result<()> {
        if !self.finished {
            self.finished = true;
            for _ in 0..5 {
                self.shift_low()?;
            }
            self.out.flush()?;
        }
        Ok(())
    }
}

pub struct RangeReader<R: Read> {
    input: R,
    code: u32,
    range: u32,
}

impl<R: Read> RangeReader<R> {
    pub fn new(mut input: R) -> io::Result<Self> {
        let mut code = 0u32;
        for _ in 0..5 {
            code = (code << 8) | next_byte(&mut input) as u32;
        }
        Ok(RangeReader { input, code, range: u32::MAX })
    }
}

fn next_byte<R: Read>(r: &mut R) -> u8 {
    let mut b = [0u8; 1];
    match r.read(&mut b) {
        Ok(1) => b[0],
        _ => 0,
    }
}

impl<R: Read> CabacReader<Prob> for RangeReader<R> {
    fn get(&mut self, p: &mut Prob) -> io::Result<bool> {
        let bound = (self.range >> PROB_BITS) * p.0 as u32;
        let bit = if self.code < bound {
            self.range = bound;
            p.0 += (PROB_ONE - p.0) >> ADAPT;
            false
        } else {
            self.code -= bound;
            self.range -= bound;
            p.0 -= p.0 >> ADAPT;
            true
        };
        while self.range < TOP {
            self.range <<= 8;
            self.code = (self.code << 8) | next_byte(&mut self.input) as u32;
        }
        Ok(bit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn roundtrip_skewed_and_random_bits() {
        let mut s = 0x1234_5678_9abc_def0u64;
        let mut bits = Vec::new();
        for i in 0..200_000 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            // mixture of very predictable and random streams
            bits.push(if i % 3 == 0 { s & 1 == 1 } else { s % 50 == 0 });
        }
        let mut buf = Vec::new();
        let mut w = RangeWriter::new(&mut buf).unwrap();
        let mut ctx = [Prob::default(); 3];
        for (i, &b) in bits.iter().enumerate() {
            w.put(b, &mut ctx[i % 3]).unwrap();
        }
        w.finish().unwrap();
        assert!(buf.len() < bits.len() / 8, "should compress: {} bytes", buf.len());
        let mut r = RangeReader::new(Cursor::new(&buf)).unwrap();
        let mut ctx = [Prob::default(); 3];
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!(r.get(&mut ctx[i % 3]).unwrap(), b, "bit {i}");
        }
    }

    #[test]
    fn unary_and_n_bits() {
        let values: Vec<(usize, u64, usize)> = (0..40u64).map(|i| ((i % 34) as usize, (i * 2654435761u64) & 0xFFFF_FFFF, (i % 32) as usize)).collect();
        let mut buf = Vec::new();
        let mut w = RangeWriter::new(&mut buf).unwrap();
        let (mut cu, mut cb) = ([Prob::default(); 32], [Prob::default(); 32]);
        for &(u, v, n) in &values {
            w.put_unary_encoded(u, &mut cu).unwrap();
            w.put_n_bits(v, n, &mut cb).unwrap();
        }
        w.finish().unwrap();
        let mut r = RangeReader::new(Cursor::new(&buf)).unwrap();
        let (mut cu, mut cb) = ([Prob::default(); 32], [Prob::default(); 32]);
        for &(u, v, n) in &values {
            assert_eq!(r.get_unary_encoded(&mut cu).unwrap(), u);
            assert_eq!(r.get_n_bits(n, &mut cb).unwrap(), v & ((1u64 << n) - 1));
        }
    }

    #[test]
    fn garbage_never_panics() {
        let mut ctx = [Prob::default(); 32];
        for len in [0usize, 1, 3, 7, 100] {
            let data: Vec<u8> = (0..len).map(|i| (i * 97 + 13) as u8).collect();
            let mut r = RangeReader::new(Cursor::new(&data)).unwrap();
            for _ in 0..1000 {
                let _ = r.get_unary_encoded(&mut ctx).unwrap();
                let _ = r.get_n_bits(31, &mut ctx).unwrap();
            }
        }
    }
}
