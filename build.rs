//! Gives nikpak.exe and unNik.exe their own icons in Explorer (Windows only; nothing to do
//! elsewhere). The window icons are drawn in the code (gui.rs, unnik.rs).
fn main() {
    println!("cargo:rerun-if-changed=assets/nikpak.rc");
    println!("cargo:rerun-if-changed=assets/unnik.rc");
    println!("cargo:rerun-if-changed=assets/nikpak.ico");
    println!("cargo:rerun-if-changed=assets/unnik.ico");
    println!("cargo:rerun-if-changed=assets/nikfile.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // a missing resource compiler (rc.exe / windres) only costs the icon, never the build
    if let Err(e) = embed_resource::compile_for("assets/nikpak.rc", ["nikpak"], embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=nikpak.exe icon not embedded: {e}");
    }
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        if let Err(e) = embed_resource::compile_for("assets/unnik.rc", ["unNik"], embed_resource::NONE).manifest_optional() {
            println!("cargo:warning=unNik.exe icon not embedded: {e}");
        }
    }
}
