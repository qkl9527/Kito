//! 对少色图探索两种确定性 palette 顺序；不量化、不改色。
use super::png::{PngImage, chunks};
use anyhow::Result;
use std::collections::BTreeMap;

pub fn variants(source: &[u8], budget: usize) -> Result<Vec<(String, Vec<u8>)>> {
    // 元数据与调色板可能相互依赖；首版自定义搜索只处理无附加元数据的输入。
    if chunks(source)?
        .iter()
        .any(|c| !matches!(&c.kind, b"IHDR" | b"IDAT" | b"IEND" | b"PLTE" | b"tRNS"))
    {
        return Ok(Vec::new());
    }
    let image = PngImage::decode(source, budget)?;
    let mut counts = BTreeMap::<[u8; 4], usize>::new();
    for pixel in image.pixels.as_chunks::<4>().0 {
        if pixel.iter().any(|c| c % 257 != 0) {
            return Ok(Vec::new());
        }
        let color = [
            (pixel[0] / 257) as u8,
            (pixel[1] / 257) as u8,
            (pixel[2] / 257) as u8,
            (pixel[3] / 257) as u8,
        ];
        *counts.entry(color).or_default() += 1;
        if counts.len() > 256 {
            return Ok(Vec::new());
        }
    }
    let colors: Vec<_> = counts.keys().copied().collect();
    let mut result = Vec::new();
    for mode in ["frequency", "luma"] {
        let mut order = colors.clone();
        if mode == "frequency" {
            order.sort_by_key(|c| (std::cmp::Reverse(counts[c]), *c));
        } else {
            order.sort_by_key(|c| {
                (
                    u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114,
                    c[3],
                    *c,
                )
            });
        }
        let indices: BTreeMap<_, _> = order
            .iter()
            .enumerate()
            .map(|(i, c)| (*c, i as u8))
            .collect();
        let data: Vec<u8> = image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| {
                indices[&[
                    (p[0] / 257) as u8,
                    (p[1] / 257) as u8,
                    (p[2] / 257) as u8,
                    (p[3] / 257) as u8,
                ]]
            })
            .collect();
        let mut output = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut output, image.width, image.height);
            encoder.set_color(png::ColorType::Indexed);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_palette(
                order
                    .iter()
                    .flat_map(|p| p[..3].iter().copied())
                    .collect::<Vec<_>>(),
            );
            if order.iter().any(|c| c[3] != 255) {
                encoder.set_trns(order.iter().map(|c| c[3]).collect::<Vec<_>>());
            }
            encoder.write_header()?.write_image_data(&data)?;
        }
        result.push((format!("palette-{mode}"), output));
    }
    Ok(result)
}
