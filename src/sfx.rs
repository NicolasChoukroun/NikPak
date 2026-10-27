//! Sound effects of the two windows (NikPak and unNik).
//!
//! The sounds are synthesised at start-up (no sound files, no audio library):
//! short 16-bit mono WAV images held in memory. On Windows they are played
//! with PlaySound from memory, asynchronously (a new sound replaces the one
//! playing). On macOS / Linux they are written once to the temporary folder
//! and played with afplay / paplay / aplay when one of them exists.
//!
//! Sounds are on by default; the speaker button in both windows switches
//! them off and on (setting `sounds=on|off`, shared by the two programs).

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sfx {
    /// a button, a link or an option
    Click,
    /// sounds switched on (two rising notes)
    On,
    /// sounds switched off (two falling notes)
    Off,
    /// a box, a menu or the viewer opens
    Pop,
    /// work starts (compression, extraction ...)
    Start,
    /// an archive was opened in unNik
    Open,
    /// work finished well (archive made, files extracted, test passed)
    Success,
    /// something went wrong
    Error,
    /// files were deleted from an archive
    Delete,
    /// the user cancelled
    Cancel,
    /// switching to the other program
    Switch,
    /// files hover over the window
    Hover,
}

pub const ALL: [Sfx; 12] = [Sfx::Click, Sfx::On, Sfx::Off, Sfx::Pop, Sfx::Start, Sfx::Open, Sfx::Success, Sfx::Error, Sfx::Delete, Sfx::Cancel, Sfx::Switch, Sfx::Hover];

impl Sfx {
    pub fn name(self) -> &'static str {
        match self {
            Sfx::Click => "click",
            Sfx::On => "on",
            Sfx::Off => "off",
            Sfx::Pop => "pop",
            Sfx::Start => "start",
            Sfx::Open => "open",
            Sfx::Success => "success",
            Sfx::Error => "error",
            Sfx::Delete => "delete",
            Sfx::Cancel => "cancel",
            Sfx::Switch => "switch",
            Sfx::Hover => "hover",
        }
    }
}

static ENABLED: AtomicBool = AtomicBool::new(true);

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Play a sound (nothing when sounds are off). Never blocks.
pub fn play(s: Sfx) {
    if !enabled() || std::env::var_os("NIKPAK_GUI_MUTE").is_some() {
        return;
    }
    backend::play(s);
}

// ---------------------------------------------------------------------------
// Synthesis
// ---------------------------------------------------------------------------

const RATE: u32 = 44_100;

/// One note: frequency glide from `f0` to `f1`, start time and length in
/// seconds, volume, attack, and a bell-like overtone amount.
struct Note {
    f0: f32,
    f1: f32,
    at: f32,
    len: f32,
    vol: f32,
    attack: f32,
    bell: f32,
}

fn note(f0: f32, f1: f32, at: f32, len: f32, vol: f32) -> Note {
    Note { f0, f1, at, len, vol, attack: 0.004, bell: 0.0 }
}

fn render(notes: &[Note], noise: Option<(f32, f32)>) -> Vec<f32> {
    let total = notes.iter().map(|n| n.at + n.len).fold(0.0f32, f32::max).max(noise.map_or(0.0, |n| n.0)) + 0.02;
    let len = (total * RATE as f32) as usize;
    let mut out = vec![0.0f32; len];
    for n in notes {
        let start = (n.at * RATE as f32) as usize;
        let count = (n.len * RATE as f32) as usize;
        let mut phase = 0.0f32;
        for i in 0..count {
            let t = i as f32 / RATE as f32;
            let k = t / n.len;
            let f = n.f0 * (n.f1 / n.f0).powf(k); // exponential glide
            phase += f / RATE as f32;
            let env = (t / n.attack).min(1.0) * (-5.0 * k).exp() * (1.0 - k).max(0.0).sqrt();
            let mut v = (phase * TAU).sin();
            if n.bell > 0.0 {
                v += n.bell * (phase * TAU * 2.0).sin() * (-9.0 * k).exp() + n.bell * 0.4 * (phase * TAU * 3.01).sin() * (-14.0 * k).exp();
            }
            if let Some(o) = out.get_mut(start + i) {
                *o += v * env * n.vol;
            }
        }
    }
    if let Some((dur, vol)) = noise {
        // a soft filtered noise "tick" at the start
        let mut x = 0x1234_5678u32;
        let mut lp = 0.0f32;
        let n = (dur * RATE as f32) as usize;
        for i in 0..n.min(out.len()) {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let w = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
            lp += 0.35 * (w - lp);
            let env = (-(i as f32) / (n as f32 * 0.25)).exp();
            out[i] += (w - lp) * env * vol;
        }
    }
    // fade the last 5 ms and limit
    let fade = (0.005 * RATE as f32) as usize;
    let l = out.len();
    for i in 0..fade.min(l) {
        out[l - 1 - i] *= i as f32 / fade as f32;
    }
    out
}

fn design(s: Sfx) -> Vec<f32> {
    let bell = |f0: f32, at: f32, len: f32, vol: f32| Note { f0, f1: f0, at, len, vol, attack: 0.003, bell: 0.5 };
    match s {
        Sfx::Click => render(&[note(2400.0, 1800.0, 0.0, 0.035, 0.16)], Some((0.012, 0.10))),
        Sfx::Hover => render(&[note(900.0, 1300.0, 0.0, 0.09, 0.07)], None),
        Sfx::Pop => render(&[note(520.0, 1040.0, 0.0, 0.07, 0.22)], Some((0.006, 0.05))),
        Sfx::On => render(&[bell(660.0, 0.0, 0.12, 0.22), bell(990.0, 0.07, 0.18, 0.22)], None),
        Sfx::Off => render(&[bell(990.0, 0.0, 0.12, 0.20), bell(660.0, 0.07, 0.18, 0.20)], None),
        Sfx::Start => render(&[note(330.0, 660.0, 0.0, 0.16, 0.22), note(660.0, 990.0, 0.08, 0.14, 0.12)], None),
        Sfx::Open => render(&[note(300.0, 900.0, 0.0, 0.18, 0.20), bell(1175.0, 0.12, 0.25, 0.12)], None),
        // C6 - E6 - G6 - C7, a short bright arpeggio
        Sfx::Success => render(&[bell(1046.5, 0.0, 0.35, 0.20), bell(1318.5, 0.07, 0.35, 0.18), bell(1568.0, 0.14, 0.40, 0.17), bell(2093.0, 0.22, 0.50, 0.12)], None),
        Sfx::Error => render(
            &[
                Note { f0: 440.0, f1: 430.0, at: 0.0, len: 0.16, vol: 0.24, attack: 0.004, bell: 0.35 },
                Note { f0: 349.2, f1: 340.0, at: 0.15, len: 0.26, vol: 0.24, attack: 0.004, bell: 0.35 },
            ],
            None,
        ),
        Sfx::Delete => render(&[note(620.0, 140.0, 0.0, 0.24, 0.26)], Some((0.08, 0.06))),
        Sfx::Cancel => render(&[note(700.0, 420.0, 0.0, 0.10, 0.20), note(420.0, 300.0, 0.09, 0.12, 0.16)], None),
        Sfx::Switch => render(&[note(400.0, 1200.0, 0.0, 0.22, 0.18), bell(1568.0, 0.16, 0.25, 0.10), bell(2093.0, 0.21, 0.25, 0.07)], None),
    }
}

/// A complete WAV file (RIFF, PCM 16-bit mono 44.1 kHz).
pub fn wav(s: Sfx) -> Vec<u8> {
    let samples = design(s);
    let data_len = (samples.len() * 2) as u32;
    let mut w = Vec::with_capacity(44 + data_len as usize);
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + data_len).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&1u16.to_le_bytes()); // mono
    w.extend_from_slice(&RATE.to_le_bytes());
    w.extend_from_slice(&(RATE * 2).to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&data_len.to_le_bytes());
    for v in samples {
        let x = (v.clamp(-1.0, 1.0) * 32000.0) as i16;
        w.extend_from_slice(&x.to_le_bytes());
    }
    w
}

/// All sounds, made once.
fn bank() -> &'static Vec<Vec<u8>> {
    static BANK: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    BANK.get_or_init(|| ALL.iter().map(|s| wav(*s)).collect())
}

fn index(s: Sfx) -> usize {
    ALL.iter().position(|x| *x == s).unwrap_or(0)
}

/// Make the sounds in the background at start, so the first click is instant.
pub fn warm_up() {
    std::thread::spawn(|| {
        let _ = bank();
        #[cfg(not(windows))]
        let _ = backend::files();
    });
}

#[cfg(windows)]
mod backend {
    use super::*;
    #[link(name = "winmm")]
    extern "system" {
        fn PlaySoundW(sound: *const u8, module: *mut core::ffi::c_void, flags: u32) -> i32;
    }
    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;

    pub fn play(s: Sfx) {
        let data = &bank()[index(s)];
        // SAFETY: the WAV image lives for the whole program (static bank);
        // SND_MEMORY reads it from there, SND_ASYNC returns at once.
        unsafe {
            PlaySoundW(data.as_ptr(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
        }
    }
}

#[cfg(not(windows))]
mod backend {
    use super::*;
    use std::path::PathBuf;

    /// The sounds as files in the temporary folder, and the player to use.
    pub fn files() -> &'static Option<(String, Vec<PathBuf>)> {
        static FILES: OnceLock<Option<(String, Vec<PathBuf>)>> = OnceLock::new();
        FILES.get_or_init(|| {
            let player = ["afplay", "paplay", "aplay"].iter().find(|p| {
                std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(p).is_file()))
            })?;
            let dir = std::env::temp_dir().join(format!("nikpak-sfx-{}", std::process::id()));
            std::fs::create_dir_all(&dir).ok()?;
            let mut list = Vec::new();
            for (k, s) in ALL.iter().enumerate() {
                let p = dir.join(format!("{}.wav", s.name()));
                std::fs::write(&p, &bank()[k]).ok()?;
                list.push(p);
            }
            Some((player.to_string(), list))
        })
    }

    pub fn play(s: Sfx) {
        if let Some((player, list)) = files() {
            let mut cmd = std::process::Command::new(player);
            if player == "aplay" {
                cmd.arg("-q");
            }
            let _ = cmd.arg(&list[index(s)]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_valid_wavs() {
        for s in ALL {
            let w = wav(s);
            assert_eq!(&w[..4], b"RIFF");
            assert_eq!(&w[8..16], b"WAVEfmt ");
            let data = u32::from_le_bytes(w[40..44].try_into().unwrap()) as usize;
            assert_eq!(w.len(), 44 + data);
            let secs = data as f32 / 2.0 / RATE as f32;
            assert!(secs > 0.02 && secs < 1.0, "{s:?}: {secs} s");
            // audible, never clipped
            let peak = w[44..].chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs()).max().unwrap();
            assert!(peak > 1500 && peak < 32_000, "{s:?}: peak {peak}");
        }
    }
}
