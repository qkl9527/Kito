//! 内置离线合成器：只生成新声音，输出 PCM16 单声道 WAV。

use std::f64::consts::{PI, TAU};

use anyhow::{Context, Result, ensure};

const MAX_VOICES: usize = 4_096;
const MAX_VOICE_FRAMES: usize = 64_000_000;
const PEAK_CEILING: f64 = 0.95;

#[derive(Clone, Copy, Debug)]
pub enum Instrument {
    Sine,
    Soft,
    Bell,
    Pluck,
    Chip,
    Kick,
    Snare,
    Hat,
    Noise,
}

#[derive(Clone, Copy, Debug)]
pub struct Voice {
    pub start: f64,
    pub duration: f64,
    pub frequency: f64,
    pub end_frequency: f64,
    pub gain: f64,
    pub instrument: Instrument,
}

#[derive(Debug)]
pub struct Rendered {
    pub wav: Vec<u8>,
    pub frames: usize,
    pub peak: f64,
    pub normalization_gain: f64,
}

/// 只检查参数与渲染工作量，不分配音频缓冲区，供 dry-run 与真实渲染共用。
///
/// 总时长允许短于 0.01 秒，以容纳高 BPM 下合法的 1/16 拍；至少要能舍入为一帧。
pub fn validate(voices: &[Voice], duration: f64, sample_rate: u32, volume: f64) -> Result<usize> {
    ensure!(
        matches!(sample_rate, 22_050 | 44_100 | 48_000),
        "采样率只支持 22050、44100 或 48000 Hz"
    );
    ensure!(
        duration.is_finite() && duration > 0.0 && duration <= 120.0,
        "声音总时长必须大于 0 且不超过 120 秒"
    );
    ensure!(
        volume.is_finite() && (0.0..=1.0).contains(&volume),
        "音量必须是 0..1 内的有限数值"
    );
    ensure!(voices.len() <= MAX_VOICES, "声音事件不能超过 4096 个");

    let rate = f64::from(sample_rate);
    let frames = (duration * rate).round() as usize;
    ensure!(frames > 0, "声音时长过短，无法生成一个采样帧");
    let frequency_limit = rate * 0.45;
    let mut work_frames = 0usize;
    for (index, voice) in voices.iter().enumerate() {
        ensure!(
            voice.start.is_finite()
                && voice.duration.is_finite()
                && voice.frequency.is_finite()
                && voice.end_frequency.is_finite()
                && voice.gain.is_finite(),
            "第 {} 个声音事件包含非有限数值",
            index + 1
        );
        ensure!(
            voice.start >= 0.0 && voice.duration > 0.0,
            "第 {} 个声音事件的开始时间不能为负，时长必须大于 0",
            index + 1
        );
        // 容忍时间求和的浮点尾差；索引仍被限制在真实输出帧数内。
        ensure!(
            voice.start + voice.duration <= duration + 1e-9,
            "第 {} 个声音事件超出输出总时长",
            index + 1
        );
        ensure!(
            (0.0..=1.0).contains(&voice.gain),
            "第 {} 个声音事件的增益必须在 0..1 内",
            index + 1
        );
        let pitched = matches!(
            voice.instrument,
            Instrument::Sine
                | Instrument::Soft
                | Instrument::Bell
                | Instrument::Pluck
                | Instrument::Chip
        );
        ensure!(
            [voice.frequency, voice.end_frequency]
                .iter()
                .all(|frequency| {
                    *frequency <= frequency_limit
                        && if pitched {
                            *frequency > 0.0
                        } else {
                            *frequency >= 0.0
                        }
                }),
            "第 {} 个声音事件的频率必须{}且不超过 {frequency_limit} Hz（采样率的 45%）",
            index + 1,
            if pitched { "大于 0" } else { "不小于 0" }
        );
        let (start, end) = voice_range(voice, rate, frames);
        work_frames += end - start;
        ensure!(
            work_frames <= MAX_VOICE_FRAMES,
            "累计声部采样量不能超过 6400 万帧，请减少重叠声音或缩短时长"
        );
    }
    Ok(frames)
}

/// 在浮点缓冲区混音，统一衰减后一次量化为 PCM16，避免逐声部削波失真。
pub fn render(
    voices: &[Voice],
    duration: f64,
    sample_rate: u32,
    volume: f64,
    seed: u64,
) -> Result<Rendered> {
    let frames = validate(voices, duration, sample_rate, volume)?;
    let mut mix = Vec::<f32>::new();
    mix.try_reserve_exact(frames)
        .context("无法分配声音混音缓冲区")?;
    mix.resize(frames, 0.0);
    let rate = f64::from(sample_rate);
    for (index, voice) in voices.iter().enumerate() {
        if voice.gain == 0.0 {
            continue;
        }
        let (start, end) = voice_range(voice, rate, frames);
        // 只遍历该事件覆盖的片段，休止和远离事件的时间不会重复计算。
        let event_seed = seed.wrapping_add((index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        render_voice(&mut mix[start..end], voice, rate, event_seed);
    }
    // 即使以后增加新乐器，文件本身仍明确保证首尾为零。
    mix[0] = 0.0;
    mix[frames - 1] = 0.0;
    let mix_peak = mix
        .iter()
        .map(|value| f64::from(*value).abs())
        .fold(0.0, f64::max);
    let normalization_gain = if mix_peak > PEAK_CEILING {
        PEAK_CEILING / mix_peak
    } else {
        1.0
    };

    // 上限 120 秒 × 48000 Hz × 2 字节，已在 validate 中保证 RIFF 32 位长度安全。
    let data_bytes = (frames * 2) as u32;
    let mut wav = Vec::new();
    wav.try_reserve_exact(44 + data_bytes as usize)
        .context("无法分配 WAV 输出缓冲区")?;
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes.to_le_bytes());
    let mut peak = 0.0f64;
    let output_gain = normalization_gain * volume;
    for sample in mix {
        // 峰值已经统一压到 0.95；此处无需 clamp，不会把过载声波硬削成平顶。
        let pcm = (f64::from(sample) * output_gain * f64::from(i16::MAX)).round() as i16;
        peak = peak.max(f64::from(pcm).abs() / f64::from(i16::MAX));
        wav.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(Rendered {
        wav,
        frames,
        peak,
        normalization_gain,
    })
}

fn voice_range(voice: &Voice, rate: f64, frames: usize) -> (usize, usize) {
    let start = ((voice.start * rate).round() as usize).min(frames);
    let end = (((voice.start + voice.duration) * rate).round() as usize).min(frames);
    (start, end)
}

/// 倍频、幅度、衰减速率；谐波会按整个扫频区间的最高频率预先裁剪。
struct Partial {
    multiple: f64,
    weight: f64,
    decay: f64,
}

fn partials(instrument: Instrument, highest_frequency: f64, rate: f64) -> Vec<Partial> {
    let mut result = Vec::new();
    let mut add = |multiple, weight, decay| {
        if highest_frequency * multiple <= rate * 0.45 {
            result.push(Partial {
                multiple,
                weight,
                decay,
            });
        }
    };
    match instrument {
        Instrument::Sine => add(1.0, 1.0, 0.0),
        Instrument::Soft => {
            add(1.0, 0.8, 0.0);
            add(2.0, 0.15, 0.0);
            add(3.0, 0.05, 0.0);
        }
        Instrument::Bell => {
            add(1.0, 0.6, 3.0);
            add(2.005, 0.25, 5.0);
            add(2.99, 0.15, 7.0);
        }
        Instrument::Pluck => {
            for harmonic in 1..=6 {
                let harmonic = f64::from(harmonic);
                add(harmonic, 1.0 / harmonic.powi(2), 2.0 + harmonic);
            }
        }
        Instrument::Chip => {
            // 通过有限奇次谐波重建方波；不用 sign(sin)，防止高次谐波折回可听频段。
            for harmonic in (1..=31).step_by(2) {
                let harmonic = f64::from(harmonic);
                add(harmonic, 4.0 / (PI * harmonic), 0.0);
            }
        }
        Instrument::Kick | Instrument::Snare | Instrument::Hat | Instrument::Noise => {}
    }
    result
}

fn render_voice(mix: &mut [f32], voice: &Voice, rate: f64, seed: u64) {
    if mix.len() < 3 {
        return;
    }
    let mut random = Noise(seed);
    let mut previous_noise = 0.0;
    let last = (mix.len() - 1) as f64;
    let attack_frames = (rate * 0.005).min(last * 0.25).max(1.0);
    let release_frames = (rate * 0.03).min(last / 3.0).max(1.0);
    let partials = partials(
        voice.instrument,
        voice.frequency.max(voice.end_frequency),
        rate,
    );
    // 打击乐允许 frequency=0；Kick 的零值选择内置鼓频，其他打击乐用其专有音色。
    let (frequency, end_frequency) = if matches!(voice.instrument, Instrument::Kick) {
        (
            if voice.frequency == 0.0 {
                160.0
            } else {
                voice.frequency
            },
            if voice.end_frequency == 0.0 {
                45.0
            } else {
                voice.end_frequency
            },
        )
    } else {
        (voice.frequency, voice.end_frequency)
    };
    for (index, mixed) in mix.iter_mut().enumerate() {
        let position = index as f64;
        let elapsed = position / rate;
        let progress = position / last;
        // 频率必须先积分得到相位；直接用当前频率乘时间会令线性扫频速度翻倍。
        let phase = TAU
            * (frequency * elapsed
                + 0.5 * (end_frequency - frequency) * elapsed * (elapsed / voice.duration));
        let sample = match voice.instrument {
            Instrument::Kick => phase.sin() * (-7.0 * progress).exp(),
            Instrument::Snare => {
                (0.8 * random.next() + 0.2 * (TAU * 180.0 * elapsed).sin())
                    * (-8.0 * progress).exp()
            }
            Instrument::Hat => {
                let noise = random.next();
                let high_passed = (noise - previous_noise) * 0.5;
                previous_noise = noise;
                high_passed * (-12.0 * progress).exp()
            }
            Instrument::Noise => random.next(),
            _ => partials
                .iter()
                .map(|partial| {
                    (phase * partial.multiple).sin()
                        * partial.weight
                        * (-partial.decay * progress).exp()
                })
                .sum(),
        };
        // 两端使用斜率为零的 smoothstep；短声音自动缩短包络，不让尾音越过事件时长。
        let envelope =
            smoothstep(position / attack_frames) * smoothstep((last - position) / release_frames);
        *mixed += (sample * envelope * voice.gain) as f32;
    }
}

fn smoothstep(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

/// SplitMix64 不以零为吸收态；固定 seed 与声部顺序会生成可重复的噪声。
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        ((value >> 11) as f64 / 9_007_199_254_740_992.0) * 2.0 - 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(duration: f64) -> Voice {
        Voice {
            start: 0.0,
            duration,
            frequency: 440.0,
            end_frequency: 440.0,
            gain: 1.0,
            instrument: Instrument::Sine,
        }
    }

    fn samples(rendered: &Rendered) -> Vec<i16> {
        assert!(rendered.wav.len() >= 44, "必须包含完整 WAV 头");
        rendered.wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes(*bytes))
            .collect()
    }

    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn wav_header_and_payload_describe_real_pcm16_mono_samples() {
        let output = render(&[tone(1.0)], 1.0, 44_100, 0.7, 0).unwrap();
        assert_eq!(output.frames, 44_100);
        assert_eq!(output.wav.len(), 88_244);
        assert_eq!(&output.wav[..4], b"RIFF");
        assert_eq!(word(&output.wav, 4), 88_236);
        assert_eq!(&output.wav[8..16], b"WAVEfmt ");
        assert_eq!(word(&output.wav, 16), 16);
        assert_eq!(&output.wav[20..24], &[1, 0, 1, 0]);
        assert_eq!(word(&output.wav, 24), 44_100);
        assert_eq!(word(&output.wav, 28), 88_200);
        assert_eq!(&output.wav[32..36], &[2, 0, 16, 0]);
        assert_eq!(&output.wav[36..40], b"data");
        assert_eq!(word(&output.wav, 40), 88_200);
        let pcm = samples(&output);
        let peak = pcm.iter().map(|v| f64::from(*v).abs()).fold(0.0, f64::max) / 32_767.0;
        assert_eq!(output.peak, peak);
        assert!(peak > 0.5 && peak < 0.7);
    }

    #[test]
    fn sine_retains_440_hz_pitch() {
        let output = render(&[tone(1.0)], 1.0, 44_100, 0.7, 0).unwrap();
        let pcm = samples(&output);
        let crossings = pcm[4_410..39_690]
            .windows(2)
            .filter(|pair| pair[0] <= 0 && pair[1] > 0)
            .count();
        // 中间 0.8 秒应有 352 个周期，允许切片端点带来 1 个差异。
        assert!(
            (351..=353).contains(&crossings),
            "实际正向过零数：{crossings}"
        );
    }

    #[test]
    fn sweep_integrates_frequency_instead_of_multiplying_instantaneous_frequency() {
        let mut voice = tone(1.0);
        voice.frequency = 100.0;
        voice.end_frequency = 900.0;
        let output = render(&[voice], 1.0, 48_000, 0.7, 0).unwrap();
        let pcm = samples(&output);
        let crossings = pcm[24_000..36_000]
            .windows(2)
            .filter(|pair| pair[0] <= 0 && pair[1] > 0)
            .count();
        // 0.5..0.75 秒的平均频率为 600 Hz，共 150 个周期。
        assert!(
            (149..=151).contains(&crossings),
            "实际扫频周期数：{crossings}"
        );
    }

    #[test]
    fn every_instrument_fades_to_zero_at_voice_and_file_boundaries() {
        for instrument in [
            Instrument::Sine,
            Instrument::Soft,
            Instrument::Bell,
            Instrument::Pluck,
            Instrument::Chip,
            Instrument::Kick,
            Instrument::Snare,
            Instrument::Hat,
            Instrument::Noise,
        ] {
            let mut voice = tone(0.05);
            voice.start = 0.025;
            voice.instrument = instrument;
            let output = render(&[voice], 0.1, 48_000, 0.7, 0).unwrap();
            let pcm = samples(&output);
            assert_eq!(pcm.len(), 4_800);
            assert!(
                pcm[..1_201].iter().all(|value| *value == 0),
                "{instrument:?}"
            );
            assert!(
                pcm[3_599..].iter().all(|value| *value == 0),
                "{instrument:?}"
            );
            assert!(
                pcm[1_201..3_599].iter().any(|value| *value != 0),
                "{instrument:?}"
            );
            assert!(
                i32::from(pcm[1_201]).abs() < 100,
                "起音应平滑：{instrument:?}"
            );
            assert!(
                i32::from(pcm[3_598]).abs() < 100,
                "收尾应平滑：{instrument:?}"
            );
        }
        let output = render(&[tone(0.01)], 0.01, 22_050, 1.0, 0).unwrap();
        let pcm = samples(&output);
        assert_eq!(pcm.first(), Some(&0));
        assert_eq!(pcm.last(), Some(&0));
    }

    #[test]
    fn mixing_preserves_shape_by_uniform_attenuation_without_clipping() {
        let single = render(&[tone(0.1)], 0.1, 44_100, 0.8, 0).unwrap();
        let chord = render(&[tone(0.1); 16], 0.1, 44_100, 0.8, 0).unwrap();
        assert!(chord.normalization_gain > 0.0 && chord.normalization_gain < 0.07);
        assert!((single.normalization_gain / chord.normalization_gain - 16.0).abs() < 0.0001);
        assert!(chord.peak <= 0.95 * 0.8 + 1.0 / 32_767.0);
        let one = samples(&single);
        let many = samples(&chord);
        assert!(
            many.iter()
                .all(|sample| *sample != i16::MAX && *sample != i16::MIN)
        );
        assert!(
            one.iter()
                .zip(many.iter())
                .all(|(a, b)| (i32::from(*a) - i32::from(*b)).abs() <= 1)
        );
    }

    #[test]
    fn noise_seed_is_reproducible_and_zero_is_not_a_stuck_state() {
        let mut voice = tone(0.05);
        voice.instrument = Instrument::Noise;
        voice.frequency = 0.0;
        voice.end_frequency = 0.0;
        let first = render(&[voice], 0.05, 22_050, 0.7, 0).unwrap();
        let again = render(&[voice], 0.05, 22_050, 0.7, 0).unwrap();
        let different = render(&[voice], 0.05, 22_050, 0.7, 1).unwrap();
        assert_eq!(first.wav, again.wav);
        assert_ne!(first.wav, different.wav);
        assert!(samples(&first).iter().any(|sample| *sample != 0));
    }

    #[test]
    fn silence_and_zero_volume_are_valid_with_exact_rounded_frames() {
        let output = render(&[], 0.010_01, 22_050, 1.0, 0).unwrap();
        assert_eq!(output.frames, 221);
        assert_eq!(output.peak, 0.0);
        assert_eq!(output.normalization_gain, 1.0);
        assert!(samples(&output).iter().all(|value| *value == 0));
        let muted = render(&[tone(0.1)], 0.1, 48_000, 0.0, 0).unwrap();
        assert_eq!(muted.frames, 4_800);
        assert_eq!(muted.peak, 0.0);
        assert!(samples(&muted).iter().all(|value| *value == 0));
    }

    #[test]
    fn validation_rejects_non_finite_and_out_of_range_global_inputs() {
        for duration in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            -1.0,
            0.0,
            f64::MIN_POSITIVE,
            120.001,
            f64::MAX,
        ] {
            assert!(
                validate(&[], duration, 44_100, 0.7).is_err(),
                "duration={duration}"
            );
            assert!(
                render(&[], duration, 44_100, 0.7, 0).is_err(),
                "duration={duration}"
            );
        }
        for volume in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
            assert!(
                validate(&[], 1.0, 44_100, volume).is_err(),
                "volume={volume}"
            );
        }
        for sample_rate in [0, 8_000, 22_049, 44_101, 96_000, u32::MAX] {
            assert!(
                validate(&[], 1.0, sample_rate, 0.7).is_err(),
                "sample_rate={sample_rate}"
            );
        }
        assert_eq!(validate(&[], 120.0, 48_000, 1.0).unwrap(), 5_760_000);
        assert_eq!(
            validate(&[tone(0.009_375)], 0.009_375, 22_050, 0.7).unwrap(),
            207
        );
    }

    #[test]
    fn validation_rejects_invalid_voice_fields_before_allocation() {
        for field in 0..6 {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut voice = tone(1.0);
                match field {
                    0 => voice.start = value,
                    1 => voice.duration = value,
                    2 => voice.frequency = value,
                    3 => voice.end_frequency = value,
                    4 => voice.gain = value,
                    _ => {
                        voice.instrument = Instrument::Noise;
                        voice.frequency = value;
                    }
                }
                assert!(
                    validate(&[voice], 1.0, 44_100, 0.7).is_err(),
                    "field={field}, value={value}"
                );
            }
        }
        let mut voice = tone(1.0);
        voice.start = -0.01;
        assert!(validate(&[voice], 1.0, 44_100, 0.7).is_err());
        voice = tone(0.0);
        assert!(validate(&[voice], 1.0, 44_100, 0.7).is_err());
        voice = tone(1.01);
        assert!(validate(&[voice], 1.0, 44_100, 0.7).is_err());
        for gain in [-0.1, 1.1] {
            voice = tone(1.0);
            voice.gain = gain;
            assert!(validate(&[voice], 1.0, 44_100, 0.7).is_err());
        }
    }

    #[test]
    fn validation_checks_both_sweep_endpoints_against_the_band_limit() {
        for instrument in [
            Instrument::Sine,
            Instrument::Soft,
            Instrument::Bell,
            Instrument::Pluck,
            Instrument::Chip,
        ] {
            for frequency in [-1.0, 0.0, 9_923.0] {
                for at_end in [false, true] {
                    let mut voice = tone(0.01);
                    voice.instrument = instrument;
                    if at_end {
                        voice.end_frequency = frequency;
                    } else {
                        voice.frequency = frequency;
                    }
                    assert!(
                        validate(&[voice], 0.01, 22_050, 0.7).is_err(),
                        "{instrument:?}, {frequency}, end={at_end}"
                    );
                }
            }
            let mut voice = tone(0.01);
            voice.instrument = instrument;
            voice.frequency = 9_922.5;
            voice.end_frequency = 9_922.5;
            assert!(render(&[voice], 0.01, 22_050, 0.7, 0).is_ok());
        }
    }

    #[test]
    fn voice_count_and_total_voice_samples_are_bounded() {
        let voices = vec![tone(0.01); 4_096];
        assert_eq!(validate(&voices, 0.01, 22_050, 0.7).unwrap(), 221);
        assert!(validate(&vec![tone(0.01); 4_097], 0.01, 22_050, 0.7).is_err());
        assert!(validate(&[tone(120.0); 12], 120.0, 48_000, 0.7).is_err());
        assert!(validate(&[tone(120.0); 8], 120.0, 48_000, 0.7).is_ok());
    }

    #[test]
    fn high_chip_pitch_has_no_alias_from_out_of_band_harmonics() {
        let mut voice = tone(0.1);
        voice.frequency = 8_000.0;
        voice.end_frequency = 8_000.0;
        let sine = render(&[voice], 0.1, 22_050, 0.7, 0).unwrap();
        voice.instrument = Instrument::Chip;
        let chip = render(&[voice], 0.1, 22_050, 0.7, 0).unwrap();
        // 三次谐波已越过 Nyquist；高音 chip 只能保留基频，不能折回为假低音。
        let sine_samples = samples(&sine);
        let chip_samples = samples(&chip);
        let dot: f64 = sine_samples
            .iter()
            .zip(&chip_samples)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum();
        let sine_energy: f64 = sine_samples
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum();
        let chip_energy: f64 = chip_samples
            .iter()
            .map(|sample| f64::from(*sample).powi(2))
            .sum();
        let correlation = dot / (sine_energy * chip_energy).sqrt();
        assert!(correlation > 0.99999, "高音相关度：{correlation}");
    }
}
