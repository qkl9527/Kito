//! 仅接受受控数值和尺寸语法，用户输入不能直接成为 FFmpeg filter 表达式。
use crate::image::resize::ResizeSpec;
use anyhow::{Context, Result, ensure};

/// 秒、MM:SS 或 HH:MM:SS，最多微秒；不接受科学计数法和 FFmpeg 表达式。
pub fn timestamp(value: &str) -> Result<f64> {
    let parts: Vec<_> = value.split(':').collect();
    ensure!(
        (1..=3).contains(&parts.len()),
        "时间格式应为秒、MM:SS 或 HH:MM:SS"
    );
    let mut total = 0_u64;
    for (i, part) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        let (whole, fraction) = if last {
            part.split_once('.').unwrap_or((part, ""))
        } else {
            (*part, "")
        };
        ensure!(
            !whole.is_empty() && whole.bytes().all(|b| b.is_ascii_digit()),
            "时间只能包含非负十进制数字"
        );
        let whole: u64 = whole.parse().context("时间数值过大")?;
        if i > 0 {
            ensure!(whole < 60, "冒号后的分、秒必须小于 60");
        }
        ensure!(
            !part.contains('.') || (last && !fraction.is_empty()),
            "时间小数格式无效"
        );
        ensure!(
            fraction.len() <= 6 && fraction.bytes().all(|b| b.is_ascii_digit()),
            "时间小数最多 6 位"
        );
        total = total
            .checked_mul(60)
            .and_then(|v| whole.checked_mul(1_000_000).and_then(|n| v.checked_add(n)))
            .context("时间数值过大")?;
        if !fraction.is_empty() {
            let micros = fraction.parse::<u64>()? * 10_u64.pow(6 - fraction.len() as u32);
            total = total.checked_add(micros).context("时间数值过大")?;
        }
    }
    ensure!(total <= i32::MAX as u64 * 1_000_000, "时间数值过大");
    Ok(total as f64 / 1_000_000.0)
}

/// 与图片共享等比几何语义，再向下对齐到偶数，满足 H.264 yuv420p 编码要求。
pub fn size(value: &str, source: (u32, u32)) -> Result<(u32, u32)> {
    let (w, h) = value
        .parse::<ResizeSpec>()?
        .dimensions(source.0.into(), source.1.into())?;
    even_size(w, h)
}

pub fn even_size(width: u64, height: u64) -> Result<(u32, u32)> {
    ensure!(
        (2..=16384).contains(&width) && (2..=16384).contains(&height),
        "视频宽高必须在 2 至 16384 像素之间"
    );
    Ok(((width / 2 * 2) as u32, (height / 2 * 2) as u32))
}

/// atempo > 2 会跳过样本；分解到 0.5..=2 的级联以保持较好的音频质量。
pub fn tempo(mut factor: f64) -> Result<String> {
    ensure!(
        factor.is_finite() && (0.125..=8.0).contains(&factor),
        "速度倍数必须在 0.125 至 8 之间"
    );
    let mut stages = Vec::new();
    while factor > 2.0 {
        stages.push("atempo=2".to_owned());
        factor /= 2.0;
    }
    while factor < 0.5 {
        stages.push("atempo=0.5".to_owned());
        factor /= 0.5;
    }
    stages.push(format!("atempo={factor}"));
    Ok(stages.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_accept_seconds_minutes_hours_and_microseconds() {
        assert_eq!(timestamp("1.000001").unwrap(), 1.000001);
        assert_eq!(timestamp("02:03.5").unwrap(), 123.5);
        assert_eq!(timestamp("01:02:03.5").unwrap(), 3723.5);
        assert_eq!(timestamp("0").unwrap(), 0.0);
    }

    #[test]
    fn timestamps_reject_ambiguous_and_non_finite_inputs() {
        for value in [
            "",
            "-1",
            "+1",
            "NaN",
            "inf",
            "1e3",
            "1:60",
            "1:60:00",
            "1.5:00",
            "1:2:3:4",
            " 1",
            "1.",
            ".5",
            "0.0000001",
            "99999999999999999999",
        ] {
            assert!(timestamp(value).is_err(), "{value}");
        }
    }

    #[test]
    fn resize_reuses_image_geometry_and_aligns_for_yuv420() {
        assert_eq!(size("80x60", (160, 90)).unwrap(), (80, 44));
        assert_eq!(size("50%", (160, 90)).unwrap(), (80, 44));
        assert_eq!(size("x60", (160, 90)).unwrap(), (106, 60));
        for input in ["1x1", "0x", "999999999x", "80:60", "NaN%"] {
            assert!(size(input, (160, 90)).is_err(), "{input}");
        }
    }

    #[test]
    fn speed_chain_keeps_each_tempo_stage_in_quality_range() {
        assert_eq!(tempo(8.0).unwrap(), "atempo=2,atempo=2,atempo=2");
        assert_eq!(tempo(0.125).unwrap(), "atempo=0.5,atempo=0.5,atempo=0.5");
        assert_eq!(tempo(1.5).unwrap(), "atempo=1.5");
        for factor in [0.0, f64::NAN, f64::INFINITY, -1.0, 100.0] {
            assert!(tempo(factor).is_err());
        }
    }
}
