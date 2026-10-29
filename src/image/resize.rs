use std::{fmt, str::FromStr};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct ResizeSpec {
    requested: String,
    geometry: Geometry,
}

#[derive(Clone, Copy, Debug)]
enum Geometry {
    Fit { width: u64, height: u64 },
    Width(u64),
    Height(u64),
    Percentage { numerator: u128, denominator: u128 },
}

impl FromStr for ResizeSpec {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        // 只解析本工具定义的语法；不允许将 ImageMagick 的 geometry
        // 修饰符、空白或任意命令片段带入后续外部进程参数。
        let geometry = if let Some(percentage) = value.strip_suffix('%') {
            let (whole, fraction) = percentage.split_once('.').unwrap_or((percentage, ""));
            ensure!(
                !percentage.contains('.') || (!fraction.is_empty() && fraction.len() <= 4),
                "缩放百分比的小数部分必须为 1 至 4 位数字"
            );
            let whole = parse_digits(whole)?;
            let scale = 10_u128
                .checked_pow(fraction.len() as u32)
                .context("缩放百分比精度溢出")?;
            let fraction = if fraction.is_empty() {
                0
            } else {
                parse_digits(fraction)?
            };
            // 百分比保留为精确分数。例如 12.5% = 125 / 1000，
            // 不经过浮点转换，避免大图片在半像素处被精度误差错误舍入。
            let numerator = whole
                .checked_mul(scale)
                .and_then(|value| value.checked_add(fraction))
                .context("缩放百分比过大")?;
            ensure!(numerator > 0, "缩放百分比必须大于 0");
            Geometry::Percentage {
                numerator,
                denominator: 100_u128.checked_mul(scale).context("缩放百分比分母溢出")?,
            }
        } else {
            let (width, height) = value
                .split_once('x')
                .context("缩放尺寸必须使用 WxH、Wx、xH 或 P% 格式")?;
            match (width.is_empty(), height.is_empty()) {
                (false, false) => Geometry::Fit {
                    width: parse_dimension(width)?,
                    height: parse_dimension(height)?,
                },
                (false, true) => Geometry::Width(parse_dimension(width)?),
                (true, false) => Geometry::Height(parse_dimension(height)?),
                (true, true) => anyhow::bail!("缩放尺寸至少需要指定一条边"),
            }
        };
        Ok(Self {
            requested: value.to_owned(),
            geometry,
        })
    }
}

impl fmt::Display for ResizeSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.requested)
    }
}

impl ResizeSpec {
    /// 计算等比缩放后的整数像素尺寸，允许放大，且每条边至少为 1 像素。
    pub fn dimensions(&self, width: u64, height: u64) -> Result<(u64, u64)> {
        ensure!(width > 0 && height > 0, "源图片宽高必须大于 0");
        match self.geometry {
            Geometry::Fit {
                width: target_width,
                height: target_height,
            } => {
                // 比较 target_width / width 与 target_height / height，
                // 采用 checked u128 交叉乘法，避免浮点比较和 u64 中间溢出。
                let width_ratio = u128::from(target_width)
                    .checked_mul(u128::from(height))
                    .context("缩放尺寸比较溢出")?;
                let height_ratio = u128::from(target_height)
                    .checked_mul(u128::from(width))
                    .context("缩放尺寸比较溢出")?;
                if width_ratio <= height_ratio {
                    Ok((
                        target_width,
                        scaled_dimension(height, u128::from(target_width), u128::from(width))?
                            .min(target_height),
                    ))
                } else {
                    Ok((
                        scaled_dimension(width, u128::from(target_height), u128::from(height))?
                            .min(target_width),
                        target_height,
                    ))
                }
            }
            Geometry::Width(target_width) => Ok((
                target_width,
                scaled_dimension(height, u128::from(target_width), u128::from(width))?,
            )),
            Geometry::Height(target_height) => Ok((
                scaled_dimension(width, u128::from(target_height), u128::from(height))?,
                target_height,
            )),
            Geometry::Percentage {
                numerator,
                denominator,
            } => Ok((
                scaled_dimension(width, numerator, denominator)?,
                scaled_dimension(height, numerator, denominator)?,
            )),
        }
    }
}

fn parse_digits(value: &str) -> Result<u128> {
    ensure!(
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        "缩放尺寸与百分比仅允许 ASCII 十进制数字"
    );
    value.parse().context("缩放数值过大")
}

fn parse_dimension(value: &str) -> Result<u64> {
    let dimension = parse_digits(value)?;
    ensure!(
        dimension > 0 && dimension <= u128::from(u32::MAX),
        "缩放目标宽高必须在 1 至 {} 之间",
        u32::MAX
    );
    Ok(dimension as u64)
}

fn scaled_dimension(source: u64, numerator: u128, denominator: u128) -> Result<u64> {
    let product = u128::from(source)
        .checked_mul(numerator)
        .context("缩放尺寸计算溢出")?;
    let quotient = product / denominator;
    let remainder = product % denominator;
    // 使用余数判断四舍五入，恰好半像素时向上取整。
    // remainder >= denominator - remainder 等价于 2 * remainder >= denominator，
    // 但不会在翻倍余数或给 product 加半个分母时发生额外溢出。
    let rounded = quotient
        .checked_add(u128::from(remainder >= denominator - remainder))
        .context("缩放尺寸舍入溢出")?
        .max(1);
    ensure!(
        rounded <= u128::from(u32::MAX),
        "缩放后的宽高不能超过 {} 像素",
        u32::MAX
    );
    Ok(rounded as u64)
}

#[derive(Debug, Serialize, Clone)]
pub struct ResizeReport {
    pub requested: String,
    pub source_width: u64,
    pub source_height: u64,
    pub width: u64,
    pub height: u64,
    pub changed: bool,
    pub filter: String,
}

#[cfg(test)]
mod tests {
    use super::ResizeSpec;

    #[test]
    fn fits_landscape_and_portrait_inside_the_requested_box() {
        let spec: ResizeSpec = "300x200".parse().unwrap();
        assert_eq!(spec.dimensions(1200, 600).unwrap(), (300, 150));
        assert_eq!(spec.dimensions(600, 1200).unwrap(), (100, 200));
        assert_eq!(spec.dimensions(3, 2).unwrap(), (300, 200));
    }

    #[test]
    fn single_axis_scaling_rounds_half_up_and_allows_upscaling() {
        assert_eq!(
            "2x".parse::<ResizeSpec>()
                .unwrap()
                .dimensions(4, 3)
                .unwrap(),
            (2, 2)
        );
        assert_eq!(
            "x2".parse::<ResizeSpec>()
                .unwrap()
                .dimensions(3, 4)
                .unwrap(),
            (2, 2)
        );
        assert_eq!(
            "12x"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(4, 3)
                .unwrap(),
            (12, 9)
        );
        assert_eq!(
            "x12"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(3, 4)
                .unwrap(),
            (9, 12)
        );
    }

    #[test]
    fn percentage_scaling_uses_exact_decimal_arithmetic() {
        for (requested, source, expected) in [
            ("50%", (5, 3), (3, 2)),
            ("12.5%", (120, 72), (15, 9)),
            ("150%", (3, 2), (5, 3)),
            ("33.3333%", (3_000_000, 6_000_000), (999_999, 1_999_998)),
            ("0.0001%", (1, 1), (1, 1)),
            (
                "100%",
                (4_294_967_295, 4_294_967_295),
                (4_294_967_295, 4_294_967_295),
            ),
        ] {
            let spec: ResizeSpec = requested.parse().unwrap();
            assert_eq!(
                spec.dimensions(source.0, source.1).unwrap(),
                expected,
                "{requested}"
            );
        }
    }

    #[test]
    fn valid_specifications_round_trip_for_reports() {
        for requested in [
            "300x200",
            "300x",
            "x200",
            "12.5000%",
            "001x02",
            "4294967295x",
        ] {
            let spec: ResizeSpec = requested.parse().unwrap();
            assert_eq!(spec.to_string(), requested);
        }
    }

    #[test]
    fn rejects_invalid_numbers_geometry_flags_and_command_fragments() {
        for requested in [
            "",
            "x",
            "300",
            "0x20",
            "20x0",
            "0x",
            "x0",
            "0%",
            "0.0000%",
            "-1x2",
            "1x-2",
            "-10%",
            "+10%",
            "NaN%",
            "inf%",
            "Infinity%",
            "1e2%",
            ".5%",
            "5.%",
            "1.00001%",
            "1.2.3%",
            "1%%",
            "1%x2",
            "1X2",
            "1×2",
            "1.5x2",
            "1x2x3",
            " 1x2",
            "1x2 ",
            "1 x2",
            "1x2\n",
            "1\tx2",
            "1x2!",
            "1x2^",
            "1x2>",
            "1x2<",
            "1x2+0+0",
            "1x2;touch /tmp/pwned",
            "$(id)x2",
            "@geometry.txt",
            "１２x２",
            "4294967296x1",
            "1x4294967296",
            "340282366920938463463374607431768211456%",
        ] {
            assert!(
                requested.parse::<ResizeSpec>().is_err(),
                "应拒绝：{requested:?}"
            );
        }
    }

    #[test]
    fn output_is_at_least_one_pixel_and_fit_never_exceeds_its_box() {
        for (requested, source, expected) in [
            ("1x", (1000, 1), (1, 1)),
            ("x1", (1, 1000), (1, 1)),
            ("1x1", (1000, 1), (1, 1)),
            ("1x1", (1, 1000), (1, 1)),
            ("2x3", (4, 5), (2, 3)),
            ("3x2", (5, 4), (3, 2)),
        ] {
            assert_eq!(
                requested
                    .parse::<ResizeSpec>()
                    .unwrap()
                    .dimensions(source.0, source.1)
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn rejects_zero_source_dimensions_and_output_overflow() {
        let unchanged: ResizeSpec = "100%".parse().unwrap();
        for source in [(0, 1), (1, 0), (0, 0), (u64::MAX, 1), (1, u64::MAX)] {
            assert!(unchanged.dimensions(source.0, source.1).is_err());
        }
        assert!(
            "4294967295x"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(1, 2)
                .is_err()
        );
        assert!(
            "x4294967295"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(2, 1)
                .is_err()
        );
        assert!(
            "200%"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(u32::MAX as u64, 1)
                .is_err()
        );
    }

    #[test]
    fn large_sources_are_reduced_without_intermediate_u64_overflow() {
        assert_eq!(
            "4294967295x4294967295"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(u64::MAX, u64::MAX)
                .unwrap(),
            (4_294_967_295, 4_294_967_295)
        );
        assert_eq!(
            "4294967295x"
                .parse::<ResizeSpec>()
                .unwrap()
                .dimensions(u64::MAX, u64::MAX)
                .unwrap(),
            (4_294_967_295, 4_294_967_295)
        );
    }

    #[test]
    fn extreme_percentage_returns_error_instead_of_panicking() {
        let spec: ResizeSpec = "340282366920938463463374607431768211455%".parse().unwrap();
        assert!(spec.dimensions(u64::MAX, u64::MAX).is_err());
        assert!(
            "340282366920938463463374607431768211455.1%"
                .parse::<ResizeSpec>()
                .is_err()
        );
    }
}
