//! 先规划所有路径，再写文件，避免批量处理中覆盖尚未读取的源文件。
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize)]
pub struct Job {
    pub input: PathBuf,
    pub output: PathBuf,
}
pub fn read_bounded(path: &Path, budget: usize) -> Result<Vec<u8>> {
    let file = fs::File::open(path).with_context(|| format!("无法读取 {}", path.display()))?;
    let limit = budget / 8;
    ensure!(
        file.metadata()?.len() <= limit as u64,
        "输入文件超过内存预算的 1/8"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "读取过程中输入超过预算");
    Ok(bytes)
}
pub fn normalized(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let abs = std::path::absolute(path)?;
    let parent = abs.parent().context("路径没有父目录")?;
    Ok(normalized(parent)?.join(abs.file_name().context("路径没有文件名")?))
}
pub fn same(a: &Path, b: &Path) -> bool {
    same_file::is_same_file(a, b).unwrap_or(false) || normalized(a).ok() == normalized(b).ok()
}

pub fn plan(
    input: &Path,
    output: Option<&Path>,
    output_dir: Option<&Path>,
    recursive: bool,
    in_place: bool,
    overwrite: bool,
) -> Result<Vec<Job>> {
    ensure!(
        !fs::symlink_metadata(input)?.file_type().is_symlink(),
        "输入不能是符号链接，请使用真实路径"
    );
    let mut jobs = Vec::new();
    if input.is_dir() {
        ensure!(output.is_none(), "目录输入使用 --output-dir");
        ensure!(
            in_place || output_dir.is_some(),
            "目录输入需要 --output-dir 或 --in-place"
        );
        let root = fs::canonicalize(input)?;
        let dest = output_dir.map(normalized).transpose()?;
        ensure!(
            dest.as_ref() != Some(&root),
            "输出目录不能与输入目录相同，请使用 --in-place"
        );
        let entries = walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_depth(if recursive { usize::MAX } else { 1 })
            .into_iter()
            .filter_entry(|e| {
                dest.as_ref()
                    .is_none_or(|d| !d.starts_with(&root) || !e.path().starts_with(d))
            });
        for entry in entries {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            if !entry
                .path()
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("png") || s.eq_ignore_ascii_case("apng"))
            {
                continue;
            }
            let source = entry.path().to_owned();
            let target = if in_place {
                source.clone()
            } else {
                dest.as_ref().unwrap().join(source.strip_prefix(&root)?)
            };
            jobs.push(Job {
                input: source,
                output: target,
            });
        }
    } else {
        let source = fs::canonicalize(input)?;
        let name = input.file_stem().context("无文件名")?;
        let mut generated = name.to_os_string();
        generated.push(".optimized.png");
        let target = if in_place {
            source.clone()
        } else if let Some(out) = output {
            out.to_path_buf()
        } else if let Some(dir) = output_dir {
            dir.join(generated)
        } else {
            input.with_file_name(generated)
        };
        jobs.push(Job {
            input: source,
            output: target,
        });
    }
    jobs.sort_by(|a, b| a.input.cmp(&b.input));
    ensure!(!jobs.is_empty(), "没有找到 PNG 图片");
    for (i, job) in jobs.iter().enumerate() {
        validate_destination(&job.input, &job.output, in_place, overwrite)?;
        for (j, other) in jobs.iter().enumerate() {
            if i != j {
                ensure!(
                    !same(&job.output, &other.input),
                    "输出会覆盖其他输入：{}",
                    job.output.display()
                );
            }
            if i < j {
                ensure!(
                    !same(&job.output, &other.output),
                    "输出路径冲突：{}",
                    job.output.display()
                );
            }
        }
    }
    Ok(jobs)
}
pub fn validate_destination(
    input: &Path,
    output: &Path,
    in_place: bool,
    overwrite: bool,
) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(output) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "输出必须是普通文件，不能是符号链接"
        );
    }
    ensure!(
        in_place || !same(input, output),
        "输出与输入指向相同文件；替换原图请使用 --in-place"
    );
    ensure!(
        in_place || overwrite || !output.exists(),
        "输出已存在：{}；可显式指定 --overwrite",
        output.display()
    );
    Ok(())
}
pub fn publish(bytes: &[u8], path: &Path, overwrite: bool) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "拒绝替换非普通文件"
        );
    }
    // 临时文件与目标同目录，保证最终 rename 不跨文件系统。noclobber 仍在
    // 发布瞬间检查存在性，而非依赖先前 exists()，避免意外覆盖并发写入者。
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(bytes)?;
    publish_staged(temp, path, overwrite)
}

/// 发布已经写入并验证的同目录候选文件；视频等大文件无需整体读入内存。
/// 调用者负责确保候选与目标位于同一文件系统，并在调用前完成内容校验。
pub fn publish_staged(temp: tempfile::NamedTempFile, path: &Path, overwrite: bool) -> Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        ensure!(
            meta.is_file() && !meta.file_type().is_symlink(),
            "拒绝替换非普通文件"
        );
    }
    temp.as_file().sync_all()?;
    if overwrite {
        if let Ok(meta) = fs::metadata(path) {
            temp.as_file().set_permissions(meta.permissions())?;
        }
        temp.persist(path).map_err(|e| e.error)?;
    } else {
        temp.persist_noclobber(path).map_err(|e| e.error)?;
    }
    Ok(())
}
