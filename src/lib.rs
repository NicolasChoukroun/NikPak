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
pub mod update;
pub mod util;
