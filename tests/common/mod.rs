#![allow(dead_code)]
use std::path::Path;
pub fn png_bytes(
    color: png::ColorType,
    depth: png::BitDepth,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        encoder.set_compression(png::Compression::NoCompression);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(pixels)
            .unwrap();
    }
    bytes
}
pub fn rgba() -> Vec<u8> {
    png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        2,
        1,
        &[12, 34, 56, 0, 200, 100, 50, 255],
    )
}
pub fn add_chunk(bytes: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = bytes[..33].to_vec();
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut h = crc32fast::Hasher::new();
    h.update(kind);
    h.update(data);
    out.extend_from_slice(&h.finalize().to_be_bytes());
    out.extend_from_slice(&bytes[33..]);
    out
}

pub fn before_iend(bytes: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let standalone = add_chunk(&rgba(), kind, data);
    let mut out = bytes[..bytes.len() - 12].to_vec();
    out.extend_from_slice(&standalone[33..33 + 12 + data.len()]);
    out.extend_from_slice(&bytes[bytes.len() - 12..]);
    out
}
pub fn cli(args: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_kito"))
        .args(args)
        .output()
        .unwrap()
}
pub fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}
