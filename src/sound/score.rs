//! 乐谱的整数 tick 表示与 Standard MIDI File type 0 编码。

use anyhow::{Context, Result, bail, ensure};

pub const TICKS_PER_BEAT: u32 = 480;
const MAX_INPUT_BYTES: usize = 64 * 1024;
const MAX_NOTE_EVENTS: usize = 4096;
const MAX_CHORD_NOTES: usize = 8;
const MAX_DURATION_SECONDS: u32 = 120;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteEvent {
    pub start_tick: u32,
    pub duration_ticks: u32,
    pub midi: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Score {
    pub events: Vec<NoteEvent>,
    pub total_ticks: u32,
    pub bpm: u16,
}

impl Score {
    pub fn duration_seconds(&self) -> f64 {
        f64::from(self.total_ticks) * 60.0 / (f64::from(self.bpm) * f64::from(TICKS_PER_BEAT))
    }
}

/// 将顺序事件展开成绝对 tick 时间线；一个和弦里的音共享起点和时长。
pub fn parse_score(text: &str, bpm: u16, repeat: u16) -> Result<Score> {
    ensure!(text.len() <= MAX_INPUT_BYTES, "乐谱输入最多 64 KiB");
    validate_tempo(bpm)?;
    ensure!((1..=64).contains(&repeat), "重复次数必须在 1..=64 之间");

    let mut events = Vec::new();
    let mut total_ticks = 0_u32;
    for token in text.split_whitespace().filter(|token| *token != "|") {
        let (notes, duration_ticks) = match token.split_once(':') {
            Some((notes, beats)) => (
                notes,
                parse_beats(beats).with_context(|| format!("无效的乐谱事件 {token:?}"))?,
            ),
            None => (token, TICKS_PER_BEAT),
        };
        let end_tick = total_ticks
            .checked_add(duration_ticks)
            .context("乐谱总 tick 数溢出")?;
        // 先限制时间线，再展开声部；纯休止输入也遵守相同资源限制。
        validate_duration(end_tick, bpm)?;
        if !notes.eq_ignore_ascii_case("R") {
            let mut chord = Vec::new();
            for note in notes.split('+') {
                ensure!(chord.len() < MAX_CHORD_NOTES, "一个和弦最多包含 8 个音");
                let midi = parse_note(note).with_context(|| format!("无效的乐谱事件 {token:?}"))?;
                ensure!(
                    !chord.contains(&midi),
                    "和弦中不能重复同一个音（包括等音音名）"
                );
                chord.push(midi);
            }
            ensure!(
                events.len() + chord.len() <= MAX_NOTE_EVENTS,
                "乐谱最多包含 4096 个发声音符"
            );
            events.extend(chord.into_iter().map(|midi| NoteEvent {
                start_tick: total_ticks,
                duration_ticks,
                midi,
            }));
        }
        total_ticks = end_tick;
    }
    ensure!(total_ticks > 0, "乐谱至少需要一个音符或休止符");

    let repeated_ticks = total_ticks
        .checked_mul(u32::from(repeat))
        .context("重复后的总 tick 数溢出")?;
    validate_duration(repeated_ticks, bpm)?;
    let repeated_count = events
        .len()
        .checked_mul(usize::from(repeat))
        .context("重复后的音符数量溢出")?;
    ensure!(
        repeated_count <= MAX_NOTE_EVENTS,
        "重复后的乐谱最多包含 4096 个发声音符"
    );

    // 所有上限均通过后才为重复序列分配空间，避免无效 repeat 放大内存占用。
    let mut repeated = Vec::with_capacity(repeated_count);
    for index in 0..u32::from(repeat) {
        let offset = index * total_ticks;
        repeated.extend(events.iter().map(|event| NoteEvent {
            start_tick: event.start_tick + offset,
            duration_ticks: event.duration_ticks,
            midi: event.midi,
        }));
    }
    Ok(Score {
        events: repeated,
        total_ticks: repeated_ticks,
        bpm,
    })
}

/// 接受 ASCII 音名、可选的单升降号、带负号的八度；A4 对应 MIDI 69。
pub fn parse_note(text: &str) -> Result<u8> {
    let bytes = text.as_bytes();
    ensure!(
        text.is_ascii() && bytes.len() >= 2,
        "音名应为 C4、F#4 或 Bb3 等 ASCII 形式"
    );
    let pitch = match bytes[0].to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => bail!("音名必须以 A..G 开头"),
    };
    let (accidental, octave_start) = match bytes[1] {
        b'#' => (1, 2),
        b'b' => (-1, 2),
        _ => (0, 1),
    };
    let octave_text = &text[octave_start..];
    let digits = octave_text.strip_prefix('-').unwrap_or(octave_text);
    ensure!(
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()),
        "音名必须包含整数八度，升降号只能出现一次"
    );
    let octave = octave_text.parse::<i16>().context("音名八度超出范围")?;
    let midi = (i32::from(octave) + 1) * 12 + pitch + accidental;
    ensure!((0..=127).contains(&midi), "音名必须落在 MIDI 0..127 范围内");
    Ok(midi as u8)
}

pub fn frequency(midi: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(midi) - 69.0) / 12.0)
}

/// 编码一个轨道，固定钢琴音色与力度；所有 note off 在同 tick 的 note on 前。
pub fn encode_midi(score: &Score) -> Result<Vec<u8>> {
    validate_score(score)?;
    let mut track = Vec::new();
    // SMF tempo 的单位是每四分音符的整数微秒；对不可整除 BPM 取最近微秒。
    let tempo = (60_000_000_u32 + u32::from(score.bpm) / 2) / u32::from(score.bpm);
    track.extend_from_slice(&[0, 0xff, 0x51, 3]);
    track.extend_from_slice(&tempo.to_be_bytes()[1..]);
    track.extend_from_slice(&[0, 0xff, 0x58, 4, 4, 2, 24, 8]);
    track.extend_from_slice(&[0, 0xc0, 0]);

    let mut timed_events = Vec::with_capacity(score.events.len() * 2);
    for event in &score.events {
        timed_events.push((event.start_tick, 0x90_u8, event.midi, 100_u8));
        timed_events.push((event.start_tick + event.duration_ticks, 0x80, event.midi, 0));
    }
    // 状态字节 0x80 < 0x90，自然得到「先关后开」；音高排序使和弦输出稳定。
    timed_events.sort_unstable();
    let mut previous_tick = 0;
    for (tick, status, midi, velocity) in timed_events {
        append_variable_length(tick - previous_tick, &mut track);
        track.extend_from_slice(&[status, midi, velocity]);
        previous_tick = tick;
    }
    // EOT 使用整个乐谱终点，不能使用最后一个 note off，否则会吞掉尾部休止。
    append_variable_length(score.total_ticks - previous_tick, &mut track);
    track.extend_from_slice(&[0xff, 0x2f, 0]);

    let track_len = u32::try_from(track.len()).context("MIDI 轨道过大")?;
    let mut bytes = Vec::with_capacity(22 + track.len());
    bytes.extend_from_slice(b"MThd\0\0\0\x06\0\0\0\x01");
    bytes.extend_from_slice(&(TICKS_PER_BEAT as u16).to_be_bytes());
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&track_len.to_be_bytes());
    bytes.extend_from_slice(&track);
    Ok(bytes)
}

fn validate_tempo(bpm: u16) -> Result<()> {
    ensure!((20..=400).contains(&bpm), "BPM 必须在 20..=400 之间");
    Ok(())
}

fn validate_duration(total_ticks: u32, bpm: u16) -> Result<()> {
    // 用交叉相乘比较时长，120 秒边界不受浮点舍入影响。
    ensure!(
        u64::from(total_ticks) * 60
            <= u64::from(MAX_DURATION_SECONDS) * u64::from(bpm) * u64::from(TICKS_PER_BEAT),
        "声音总时长最多 120 秒"
    );
    Ok(())
}

fn validate_score(score: &Score) -> Result<()> {
    validate_tempo(score.bpm)?;
    ensure!(score.total_ticks > 0, "乐谱总时长必须大于零");
    validate_duration(score.total_ticks, score.bpm)?;
    ensure!(
        score.events.len() <= MAX_NOTE_EVENTS,
        "乐谱最多包含 4096 个发声音符"
    );
    for event in &score.events {
        ensure!(event.midi <= 127, "音符必须落在 MIDI 0..127 范围内");
        ensure!(event.duration_ticks > 0, "音符时长必须大于零");
        let end_tick = event
            .start_tick
            .checked_add(event.duration_ticks)
            .context("音符结束 tick 溢出")?;
        ensure!(end_tick <= score.total_ticks, "音符不能超出乐谱总时长");
    }
    Ok(())
}

/// 十进制文本直接转为有理数，不经 f64，以便拒绝不能精确表示的 tick。
fn parse_beats(text: &str) -> Result<u32> {
    let (whole_text, fraction_text) = match text.split_once('.') {
        Some((whole, fraction)) => {
            ensure!(!fraction.is_empty(), "小数点后必须有数字");
            (whole, fraction)
        }
        None => (text, ""),
    };
    ensure!(
        !whole_text.is_empty()
            && whole_text.bytes().all(|byte| byte.is_ascii_digit())
            && fraction_text.bytes().all(|byte| byte.is_ascii_digit()),
        "拍数必须是十进制数字，例如 1、0.5 或 0.0625"
    );
    let significant_whole = whole_text.trim_start_matches('0');
    let whole = if significant_whole.is_empty() {
        0
    } else {
        significant_whole.parse::<u32>().context("拍数超出范围")?
    };
    ensure!(whole <= 64, "拍数必须在 1/16..=64 之间");
    let fraction = fraction_text.trim_end_matches('0');
    // 480 = 2^5 × 3 × 5：去除尾零后，超过 5 位小数必定无法得到整数 tick。
    ensure!(
        fraction.len() <= 5,
        "拍数必须能精确换算为整数 tick（每拍 480 ticks）"
    );
    let scale = 10_u32.pow(fraction.len() as u32);
    let fraction_value = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u32>().context("小数拍数无效")?
    };
    let numerator = (whole * scale + fraction_value) * TICKS_PER_BEAT;
    ensure!(
        numerator.is_multiple_of(scale),
        "拍数必须能精确换算为整数 tick（每拍 480 ticks）"
    );
    let ticks = numerator / scale;
    ensure!(
        (TICKS_PER_BEAT / 16..=64 * TICKS_PER_BEAT).contains(&ticks),
        "拍数必须在 1/16..=64 之间"
    );
    Ok(ticks)
}

fn append_variable_length(mut value: u32, bytes: &mut Vec<u8>) {
    // 验证后的 120 秒时间线最多 384000 ticks，始终在 SMF 四字节 VLQ 范围内。
    let mut buffer = [0_u8; 4];
    let mut index = 3;
    buffer[index] = (value & 0x7f) as u8;
    value >>= 7;
    while value != 0 {
        index -= 1;
        buffer[index] = ((value & 0x7f) as u8) | 0x80;
        value >>= 7;
    }
    bytes.extend_from_slice(&buffer[index..]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_names_cover_accidentals_and_midi_boundaries() {
        for (name, expected) in [
            ("A4", 69),
            ("Bb3", 58),
            ("f#4", 66),
            ("C-1", 0),
            ("G9", 127),
            ("B#3", 60),
            ("Cb4", 59),
            ("c4", 60),
        ] {
            assert_eq!(parse_note(name).unwrap(), expected, "{name}");
        }
        assert!((frequency(69) - 440.0).abs() < 1e-10);
        assert!((frequency(81) - 880.0).abs() < 1e-10);
        assert!((frequency(60) - 261.625_565_300_598_6).abs() < 1e-10);
    }

    #[test]
    fn invalid_or_out_of_range_note_names_are_rejected() {
        for name in [
            "",
            "A",
            "H4",
            "C##4",
            "Cbb4",
            "C♯4",
            "A4x",
            "C-2",
            "G#9",
            "Cb-1",
            "A+4",
            " A4",
            "A4 ",
            "C9999999999999999999",
        ] {
            assert!(parse_note(name).is_err(), "{name}");
        }
    }

    #[test]
    fn chord_rest_and_repeat_preserve_exact_timeline() {
        let score = parse_score("C4 D4:0.5 | C4+E4+G4:2 R:1", 120, 2).unwrap();
        assert_eq!(score.total_ticks, 4320);
        assert_eq!(score.events.len(), 10);
        assert_eq!(
            score.events[1],
            NoteEvent {
                start_tick: 480,
                duration_ticks: 240,
                midi: 62
            }
        );
        assert_eq!(
            score.events[2],
            NoteEvent {
                start_tick: 720,
                duration_ticks: 960,
                midi: 60
            }
        );
        assert_eq!(
            score.events[4],
            NoteEvent {
                start_tick: 720,
                duration_ticks: 960,
                midi: 67
            }
        );
        assert_eq!(score.events[5].start_tick, 2160);
        assert_eq!(score.duration_seconds(), 4.5);
    }

    #[test]
    fn decimal_beats_require_exact_ticks_without_float_rounding() {
        let score = parse_score("C4:0.0625 D4:0.1 E4:1.000000000000000000000", 120, 1).unwrap();
        assert_eq!(
            score
                .events
                .iter()
                .map(|event| event.duration_ticks)
                .collect::<Vec<_>>(),
            [30, 48, 480]
        );
        for value in [
            "0",
            "-1",
            "+1",
            "0.0624",
            "64.0625",
            "0.333",
            "0.333333333333333333333333333333",
            "1/4",
            "1e0",
            "NaN",
            "inf",
            "1.2.3",
            "",
            ".5",
            "1.",
        ] {
            assert!(
                parse_score(&format!("C4:{value}"), 120, 1).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn pure_rests_are_valid_but_empty_or_malformed_scores_are_not() {
        let score = parse_score("R:2 | r", 120, 1).unwrap();
        assert!(score.events.is_empty());
        assert_eq!(score.total_ticks, 1440);
        for text in [
            "",
            "  |  ",
            "C4|D4",
            "C4+",
            "+C4",
            "C4::1",
            "R+C4",
            "C4+C4",
            "C4+B#3",
            "C4+D4+E4+F4+G4+A4+B4+C5+D5",
        ] {
            assert!(parse_score(text, 120, 1).is_err(), "{text}");
        }
    }

    #[test]
    fn tempo_repeat_duration_and_input_limits_are_enforced() {
        assert!(parse_score("R:40", 20, 1).is_ok());
        assert!(parse_score("R:40.0625", 20, 1).is_err());
        assert!(parse_score("R", 32, 64).is_ok());
        assert!(parse_score("R", 20, 41).is_err());
        assert!(parse_score("R", 400, 1).is_ok());
        for bpm in [0, 19, 401, u16::MAX] {
            assert!(parse_score("C4", bpm, 1).is_err());
        }
        for repeat in [0, 65, u16::MAX] {
            assert!(parse_score("C4", 120, repeat).is_err());
        }
        let max_text = format!("{}C4", " ".repeat(65_534));
        assert!(parse_score(&max_text, 120, 1).is_ok());
        assert!(parse_score(&(max_text + " "), 120, 1).is_err());
    }

    #[test]
    fn note_event_limit_counts_chord_notes_and_repetitions() {
        let max_events = "C4:0.0625 ".repeat(4096);
        assert_eq!(parse_score(&max_events, 400, 1).unwrap().events.len(), 4096);
        assert!(parse_score(&(max_events + "C4:0.0625"), 400, 1).is_err());
        let eight_notes = "C4+D4+E4+F4+G4+A4+B4+C5:0.0625 ".repeat(8);
        assert_eq!(
            parse_score(&eight_notes, 400, 64).unwrap().events.len(),
            4096
        );
        assert!(parse_score(&(eight_notes + "D5:0.0625"), 400, 64).is_err());
    }

    /// 测试端按 SMF 的事件边界读出字节，断言实际可播放时间线而不是编码器内部实现。
    fn midi_events(data: &[u8]) -> Vec<(u32, Vec<u8>)> {
        assert_eq!(&data[..14], b"MThd\0\0\0\x06\0\0\0\x01\x01\xe0");
        assert_eq!(&data[14..18], b"MTrk");
        let len = u32::from_be_bytes(data[18..22].try_into().unwrap()) as usize;
        assert_eq!(data.len(), len + 22);
        let mut cursor = 22;
        let mut tick = 0;
        let mut events = Vec::new();
        while cursor < data.len() {
            let mut delta = 0;
            loop {
                let byte = data[cursor];
                cursor += 1;
                delta = (delta << 7) | u32::from(byte & 0x7f);
                if byte & 0x80 == 0 {
                    break;
                }
            }
            tick += delta;
            let size = match data[cursor] {
                0xff => 3 + usize::from(data[cursor + 2]),
                0xc0 => 2,
                0x80 | 0x90 => 3,
                status => panic!("未知 MIDI status: {status:02x}"),
            };
            events.push((tick, data[cursor..cursor + size].to_vec()));
            cursor += size;
        }
        events
    }

    #[test]
    fn midi_includes_tempo_meter_program_and_orders_note_off_before_on() {
        let bytes = encode_midi(&parse_score("C4 C4 R:2", 120, 1).unwrap()).unwrap();
        let events = midi_events(&bytes);
        assert_eq!(
            events,
            vec![
                (0, vec![0xff, 0x51, 3, 0x07, 0xa1, 0x20]),
                (0, vec![0xff, 0x58, 4, 4, 2, 24, 8]),
                (0, vec![0xc0, 0]),
                (0, vec![0x90, 60, 100]),
                (480, vec![0x80, 60, 0]),
                (480, vec![0x90, 60, 100]),
                (960, vec![0x80, 60, 0]),
                (1920, vec![0xff, 0x2f, 0]),
            ]
        );
    }

    #[test]
    fn midi_keeps_chords_and_all_rest_duration() {
        let events =
            midi_events(&encode_midi(&parse_score("C4+E4+G4:0.5", 100, 1).unwrap()).unwrap());
        assert_eq!(&events[0], &(0, vec![0xff, 0x51, 3, 0x09, 0x27, 0xc0]));
        assert_eq!(
            events.iter().filter(|(_, event)| event[0] == 0x90).count(),
            3
        );
        assert_eq!(
            events
                .iter()
                .filter(|(tick, event)| *tick == 240 && event[0] == 0x80)
                .count(),
            3
        );
        let rest_events = midi_events(&encode_midi(&parse_score("R:4", 120, 1).unwrap()).unwrap());
        assert_eq!(rest_events.last().unwrap(), &(1920, vec![0xff, 0x2f, 0]));
        assert_eq!(rest_events.len(), 4);
    }

    #[test]
    fn midi_revalidates_public_score_before_encoding() {
        let valid = parse_score("C4", 120, 1).unwrap();
        let mut invalid = valid.clone();
        invalid.bpm = 0;
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.events[0].midi = 128;
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.events[0].duration_ticks = 0;
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.events[0].start_tick = u32::MAX;
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.total_ticks = 1;
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.events = vec![valid.events[0].clone(); 4097];
        assert!(encode_midi(&invalid).is_err());
        invalid = valid.clone();
        invalid.total_ticks = 115_201;
        assert!(encode_midi(&invalid).is_err());
        assert!(
            encode_midi(&Score {
                events: vec![],
                total_ticks: 0,
                bpm: 120
            })
            .is_err()
        );
    }
}
