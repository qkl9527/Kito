mod common;
use kito::image::{
    optimize::{OptimizeConfig, optimize},
    png::{PngImage, equivalent},
};
const BUDGET: usize = 64 * 1024 * 1024;
fn config() -> OptimizeConfig {
    OptimizeConfig {
        memory_bytes: BUDGET,
        ..Default::default()
    }
}

#[test]
fn preserves_hidden_rgb_and_rejects_changed_transparent_pixels() {
    let bytes = common::rgba();
    let decoded = PngImage::decode(&bytes, BUDGET).unwrap();
    assert_eq!(
        decoded.pixels,
        vec![3084, 8738, 14392, 0, 51400, 25700, 12850, 65535]
    );
    let changed = common::png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        2,
        1,
        &[0, 0, 0, 0, 200, 100, 50, 255],
    );
    assert!(equivalent(&bytes, &changed, false, BUDGET).is_err());
    let result = optimize(&bytes, &config()).unwrap();
    assert_eq!(
        PngImage::decode(&result.bytes, BUDGET).unwrap().pixels,
        decoded.pixels
    );
    assert!(result.bytes.len() <= bytes.len());
}

#[test]
fn keeps_low_bits_of_sixteen_bit_samples() {
    let bytes = common::png_bytes(
        png::ColorType::Rgb,
        png::BitDepth::Sixteen,
        1,
        1,
        &[0x12, 0x34, 0xab, 0xcd, 0x00, 0x01],
    );
    let result = optimize(&bytes, &config()).unwrap();
    assert_eq!(
        PngImage::decode(&result.bytes, BUDGET).unwrap().pixels,
        vec![0x1234, 0xabcd, 1, 65535]
    );
    let altered = common::png_bytes(
        png::ColorType::Rgb,
        png::BitDepth::Sixteen,
        1,
        1,
        &[0x12, 0x35, 0xab, 0xcd, 0x00, 0x01],
    );
    assert!(equivalent(&bytes, &altered, false, BUDGET).is_err());
}

#[test]
fn metadata_is_preserved_unless_explicitly_stripped() {
    let bytes = common::add_chunk(&common::rgba(), b"tEXt", b"author\0example");
    let bytes = common::add_chunk(&bytes, b"gAMA", &45455u32.to_be_bytes());
    let result = optimize(&bytes, &config()).unwrap();
    assert!(result.bytes.windows(14).any(|w| w == b"author\0example"));
    equivalent(&bytes, &result.bytes, false, BUDGET).unwrap();
    assert!(equivalent(&bytes, &common::rgba(), false, BUDGET).is_err());
    assert!(equivalent(&bytes, &common::rgba(), true, BUDGET).is_err());
    let stripped = optimize(
        &bytes,
        &OptimizeConfig {
            strip: true,
            ..config()
        },
    )
    .unwrap();
    assert!(!stripped.bytes.windows(14).any(|w| w == b"author\0example"));
    equivalent(&bytes, &stripped.bytes, true, BUDGET).unwrap();
}

#[test]
fn rejects_corrupt_crc_animation_and_memory_overflow() {
    let bytes = common::rgba();
    let mut broken = bytes.clone();
    broken[29] ^= 1;
    assert!(PngImage::decode(&broken, BUDGET).is_err());
    assert!(PngImage::decode(&bytes, 8).is_err());
    let animated = common::add_chunk(&bytes, b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]);
    assert!(optimize(&animated, &config()).is_err());
    assert!(PngImage::decode(&bytes[..bytes.len() - 5], BUDGET).is_err());
}

#[test]
fn solid_image_gets_smaller_without_changing_pixels() {
    let bytes = common::png_bytes(
        png::ColorType::Rgba,
        png::BitDepth::Eight,
        64,
        64,
        &vec![255; 64 * 64 * 4],
    );
    let result = optimize(&bytes, &config()).unwrap();
    assert!(result.bytes.len() < bytes.len() / 2);
    equivalent(&bytes, &result.bytes, false, BUDGET).unwrap();
}

#[test]
fn palette_and_low_depth_expand_exactly() {
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, 2, 1);
        e.set_color(png::ColorType::Indexed);
        e.set_depth(png::BitDepth::One);
        e.set_palette(vec![12, 34, 56, 200, 100, 50]);
        e.set_trns(vec![0, 255]);
        e.write_header()
            .unwrap()
            .write_image_data(&[0b01000000])
            .unwrap();
    }
    assert_eq!(
        PngImage::decode(&bytes, BUDGET).unwrap().pixels,
        vec![3084, 8738, 14392, 0, 51400, 25700, 12850, 65535]
    );
    let result = optimize(&bytes, &config()).unwrap();
    equivalent(&bytes, &result.bytes, false, BUDGET).unwrap();
}

#[test]
fn max_search_checks_custom_palette_candidates() {
    let bytes = common::rgba();
    let result = optimize(
        &bytes,
        &OptimizeConfig {
            max: true,
            ..config()
        },
    )
    .unwrap();
    assert!(
        result
            .trials
            .iter()
            .any(|t| t.engine.starts_with("palette-") && t.valid)
    );
    equivalent(&bytes, &result.bytes, false, BUDGET).unwrap();
}

#[test]
fn unknown_unsafe_chunk_keeps_original_and_zero_budget_is_reported() {
    let bytes = common::add_chunk(&common::rgba(), b"vpAU", b"unknown");
    let result = optimize(&bytes, &config()).unwrap();
    assert_eq!(result.bytes, bytes);
    let result = optimize(
        &common::rgba(),
        &OptimizeConfig {
            max: true,
            time_limit: Some(std::time::Duration::ZERO),
            ..config()
        },
    )
    .unwrap();
    assert!(!result.search_complete);
    assert_eq!(result.bytes, common::rgba());
}

#[test]
fn rejects_invalid_chunks_even_after_pixel_data() {
    let bytes = common::rgba();
    let critical = common::before_iend(&bytes, b"VpAg", b"unknown critical");
    assert!(PngImage::decode(&critical, BUDGET).is_err());
    let bad_gamma = common::add_chunk(&bytes, b"gAMA", &[1]);
    assert!(PngImage::decode(&bad_gamma, BUDGET).is_err());
    let text = common::before_iend(&bytes, b"tEXt", b"a\0b");
    let split = common::before_iend(&text, b"IDAT", &[]);
    assert!(PngImage::decode(&split, BUDGET).is_err());
}
