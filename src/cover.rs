//! Archive covers: any picture, cropped square from its centre and scaled to
//! 1024 x 1024, stored as a JPEG in the archive header (see format.rs).

use image::imageops::FilterType;

/// The side of every stored cover, in pixels.
pub const COVER_SIDE: u32 = 1024;
/// Largest picture file accepted as a source (before it is scaled down).
pub const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
/// Largest cover stored in an archive (the encoded JPEG or PNG).
pub const MAX_COVER_BYTES: usize = 4 * 1024 * 1024;

/// JPEG or PNG, by its first bytes.
pub fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

/// Make a cover from a picture file's bytes (PNG, JPEG, WebP, BMP or GIF): cropped
/// square from the centre, scaled to 1024 x 1024, encoded as a quality-90 JPEG.
pub fn from_picture(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("The picture is too large (at most 64 MB).".into());
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("This picture cannot be read: {e}"))?;
    if img.width() == 0 || img.height() == 0 {
        return Err("This picture is empty.".into());
    }
    let square = img.resize_to_fill(COVER_SIDE, COVER_SIDE, FilterType::Lanczos3).to_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&square)
        .map_err(|e| format!("The cover could not be written: {e}"))?;
    Ok(out)
}

/// Make a cover from a picture file.
pub fn from_file(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("Cannot read '{}': {e}", path.display()))?.len();
    if len as usize > MAX_SOURCE_BYTES {
        return Err("The picture is too large (at most 64 MB).".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read '{}': {e}", path.display()))?;
    from_picture(&bytes)
}

/// Decode a stored cover into RGBA pixels for a window to draw: (width, height, rgba).
pub fn rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    kind(bytes)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    Some((img.width(), img.height(), img.into_raw()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn any_picture_becomes_a_square_1024_jpeg() {
        for (w, h) in [(2048, 2048), (300, 900), (1600, 400), (7, 5)] {
            let c = from_picture(&png(w, h)).unwrap();
            assert_eq!(kind(&c), Some("image/jpeg"));
            assert!(c.len() < MAX_COVER_BYTES);
            let (cw, ch, px) = rgba(&c).unwrap();
            assert_eq!((cw, ch), (1024, 1024), "{w}x{h}");
            assert_eq!(px.len(), 1024 * 1024 * 4);
        }
    }

    #[test]
    fn something_that_is_not_a_picture_is_refused() {
        assert!(from_picture(b"<html>no</html>").is_err());
        assert!(rgba(b"\xFF\xD8\xFFtruncated").is_none());
        assert_eq!(kind(b"GIF89a"), None);
    }
}
