//! The nikpak library: the .nik format, its engines and filters, and the two
//! windows. It is shared by the two programs built from this project:
//!
//! * `nikpak` (src/main.rs): the archiver - command line and drop window;
//! * `unNik`  (src/bin/unnik.rs): the free archive browser / extractor.

pub mod archive;
pub mod bits;
pub mod cli;
pub mod cover;
pub mod crypto;
pub mod engine;
pub mod envpath;
pub mod error;
pub mod format;
pub mod license;
#[cfg(feature = "gui")]
pub mod gui;
#[cfg(feature = "gui")]
pub mod i18n;
#[cfg(feature = "gui")]
pub mod sfx;
#[cfg(feature = "gui")]
pub mod shell;
#[cfg(feature = "gui")]
pub mod skin;
#[cfg(feature = "gui")]
mod skin_scenes;
#[cfg(feature = "gui")]
mod skins_data;
#[cfg(feature = "gui")]
pub mod textshape;
#[cfg(feature = "gui")]
pub mod unnik;
pub mod media;
pub mod nac;
pub mod recomp;
/// A `.nik` that is a Nikkosynth soundbank rather than a NikPak archive: its
/// readable header, so unNik can say what the file is instead of refusing it.
pub mod soundbank;
/// A `.nik` that is a NikPak **package** rather than an archive: an encrypted,
/// compressed bundle of files with a name, cover, description and EULA (the
/// NikPak sibling of Nikkosynth's soundbank, with its own magic so the two never
/// import into each other).
pub mod package;
/// Split volumes (`photos.nik`, `photos.n01`, …) and self-extracting `.exe`
/// archives: the reader that follows the volumes, the writer that produces
/// them, and the trailer that marks an archive appended to unNik.
pub mod split;
pub mod update;
pub mod util;
