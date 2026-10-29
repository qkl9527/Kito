//! Vido 的帮助案例单独维护，避免帮助布局调整介入视频功能的实现。

use clap::Command;

/// 只补充尚未设置的案例；视频模块自行维护的 after_help 始终优先。
/// 按命令名称进入 audio 子树，避免 convert/trim 等同名能力共用错误案例。
pub(super) fn with_examples(command: Command) -> Command {
    with_fallback(command, VIDEO).mut_subcommands(|command| {
        let examples = match command.get_name() {
            "doctor" => DOCTOR,
            "info" => INFO,
            "convert" => CONVERT,
            "compress" => COMPRESS,
            "trim" => TRIM,
            "resize" => RESIZE,
            "crop" => CROP,
            "rotate" => ROTATE,
            "speed" => SPEED,
            "concat" => CONCAT,
            "mute" => MUTE,
            "snapshot" => SNAPSHOT,
            "gif" => GIF,
            "audio" => return with_audio_examples(command),
            _ => return command,
        };
        with_fallback(command, examples)
    })
}

fn with_audio_examples(command: Command) -> Command {
    with_fallback(command, AUDIO).mut_subcommands(|command| {
        let examples = match command.get_name() {
            "extract" => AUDIO_EXTRACT,
            "replace" => AUDIO_REPLACE,
            "volume" => AUDIO_VOLUME,
            "convert" => AUDIO_CONVERT,
            "trim" => AUDIO_TRIM,
            "concat" => AUDIO_CONCAT,
            "mix" => AUDIO_MIX,
            "speed" => AUDIO_SPEED,
            "fade" => AUDIO_FADE,
            "normalize" => AUDIO_NORMALIZE,
            _ => return command,
        };
        with_fallback(command, examples)
    })
}

fn with_fallback(command: Command, examples: &'static str) -> Command {
    if command.get_after_help().is_some() {
        command
    } else {
        command.after_help(examples)
    }
}

const VIDEO: &str = r#"使用案例:
  doctor：检查 FFmpeg、ffprobe 后端路径与版本
    kito video doctor --json
  info：查看容器、音视频轨道、尺寸与时长
    kito video info "clip.mp4" --json
  convert：重新编码为 MP4
    kito video convert "clip.mov" --to mp4 -o "converted.mp4"
  compress：使用体积优先档位；结果不保证更小
    kito video compress "clip.mp4" --quality small --preset slow -o "compressed.mp4"
  trim：从第 10 秒开始截取 5 秒
    kito video trim "clip.mp4" --start 10 --duration 5 -o "trimmed.mp4"
  resize：等比缩放至 1280×720 范围内
    kito video resize "clip.mp4" --size 1280x720 -o "resized.mp4"
  crop：从指定坐标裁切矩形画面
    kito video crop "clip.mp4" --width 640 --height 360 --x 100 --y 50 -o "cropped.mp4"
  rotate：顺时针旋转 90 度
    kito video rotate "clip.mp4" --angle 90 -o "rotated.mp4"
  speed：两倍速播放，音画同步并保持音调
    kito video speed "clip.mp4" --factor 2 -o "faster.mp4"
  concat：按输入顺序拼接并统一画布和帧率
    kito video concat "opening.mp4" "main.mp4" --size 1280x720 --fps 30 -o "joined.mp4"
  mute：移除视频音轨
    kito video mute "clip.mp4" -o "muted.mp4"
  audio：提取第一条音轨为 MP3
    kito video audio extract "clip.mp4" --to mp3 -o "soundtrack.mp3"
  snapshot：截取指定时刻的 PNG 图片
    kito video snapshot "clip.mp4" --at 3.5 -o "cover.png"
  gif：将 3 秒片段制作为 GIF
    kito video gif "clip.mp4" --start 2 --duration 3 --width 480 --fps 12 -o "preview.gif""#;

const DOCTOR: &str = r#"使用案例:
  查看 FFmpeg、ffprobe 后端及编码能力
    kito video doctor
  输出 JSON 后端报告
    kito video doctor --json"#;

const INFO: &str = r#"使用案例:
  查看视频的容器、轨道、尺寸与时长
    kito video info "clip.mp4"
  查看音频文件信息并输出 JSON
    kito video info "soundtrack.flac" --json"#;

const CONVERT: &str = r#"使用案例:
  以高清晰度档位重新编码为 MP4
    kito video convert "clip.mov" --to mp4 --quality high -o "share.mp4"
  预览 WebM 转换参数，不编码、不创建目标
    kito video convert "clip.mp4" --to webm --preset slow -o "web.webm" --dry-run --json"#;

const COMPRESS: &str = r#"使用案例:
  使用体积优先质量与慢速编码；结果不保证更小
    kito video compress "clip.mp4" --quality small --preset slow -o "compressed.mp4"
  改用均衡质量，设置每次子进程的时限并输出 JSON
    kito video compress "clip.mp4" --quality balanced --time-limit 7200 -o "balanced.mp4" --json"#;

const TRIM: &str = r#"使用案例:
  从第 12.5 秒开始截取 7.5 秒
    kito video trim "clip.mp4" --start 12.5 --duration 7.5 -o "highlight.mp4"
  用起点和绝对终点截取片段；--end 与 --duration 二选一
    kito video trim "clip.mp4" --start "00:01:10" --end "00:01:20" -o "ten-seconds.mp4" --json"#;

const RESIZE: &str = r#"使用案例:
  等比放入 1280×720 范围，允许放大
    kito video resize "clip.mp4" --size 1280x720 -o "resized.mp4"
  将宽高缩小到 50%，并输出 JSON 报告
    kito video resize "clip.mp4" --size "50%" -o "half-size.mp4" --json"#;

const CROP: &str = r#"使用案例:
  从画面坐标 x=100、y=50 裁切 640×360 区域
    kito video crop "clip.mp4" --width 640 --height 360 --x 100 --y 50 -o "cropped.mp4"
  从左上角裁切 320×240 区域，预览参数
    kito video crop "clip.mp4" --width 320 --height 240 -o "corner.mp4" --dry-run --json"#;

const ROTATE: &str = r#"使用案例:
  顺时针旋转 90 度
    kito video rotate "clip.mp4" --angle 90 -o "clockwise.mp4"
  顺时针旋转 270 度，以高清晰度档位重新编码
    kito video rotate "clip.mp4" --angle 270 --quality high -o "counterclockwise.mp4" --json"#;

const SPEED: &str = r#"使用案例:
  两倍速播放，音频同步变速并保持音调
    kito video speed "clip.mp4" --factor 2 -o "double-speed.mp4"
  半速播放，输出 JSON 报告
    kito video speed "clip.mp4" --factor 0.5 -o "half-speed.mp4" --json"#;

const CONCAT: &str = r#"使用案例:
  按参数顺序拼接三段视频
    kito video concat "opening.mp4" "main.mov" "ending.webm" -o "joined.mp4"
  统一为 1280×720 画布和 30 fps，比例不同时补黑边
    kito video concat "portrait.mp4" "landscape.mp4" --size 1280x720 --fps 30 -o "joined-landscape.mp4" --json"#;

const MUTE: &str = r#"使用案例:
  移除音频轨道，保存为新视频
    kito video mute "clip.mp4" -o "muted.mp4"
  选择视频质量并预览去音轨参数
    kito video mute "clip.mp4" --quality high -o "muted-high.mp4" --dry-run --json"#;

const SNAPSHOT: &str = r#"使用案例:
  截取第 3.5 秒的一帧，保存为 PNG
    kito video snapshot "clip.mp4" --at 3.5 -o "cover.png"
  使用时分秒指定截图时刻并输出 JSON
    kito video snapshot "clip.mp4" --at "00:00:10" -o "frame-at-ten.png" --json"#;

const GIF: &str = r#"使用案例:
  从第 2 秒开始生成 3 秒 GIF，宽度 480、帧率 12
    kito video gif "clip.mp4" --start 2 --duration 3 --width 480 --fps 12 -o "preview.gif"
  为较窄的 GIF 预览编码参数
    kito video gif "clip.mp4" --start "00:00:10" --duration 2 --width 320 --fps 10 -o "small.gif" --dry-run --json"#;

const AUDIO: &str = r#"使用案例:
  extract：提取第一条音轨为 MP3
    kito video audio extract "clip.mp4" --to mp3 -o "soundtrack.mp3"
  replace：用音乐文件替换视频的原音轨
    kito video audio replace "clip.mp4" --audio "music.mp3" -o "scored.mp4"
  volume：把音频线性音量调为原来的 50%
    kito video audio volume "song.wav" --factor 0.5 --to wav -o "quieter.wav"
  convert：转换音频格式
    kito video audio convert "recording.wav" --to flac -o "archive.flac"
  trim：从第 10 秒开始截取 5 秒音频
    kito video audio trim "song.wav" --start 10 --duration 5 --to wav -o "excerpt.wav"
  concat：顺序拼接音频，统一为 48 kHz 立体声
    kito video audio concat "intro.wav" "speech.wav" --to mp3 -o "program.mp3"
  mix：等权混合音频，以最长一轨为准
    kito video audio mix "voice.wav" "music.wav" --to wav -o "mixed.wav"
  speed：改变播放速度并保持音调
    kito video audio speed "speech.wav" --factor 1.25 --to wav -o "faster-speech.wav"
  fade：开头淡入 2 秒，结尾淡出 3 秒
    kito video audio fade "song.wav" --fade-in 2 --fade-out 3 --to wav -o "faded.wav"
  normalize：按目标 LUFS 做单遍响度处理
    kito video audio normalize "podcast.wav" --loudness -16 --to wav -o "normalized.wav""#;

const AUDIO_EXTRACT: &str = r#"使用案例:
  将视频的第一条音轨重新编码为 MP3
    kito video audio extract "clip.mp4" --to mp3 -o "soundtrack.mp3"
  提取为 WAV，输出 JSON 报告
    kito video audio extract "clip.mp4" --to wav -o "soundtrack.wav" --json"#;

const AUDIO_REPLACE: &str = r#"使用案例:
  替换原音轨；新音轨过短补静音、过长截断
    kito video audio replace "clip.mp4" --audio "music.mp3" -o "scored.mp4"
  以高清晰度档位预览替换配乐参数
    kito video audio replace "clip.mp4" --audio "music.wav" --quality high -o "scored-high.mp4" --dry-run --json"#;

const AUDIO_VOLUME: &str = r#"使用案例:
  保留视频画面，将原音轨的线性音量减半
    kito video audio volume "clip.mp4" --factor 0.5 -o "quieter.mp4"
  明确选择音频输出格式，仅保存调整音量后的音轨
    kito video audio volume "clip.mp4" --factor 0.8 --to wav -o "quieter.wav" --json"#;

const AUDIO_CONVERT: &str = r#"使用案例:
  将 WAV 转为 FLAC
    kito video audio convert "recording.wav" --to flac -o "archive.flac"
  将 FLAC 转为有损 M4A/AAC，输出 JSON 报告
    kito video audio convert "song.flac" --to m4a -o "portable.m4a" --json"#;

const AUDIO_TRIM: &str = r#"使用案例:
  从第 1 分钟开始截取 30 秒音频
    kito video audio trim "recording.wav" --start "00:01:00" --duration 30 --to wav -o "excerpt.wav"
  指定起点与绝对终点；--end 与 --duration 二选一
    kito video audio trim "song.flac" --start 5 --end 25 --to flac -o "twenty-seconds.flac" --json"#;

const AUDIO_CONCAT: &str = r#"使用案例:
  按输入顺序拼接三段音频
    kito video audio concat "intro.wav" "speech.wav" "outro.wav" --to mp3 -o "program.mp3"
  预览统一为 48 kHz 立体声的音频拼接参数
    kito video audio concat "part-one.flac" "part-two.wav" --to flac -o "joined.flac" --dry-run --json"#;

const AUDIO_MIX: &str = r#"使用案例:
  从同一时刻开始等权混合两条音轨
    kito video audio mix "voice.wav" "music.wav" --to wav -o "mixed.wav"
  混合三条音轨，以最长轨道为准并输出 JSON
    kito video audio mix "voice.wav" "music.wav" "ambience.wav" --to flac -o "mixed.flac" --json"#;

const AUDIO_SPEED: &str = r#"使用案例:
  以 1.25 倍速播放人声，保持音调
    kito video audio speed "speech.wav" --factor 1.25 --to wav -o "faster-speech.wav"
  将音乐改为半速播放并输出 JSON
    kito video audio speed "song.flac" --factor 0.5 --to flac -o "slower-song.flac" --json"#;

const AUDIO_FADE: &str = r#"使用案例:
  在开头淡入 2 秒，在结束前淡出 3 秒
    kito video audio fade "song.wav" --fade-in 2 --fade-out 3 --to wav -o "faded.wav"
  只设置结尾 1.5 秒淡出，并预览处理参数
    kito video audio fade "song.flac" --fade-out "00:00:01.5" --to flac -o "fade-out.flac" --dry-run --json"#;

const AUDIO_NORMALIZE: &str = r#"使用案例:
  以 -16 LUFS 为目标做单遍响度处理
    kito video audio normalize "podcast.wav" --loudness -16 --to wav -o "normalized.wav"
  改用 -18 LUFS 目标，输出 JSON；单遍处理不保证精确达标
    kito video audio normalize "speech.wav" --loudness -18 --to flac -o "normalized.flac" --json"#;
