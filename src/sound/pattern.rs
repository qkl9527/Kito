//! 把用户熟悉的场景、音色与节奏格转换为合成声部。
//! 预设只是一组公开、可重复的合成参数，不读取素材，也不依赖采样包。
use super::synth::{Instrument, Voice};
use anyhow::{Result, ensure};
use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Timbre {
    /// 干净的正弦音，适合校音和测试。
    Sine,
    /// 柔和的合成音，适合旋律与和弦。
    Soft,
    /// 明亮的钟铃合成音。
    Bell,
    /// 短促的拨弦合成音。
    Pluck,
    /// 复古游戏风格的电子音。
    Chip,
}
impl Timbre {
    pub fn instrument(self) -> Instrument {
        match self {
            Self::Sine => Instrument::Sine,
            Self::Soft => Instrument::Soft,
            Self::Bell => Instrument::Bell,
            Self::Pluck => Instrument::Pluck,
            Self::Chip => Instrument::Chip,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// 清脆的通知声。
    Notify,
    /// 上行的成功提示。
    Success,
    /// 低沉的错误提示。
    Error,
    /// 复古游戏收集金币声。
    Coin,
    /// 音高上扬的跳跃声。
    Jump,
    /// 音高下降的激光声。
    Laser,
    /// 噪声和低频组成的爆炸声。
    Explosion,
    /// 渐强后消退的掠过声。
    Whoosh,
}
impl Effect {
    pub fn name(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Success => "success",
            Self::Error => "error",
            Self::Coin => "coin",
            Self::Jump => "jump",
            Self::Laser => "laser",
            Self::Explosion => "explosion",
            Self::Whoosh => "whoosh",
        }
    }
    pub fn duration(self) -> f64 {
        match self {
            Self::Notify | Self::Jump => 0.45,
            Self::Success => 0.9,
            Self::Error => 0.65,
            Self::Coin => 0.4,
            Self::Laser => 0.35,
            Self::Explosion => 1.2,
            Self::Whoosh => 0.8,
        }
    }
}

pub fn effect(effect: Effect, duration: f64, pitch: f64) -> Result<Vec<Voice>> {
    ensure!(
        duration.is_finite() && (0.05..=10.0).contains(&duration),
        "音效时长必须在 0.05–10 秒之间"
    );
    ensure!(
        pitch.is_finite() && (-24.0..=24.0).contains(&pitch),
        "--pitch 必须在 -24–24 半音之间"
    );
    let ratio = 2.0_f64.powf(pitch / 12.0);
    let voice = |start, length, hz, end_hz, gain, instrument| Voice {
        start: start * duration,
        duration: length * duration,
        frequency: hz * ratio,
        end_frequency: end_hz * ratio,
        gain,
        instrument,
    };
    use Instrument::*;
    Ok(match effect {
        Effect::Notify => vec![
            voice(0.0, 0.8, 1046.502, 1046.502, 0.75, Bell),
            voice(0.2, 0.8, 1318.51, 1318.51, 0.5, Bell),
        ],
        Effect::Success => [523.251, 659.255, 783.991, 1046.502]
            .iter()
            .enumerate()
            .map(|(i, &f)| voice(i as f64 * 0.2, 0.4, f, f, 0.65, Bell))
            .collect(),
        Effect::Error => vec![
            voice(0.0, 0.5, 220.0, 220.0, 0.8, Soft),
            voice(0.4, 0.6, 174.614, 174.614, 0.8, Soft),
        ],
        Effect::Coin => vec![
            voice(0.0, 0.45, 987.767, 987.767, 0.65, Chip),
            voice(0.3, 0.7, 1318.51, 1318.51, 0.65, Chip),
        ],
        Effect::Jump => vec![voice(0.0, 1.0, 240.0, 800.0, 0.8, Chip)],
        Effect::Laser => vec![voice(0.0, 1.0, 1400.0, 90.0, 0.8, Soft)],
        Effect::Explosion => vec![
            voice(0.0, 1.0, 0.0, 0.0, 0.8, Noise),
            voice(0.0, 0.5, 100.0, 35.0, 0.9, Kick),
        ],
        Effect::Whoosh => (0..5)
            .map(|i| {
                let gain = [0.12, 0.28, 0.5, 0.28, 0.12][i];
                voice(i as f64 * 0.15, 0.4, 0.0, 0.0, gain, Noise)
            })
            .collect(),
    })
}

#[derive(Clone, Copy, Debug, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum BeatPreset {
    /// 四拍底鼓、二四拍军鼓和八分音符踩镲。
    Basic,
    /// 四拍底鼓和反拍踩镲。
    House,
    /// 带切分底鼓的简短 Hip-hop 节奏。
    Hiphop,
}
impl BeatPreset {
    pub fn patterns(self) -> [&'static str; 3] {
        match self {
            Self::Basic => ["x---x---x---x---", "----x-------x---", "x-x-x-x-x-x-x-x-"],
            Self::House => ["x---x---x---x---", "----x-------x---", "--x---x---x---x-"],
            Self::Hiphop => ["x-----x-x-------", "----x-------x---", "x-x-x-x-x-x-x-x-"],
        }
    }
}

pub fn beat(patterns: [&str; 3], bpm: u16, bars: u16) -> Result<(Vec<Voice>, f64)> {
    ensure!((20..=400).contains(&bpm), "BPM 必须在 20–400 之间");
    ensure!((1..=64).contains(&bars), "--bars 必须在 1–64 之间");
    for pattern in patterns {
        ensure!(
            pattern.len() == 16
                && pattern
                    .bytes()
                    .all(|b| matches!(b, b'x' | b'X' | b'-' | b'.')),
            "鼓点模式必须恰好 16 格：x/X 表示敲击，- 或 . 表示休止（不含空格）"
        );
    }
    let step = 60.0 / f64::from(bpm) / 4.0;
    // 用整数比较恰好 120 秒的边界，再一次除法计算时长，避免 step 连乘累积尾差。
    ensure!(
        240 * u32::from(bars) <= 120 * u32::from(bpm),
        "鼓点总时长不能超过 120 秒，请减少 --bars 或提高 BPM"
    );
    let duration = f64::from(bars) * 240.0 / f64::from(bpm);
    let mut voices = Vec::new();
    for bar in 0..bars {
        for (track, pattern) in patterns.iter().enumerate() {
            for (slot, mark) in pattern.bytes().enumerate() {
                if matches!(mark, b'x' | b'X') {
                    let start = (usize::from(bar) * 16 + slot) as f64 * step;
                    // 尾声限制在小节范围内，确保 bars 和 BPM 定义的循环长度准确。
                    let (instrument, length, gain) = match track {
                        0 => (Instrument::Kick, 0.25_f64, 0.9),
                        1 => (Instrument::Snare, 0.18_f64, 0.65),
                        _ => (Instrument::Hat, 0.06_f64, 0.3),
                    };
                    voices.push(Voice {
                        start,
                        duration: length.min(duration - start),
                        frequency: 0.0,
                        end_frequency: 0.0,
                        gain,
                        instrument,
                    });
                }
            }
        }
    }
    Ok((voices, duration))
}
