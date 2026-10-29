//! 排他目录与无覆盖发布。目录和每份候选文件均记录身份，错误清理只删除自己创建的对象。
use anyhow::{Context, Result, bail};
use same_file::Handle;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

pub struct Directory {
    pub path: PathBuf,
    identity: Handle,
    files: Vec<(PathBuf, Handle)>,
    keep: bool,
}
impl Directory {
    pub fn create(path: &Path) -> Result<Self> {
        // 只让最终密钥目录拥有本次操作的清理权限；共享父目录不含密钥，失败时保留。
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).context("无法创建密钥目录的父路径")?;
        }
        create_private_directory(path)
            .with_context(|| format!("无法排他创建密钥目录 {}；目录必须不存在", path.display()))?;
        Ok(Self {
            path: path.to_owned(),
            identity: Handle::from_path(path)?,
            files: Vec::new(),
            keep: false,
        })
    }
    pub fn candidate(&mut self, name: &str) -> Result<PathBuf> {
        let path = self.path.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(&path)
            .context("无法创建受保护的密钥候选文件")?;
        self.files.push((path.clone(), Handle::from_file(file)?));
        #[cfg(windows)]
        windows::verify(&path, false)?;
        Ok(path)
    }
    pub fn publish(&mut self, candidate: &Path, name: &str) -> Result<PathBuf> {
        let target = self.path.join(name);
        publish(candidate, &target)?;
        self.files
            .push((target.clone(), Handle::from_path(&target)?));
        Ok(target)
    }
    pub fn finish(mut self) -> Result<()> {
        for (path, _) in &self.files {
            if path
                .file_name()
                .is_some_and(|n| n != "private.pem" && n != "public.pem")
            {
                fs::remove_file(path)?;
            }
        }
        self.keep = true;
        Ok(())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.keep || !Handle::from_path(&self.path).is_ok_and(|h| h == self.identity) {
            return;
        }
        for (path, identity) in &self.files {
            if Handle::from_path(path).is_ok_and(|h| h == *identity) {
                let _ = fs::remove_file(path);
            }
        }
        let _ = fs::remove_dir(&self.path);
    }
}
pub fn publish(candidate: &Path, target: &Path) -> Result<()> {
    // hard_link 是同文件系统内的排他发布：即使检查后目标被占用，也不会覆盖。
    // Windows FlushFileBuffers 需要写权限，不能用只读 File::open 的句柄。
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(candidate)?
        .sync_all()?;
    fs::hard_link(candidate, target)
        .with_context(|| format!("无法发布 {}；输出必须不存在", target.display()))
}
pub fn reject_existing(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(_) => bail!("输出已存在，拒绝覆盖：{}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("无法检查输出位置"),
    }
}
#[cfg(unix)]
pub(crate) fn create_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path)
}
#[cfg(windows)]
pub(crate) fn create_private_directory(path: &Path) -> std::io::Result<()> {
    windows::create(path)
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn create_private_directory(_: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other("当前平台尚未实现密钥访问控制"))
}
#[cfg(windows)]
#[path = "windows.rs"]
mod windows;
