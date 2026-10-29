//! 新伙伴共用的原始字节输入与安全输出。
//!
//! 文本参数明确按 UTF-8 解释，文件与 stdin 不做隐式转换；只有要求文本的
//! 调用方才使用 read_utf8。这样 Hash 和 Base64 能完整保留二进制内容。
use anyhow::{Context, Result, bail, ensure};
use clap::Args;
use std::{
    fs::File,
    io::{self, Cursor, IsTerminal, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Args, Clone, Debug, Default)]
pub struct InputArgs {
    /// 输入文本（UTF-8）；省略时从非终端 stdin 读取，与 --file 互斥。
    #[arg(value_name = "TEXT", conflicts_with = "file")]
    pub text: Option<String>,
    /// 读取文件原始内容；- 表示 stdin。
    #[arg(long, value_name = "PATH", conflicts_with = "text")]
    pub file: Option<PathBuf>,
}

impl InputArgs {
    /// 返回可流式读取的输入和报告所用的来源类型，不在报告中复述输入内容。
    pub fn open(&self) -> Result<(Box<dyn Read>, &'static str)> {
        ensure!(
            self.text.is_none() || self.file.is_none(),
            "文本与 --file 不能同时指定"
        );
        if let Some(text) = &self.text {
            return Ok((Box::new(Cursor::new(text.as_bytes().to_vec())), "text"));
        }
        if let Some(path) = &self.file {
            if path != Path::new("-") {
                let file = File::open(path)
                    .with_context(|| format!("无法读取文件：{}", path.display()))?;
                return Ok((Box::new(file), "file"));
            }
        } else if io::stdin().is_terminal() {
            bail!("请传入文本、--file PATH，或通过管道提供 stdin");
        }
        Ok((Box::new(io::stdin()), "stdin"))
    }

    /// 多读一个字节用于发现动态增长的输入；检查不只依赖最初文件长度。
    pub fn read_bytes(&self, max_bytes: usize) -> Result<Vec<u8>> {
        let (reader, _) = self.open()?;
        let limit = u64::try_from(max_bytes)?
            .checked_add(1)
            .context("输入上限过大")?;
        let mut bytes = Vec::new();
        reader
            .take(limit)
            .read_to_end(&mut bytes)
            .context("读取输入失败")?;
        ensure!(
            bytes.len() <= max_bytes,
            "输入超过允许的字节数上限 {max_bytes}"
        );
        Ok(bytes)
    }

    pub fn read_utf8(&self, max_bytes: usize) -> Result<String> {
        String::from_utf8(self.read_bytes(max_bytes)?).context("输入不是有效 UTF-8 文本")
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct OutputArgs {
    /// 保存到文件；省略时写 stdout，具体结果格式由当前命令决定。
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<PathBuf>,
    /// 明确允许覆盖已有结果文件；仍禁止覆盖输入文件。
    #[arg(long, requires = "output")]
    pub overwrite: bool,
}

impl OutputArgs {
    pub fn write(&self, input: &InputArgs, bytes: &[u8], terminal_guard: bool) -> Result<()> {
        if let Some(path) = &self.output {
            if let Some(source) = input.file.as_deref().filter(|p| *p != Path::new("-")) {
                // 在 publish 之前拒绝同路径、硬链接与符号链接，避免覆盖读取来源。
                if crate::files::same(source, path) {
                    bail!("输出与输入指向同一文件，不能覆盖输入");
                }
                crate::files::validate_destination(source, path, false, self.overwrite)?;
            }
            return crate::files::publish(bytes, path, self.overwrite);
        }
        if terminal_guard && io::stdout().is_terminal() {
            let text = std::str::from_utf8(bytes)
                .context("结果包含二进制字节，请使用 -o PATH 或重定向保存")?;
            ensure!(
                text.chars()
                    .all(|c| !c.is_control() || matches!(c, '\t' | '\n' | '\r')),
                "结果包含终端控制字符，请使用 -o PATH 或重定向保存"
            );
        }
        write_stdout(bytes)
    }
}

/// 管道下游正常提前退出时，不将 BrokenPipe 升级为 panic 或额外错误输出。
pub fn write_stdout(bytes: &[u8]) -> Result<()> {
    match io::stdout().lock().write_all(bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(error) => Err(error).context("写入 stdout 失败"),
    }
}
