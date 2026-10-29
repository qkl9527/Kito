//! 仅本机可访问的会话服务：上传 ID 和下载 ID 是随机能力标识，绝不映射用户路径。
use super::{
    assets,
    commands::{self, RunRequest},
    process,
};
use anyhow::{Context, Result, ensure};
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path as ApiPath, Query, Request, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::sync::Semaphore;
use tokio_util::io::ReaderStream;

pub const UPLOAD_LIMIT: usize = 64 * 1024 * 1024;
const SESSION_UPLOAD_LIMIT: u64 = 256 * 1024 * 1024;
const SESSION_OUTPUT_LIMIT: u64 = 512 * 1024 * 1024;
const JOB_LIMIT: usize = 64;
const UPLOAD_COUNT_LIMIT: usize = 64;
const JSON_LIMIT: usize = 128 * 1024;

type ApiResult<T> = std::result::Result<T, ApiError>;
pub struct ApiError(StatusCode, String);
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error":self.1}))).into_response()
    }
}
impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self(StatusCode::BAD_REQUEST, format!("{error:#}"))
    }
}
impl From<std::io::Error> for ApiError {
    fn from(error: std::io::Error) -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("会话文件操作失败：{error}"),
        )
    }
}
fn error(status: StatusCode, message: &str) -> ApiError {
    ApiError(status, message.into())
}

#[derive(Clone, Serialize)]
struct Upload {
    id: String,
    name: String,
    size: u64,
    #[serde(skip)]
    path: PathBuf,
}
#[derive(Clone, Serialize)]
struct Download {
    id: String,
    name: String,
    size: u64,
    mime: String,
    #[serde(skip)]
    path: PathBuf,
}
#[derive(Clone, Serialize)]
struct Job {
    id: String,
    state: &'static str,
    stdout: String,
    report: Option<Value>,
    files: Vec<Download>,
    error: Option<String>,
}
#[derive(Default)]
struct Store {
    uploads: HashMap<String, Upload>,
    files: HashMap<String, Download>,
    jobs: HashMap<String, Job>,
    upload_bytes: u64,
    output_bytes: u64,
    active: usize,
}

pub struct Session {
    _temporary: tempfile::TempDir,
    root: PathBuf,
    executable: PathBuf,
    origin: String,
    host: String,
    token: String,
    store: Mutex<Store>,
    stopping: AtomicBool,
    requests: Arc<Semaphore>,
    capabilities: Value,
}

fn random_id() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).context("无法获取安全随机数，Web 服务未启动")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

/// Host 防止 DNS rebinding，Origin 防止第三方网页冒用本机服务；token 不由网络静态页提供。
fn authorize(headers: &HeaderMap, session: &Session, api: bool) -> ApiResult<()> {
    if headers.get_all(header::HOST).iter().count() != 1
        || headers.get(header::HOST).and_then(|v| v.to_str().ok()) != Some(&session.host)
    {
        return Err(error(StatusCode::FORBIDDEN, "仅接受本机启动地址的请求"));
    }
    if headers.get_all(header::ORIGIN).iter().count() > 1
        || headers
            .get(header::ORIGIN)
            .is_some_and(|origin| origin.to_str().ok() != Some(&session.origin))
    {
        return Err(error(StatusCode::FORBIDDEN, "不接受其他网页发起的请求"));
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        == Some("cross-site")
    {
        return Err(error(StatusCode::FORBIDDEN, "不接受跨站请求"));
    }
    if api
        && !headers
            .get("x-kito-token")
            .is_some_and(|token| constant_time_equal(token.as_bytes(), session.token.as_bytes()))
    {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "会话凭证无效，请使用终端显示的完整地址打开页面",
        ));
    }
    Ok(())
}

async fn guard(State(session): State<Arc<Session>>, request: Request, next: Next) -> Response {
    let check = authorize(
        request.headers(),
        &session,
        request.uri().path().starts_with("/api/"),
    );
    let mut response = if let Err(error) = check {
        error.into_response()
    } else {
        match session.requests.clone().try_acquire_owned() {
            Err(_) => error(StatusCode::TOO_MANY_REQUESTS, "请求较多，请稍后重试").into_response(),
            Ok(_permit) => {
                match tokio::time::timeout(Duration::from_secs(35), next.run(request)).await {
                    Ok(response) => response,
                    Err(_) => error(
                        StatusCode::REQUEST_TIMEOUT,
                        "请求超时，请重新选择文件或重试",
                    )
                    .into_response(),
                }
            }
        }
    };
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("content-security-policy", HeaderValue::from_static("default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' blob: data:; media-src 'self' blob:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'; object-src 'none'"));
    response
}

async fn info(State(session): State<Arc<Session>>) -> Json<Value> {
    Json(
        json!({"version":env!("CARGO_PKG_VERSION"), "limits":{"upload_bytes":UPLOAD_LIMIT,"session_upload_bytes":SESSION_UPLOAD_LIMIT,"job_seconds":process::TIME_LIMIT,"concurrent_jobs":2,"job_count":JOB_LIMIT,"output_bytes":process::OUTPUT_LIMIT},"backends":session.capabilities}),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UploadQuery {
    name: String,
}

fn safe_name(name: &str) -> Result<&str> {
    ensure!(
        !name.is_empty() && name.len() <= 240,
        "文件名不能为空或超过 240 字节"
    );
    ensure!(
        !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':')),
        "请选择普通文件，文件名不能包含路径或控制字符"
    );
    ensure!(name != "." && name != "..", "文件名无效");
    Ok(name)
}

async fn upload(
    State(session): State<Arc<Session>>,
    Query(query): Query<UploadQuery>,
    request: Request,
) -> ApiResult<Json<Upload>> {
    safe_name(&query.name)?;
    let bytes = to_bytes(request.into_body(), UPLOAD_LIMIT)
        .await
        .map_err(|_| error(StatusCode::PAYLOAD_TOO_LARGE, "单文件不能超过 64 MiB"))?;
    let session_copy = session.clone();
    let uploaded = tokio::task::spawn_blocking(move || -> ApiResult<Upload> {
        let mut store = session_copy
            .store
            .lock()
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "会话状态不可用"))?;
        if store.uploads.len() >= UPLOAD_COUNT_LIMIT
            || store.upload_bytes.saturating_add(bytes.len() as u64) > SESSION_UPLOAD_LIMIT
        {
            return Err(error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "本次会话上传已达上限，请清空本次文件后重试",
            ));
        }
        let id = random_id()?;
        // 原始文件名只用于界面，不参与路径拼接；仅可信扩展名用于后端格式识别。
        let extension = Path::new(&query.name)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| {
                !e.is_empty() && e.len() <= 12 && e.bytes().all(|b| b.is_ascii_alphanumeric())
            });
        let disk_name = match extension {
            Some(ext) => format!("{id}.{}", ext.to_ascii_lowercase()),
            None => id.clone(),
        };
        let path = session_copy.root.join("uploads").join(disk_name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path)?.write_all(&bytes)?;
        let upload = Upload {
            id: id.clone(),
            name: query.name,
            size: bytes.len() as u64,
            path,
        };
        store.upload_bytes += upload.size;
        store.uploads.insert(id, upload.clone());
        Ok(upload)
    })
    .await
    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "无法保存本次上传"))??;
    Ok(Json(uploaded))
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mid" | "midi" => "audio/midi",
        "json" => "application/json",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn collect_files(directory: &Path) -> Result<Vec<Download>> {
    process::output_size(directory)?;
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(directory).follow_links(false) {
        let entry = entry?;
        ensure!(!entry.file_type().is_symlink(), "输出包含链接，不能下载");
        if entry.file_type().is_file() {
            paths.push(entry.into_path());
        }
    }
    paths.sort();
    ensure!(paths.len() <= 32, "任务生成的文件数量超过限制");
    paths
        .into_iter()
        .map(|path| {
            Ok(Download {
                id: random_id()?,
                name: path
                    .file_name()
                    .context("输出缺少文件名")?
                    .to_string_lossy()
                    .into_owned(),
                size: fs::metadata(&path)?.len(),
                mime: mime(&path).into(),
                path,
            })
        })
        .collect()
}

fn execute(
    session: Arc<Session>,
    tool: String,
    id: String,
    directory: PathBuf,
    plan: commands::PreparedCommand,
) {
    let outcome = (|| -> Result<Job> {
        let output_dir = directory.join("output");
        fs::create_dir_all(&output_dir)?;
        let output = process::run(&session.executable, plan, &directory, &session.stopping)?;
        // 密钥报告不含私钥/口令，仍不把 stderr 的未知内容自动回显给用户。
        let diagnostic = if tool == "key" {
            "密钥处理失败，请检查算法、口令、文件与 OpenSSL 3 后端。".to_string()
        } else {
            output.stderr.trim().to_string()
        };
        ensure!(
            output.success,
            "{}",
            if diagnostic.is_empty() {
                "处理失败，请检查输入与参数"
            } else {
                &diagnostic
            }
        );
        let stdout = String::from_utf8(output.stdout)
            .context("任务输出不是 UTF-8 文本，请使用结果文件下载")?;
        let report = serde_json::from_str(&stdout).ok();
        let files = collect_files(&output_dir)?;
        Ok(Job {
            id: id.clone(),
            state: "completed",
            stdout,
            report,
            files,
            error: None,
        })
    })();
    let mut job = match outcome {
        Ok(job) => job,
        Err(error) => {
            // 错误不能产生任何下载入口，包括领域命令曾写出的部分文件。
            let _ = fs::remove_dir_all(directory.join("output"));
            Job {
                id: id.clone(),
                state: "failed",
                stdout: String::new(),
                report: None,
                files: vec![],
                error: Some(format!("{error:#}")),
            }
        }
    };
    // 请求中的口令随该工作线程释放；会话状态只保存经过 CLI 约束的结果。
    if let Ok(mut store) = session.store.lock() {
        let total: u64 = job.files.iter().map(|file| file.size).sum();
        if store.output_bytes.saturating_add(total) > SESSION_OUTPUT_LIMIT {
            let _ = fs::remove_dir_all(directory.join("output"));
            job.state = "failed";
            job.stdout.clear();
            job.report = None;
            job.files.clear();
            job.error = Some("会话结果超过 512 MiB，请下载需要的文件并清空会话".into());
        } else {
            store.output_bytes += total;
            for file in &job.files {
                store.files.insert(file.id.clone(), file.clone());
            }
        }
        store.jobs.insert(id, job);
        store.active = store.active.saturating_sub(1);
    }
    // 文本日志也可能包含用户内容，不在任务结束后多留一份磁盘副本。
    let _ = fs::remove_file(directory.join("stdout.log"));
    let _ = fs::remove_file(directory.join("stderr.log"));
}

async fn start_job(
    State(session): State<Arc<Session>>,
    request: Request,
) -> ApiResult<(StatusCode, Json<Value>)> {
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .is_none_or(|v| !v.starts_with("application/json"))
    {
        return Err(error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "任务请求须为 JSON",
        ));
    }
    let body = to_bytes(request.into_body(), JSON_LIMIT)
        .await
        .map_err(|_| error(StatusCode::PAYLOAD_TOO_LARGE, "任务参数过大"))?;
    let request: RunRequest = serde_json::from_slice(&body)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "任务 JSON 格式无效"))?;
    let id = random_id()?;
    let directory = session.root.join("jobs").join(&id);
    let mut store = session
        .store
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "会话状态不可用"))?;
    if session.stopping.load(Ordering::Relaxed) {
        return Err(error(StatusCode::SERVICE_UNAVAILABLE, "服务正在停止"));
    }
    if store.active >= 2 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "已有两个任务正在处理，请稍后再试",
        ));
    }
    if store.jobs.len() >= JOB_LIMIT {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "本次会话任务已达 64 个，请清空后重试",
        ));
    }
    let mut inputs: Vec<PathBuf> = request
        .files
        .iter()
        .map(|id| {
            store
                .uploads
                .get(id)
                .map(|f| f.path.clone())
                .ok_or_else(|| {
                    error(
                        StatusCode::BAD_REQUEST,
                        "输入文件不存在或会话已清空，请重新选择文件",
                    )
                })
        })
        .collect::<ApiResult<_>>()?;
    if inputs.len() > 2 {
        return Err(error(StatusCode::BAD_REQUEST, "一次最多选择两个输入文件"));
    }
    fs::create_dir_all(directory.join("output"))?;
    if request.tool == "image" && inputs.len() == 1 {
        match super::media_guard::image_alias(&inputs[0], &directory) {
            Ok(path) => inputs[0] = path,
            Err(error) => {
                let _ = fs::remove_dir_all(&directory);
                return Err(error.into());
            }
        }
    }
    // 在入队前做同一套严格校验，让非法字段/路径不获得任务能力。
    let plan = match commands::prepare(&request, &inputs, &directory.join("output")) {
        Ok(plan) => plan,
        Err(error) => {
            let _ = fs::remove_dir_all(&directory);
            return Err(error.into());
        }
    };
    store.active += 1;
    store.jobs.insert(
        id.clone(),
        Job {
            id: id.clone(),
            state: "running",
            stdout: String::new(),
            report: None,
            files: vec![],
            error: None,
        },
    );
    drop(store);
    let job_id = id.clone();
    tokio::task::spawn_blocking(move || execute(session, request.tool, job_id, directory, plan));
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":id,"state":"running"})),
    ))
}

async fn job(
    State(session): State<Arc<Session>>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Json<Job>> {
    let store = session
        .store
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "会话状态不可用"))?;
    Ok(Json(store.jobs.get(&id).cloned().ok_or_else(|| {
        error(StatusCode::NOT_FOUND, "任务不存在或会话已清空")
    })?))
}

async fn download(
    State(session): State<Arc<Session>>,
    ApiPath(id): ApiPath<String>,
) -> ApiResult<Response> {
    let file = session
        .store
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "会话状态不可用"))?
        .files
        .get(&id)
        .cloned()
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "结果不存在或会话已清空"))?;
    let metadata = fs::symlink_metadata(&file.path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(error(StatusCode::BAD_REQUEST, "结果不是普通文件"));
    }
    let input = tokio::fs::File::open(&file.path).await?;
    let encoded: String = url::form_urlencoded::byte_serialize(file.name.as_bytes()).collect();
    let mut response = Body::from_stream(ReaderStream::new(input)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&file.mime)
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "结果类型无效"))?,
    );
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, HeaderValue::from(file.size));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename*=UTF-8''{encoded}"))
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "结果名称无效"))?,
    );
    Ok(response)
}

async fn clear(State(session): State<Arc<Session>>) -> ApiResult<Json<Value>> {
    let mut store = session
        .store
        .lock()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "会话状态不可用"))?;
    if store.active > 0 {
        return Err(error(StatusCode::CONFLICT, "任务仍在运行，请完成后再清空"));
    }
    for child in ["uploads", "jobs"] {
        let path = session.root.join(child);
        fs::remove_dir_all(&path)?;
        fs::create_dir(&path)?;
    }
    *store = Store::default();
    Ok(Json(json!({"cleared":true})))
}

async fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "没有找到这个入口").into_response()
}

fn backend_status() -> Value {
    // 只在启动时做有时间上限的可执行文件探测；没有后端不妨碍内置能力使用。
    let image = crate::backend::magick_version().and_then(|version| {
        ensure!(version.contains("ImageMagick 7"), "需要 ImageMagick 7");
        Ok(version)
    });
    let video = crate::backend::version(
        &crate::backend::program("KITO_FFMPEG", "ffmpeg"),
        &["-version"],
    )
    .and_then(|version| {
        crate::backend::version(
            &crate::backend::program("KITO_FFPROBE", "ffprobe"),
            &["-version"],
        )?;
        Ok(version)
    });
    let key = crate::backend::version(
        &crate::backend::program("KITO_OPENSSL", "openssl"),
        &["version"],
    )
    .and_then(|version| {
        ensure!(version.starts_with("OpenSSL 3."), "需要 OpenSSL 3.x");
        Ok(version)
    });
    fn status(result: Result<String>) -> Value {
        match result {
            Ok(message) => json!({"available":true,"message":message}),
            Err(error) => json!({"available":false,"message":format!("{error:#}")}),
        }
    }
    json!({"image":status(image),"video":status(video),"key":status(key)})
}

pub async fn serve(port: u16, no_open: bool) -> Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .context("无法监听本机端口，请换一个 --port 或使用 0 自动分配")?;
    let address = listener.local_addr()?;
    let temporary = tempfile::Builder::new().prefix("kito-web-").tempdir()?;
    let root = temporary.path().join("session");
    crate::key::storage::create_private_directory(&root)?;
    fs::create_dir(root.join("uploads"))?;
    fs::create_dir(root.join("jobs"))?;
    let token = random_id()?;
    let origin = format!("http://{address}");
    let url = format!("{origin}/#token={token}");
    let capabilities = tokio::task::spawn_blocking(backend_status).await?;
    let session = Arc::new(Session {
        _temporary: temporary,
        root,
        executable: std::env::current_exe()?,
        origin,
        host: address.to_string(),
        token,
        store: Mutex::new(Store::default()),
        stopping: AtomicBool::new(false),
        requests: Arc::new(Semaphore::new(4)),
        capabilities,
    });
    let router = Router::new()
        .route("/", get(assets::index))
        .route("/index.html", get(assets::index))
        .route("/api/info", get(info))
        .route("/api/uploads", post(upload))
        .route("/api/jobs", post(start_job))
        .route("/api/jobs/{id}", get(job))
        .route("/api/files/{id}", get(download))
        .route("/api/session", delete(clear))
        .route("/{*asset}", get(assets::asset))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(session.clone(), guard))
        .with_state(session.clone());
    println!("Kito 工具小队已就绪：\n{url}\n仅在本机运行。按 Ctrl+C 停止服务并清理本次临时文件。");
    std::io::stdout().flush()?;
    if !no_open {
        super::open_browser(&url);
    }
    let shutdown = session.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                if let Ok(mut terminate) =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                {
                    tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
                } else {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
            shutdown.stopping.store(true, Ordering::Relaxed);
        })
        .await?;
    session.stopping.store(true, Ordering::Relaxed);
    while session.store.lock().map(|s| s.active > 0).unwrap_or(false) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_are_display_metadata_not_paths() {
        for name in [
            "../key.pem",
            "/etc/passwd",
            "C:\\key.pem",
            "foo\r\nbar",
            "..",
            "",
        ] {
            assert!(safe_name(name).is_err());
        }
        assert!(safe_name("山间来信.png").is_ok());
    }
    #[test]
    fn token_requires_the_full_random_value() {
        assert!(constant_time_equal(b"abc", b"abc"));
        assert!(!constant_time_equal(b"abc", b"ab"));
        assert!(!constant_time_equal(b"abc", b"abd"));
    }
}
