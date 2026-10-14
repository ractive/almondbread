//! PNG output of sRGB BGRA8 pixels (the drawable / offscreen target format).

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::Path;

/// Encodes sRGB BGRA8 pixels (`width * 4` bytes per row, alpha ignored) as an RGB PNG
/// tagged sRGB.
pub fn encode_png_bgra(bgra: &[u8], width: usize, height: usize, out: impl io::Write) -> io::Result<()> {
    assert_eq!(bgra.len(), width * height * 4, "pixel buffer size");
    let mut enc = png::Encoder::new(out, width as u32, height as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    enc.set_compression(png::Compression::Fast);
    let mut writer = enc.write_header().map_err(io::Error::other)?;
    let rgb: Vec<u8> = bgra
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| [p[2], p[1], p[0]])
        .collect();
    writer.write_image_data(&rgb).map_err(io::Error::other)?;
    writer.finish().map_err(io::Error::other)
}

pub fn write_png_bgra(path: impl AsRef<Path>, bgra: &[u8], width: usize, height: usize) -> io::Result<()> {
    let file = BufWriter::new(File::create(path)?);
    encode_png_bgra(bgra, width, height, file)
}

#[cfg(test)]
mod tests {
    #[test]
    fn encodes() {
        let px = [10u8, 20, 30, 255].repeat(6);
        let mut out = Vec::new();
        super::encode_png_bgra(&px, 3, 2, &mut out).unwrap();
        assert_eq!(&out[1..4], b"PNG");
    }
}
