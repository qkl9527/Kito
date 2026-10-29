mod common;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 每例独立进程和配置目录；断言失败时 Drop 仍会回收服务进程。
struct Server {
    child: Child,
    _directory: tempfile::TempDir,
    address: String,
    token: String,
}

struct Response {
    status: u16,
    body: Vec<u8>,
}

impl Response {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|error| {
            panic!("HTTP {} 返回非 JSON: {error}: {:?}", self.status, self.body)
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // 让服务执行会话临时目录清理；只有超时才强制结束。
            // SAFETY: pid 来自仍由 Child 持有的子进程，SIGTERM 无指针参数。
            unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM) };
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Server {
    fn start() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_kito"))
            .args(["web", "--no-open", "--port", "0"])
            .env("KITO_CONFIG_DIR", directory.path().join("config"))
            .env_remove("KITO_TIMEZONE")
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(start) = line.find("http://127.0.0.1:") {
                    let _ = sender.send(line[start..].trim().to_owned());
                    break;
                }
            }
        });
        // 先建立 RAII 容器再等待，启动失败也不会留下后台服务。
        let mut server = Self {
            child,
            _directory: directory,
            address: String::new(),
            token: String::new(),
        };
        let url = receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("web 应启动并输出回环 URL");
        let (address, token) = url
            .strip_prefix("http://")
            .unwrap()
            .split_once("/#token=")
            .unwrap();
        assert!(address.starts_with("127.0.0.1:"));
        assert!(token.len() >= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()));
        server.address = address.to_owned();
        server.token = token.to_owned();
        server
    }

    /// 小型真实 HTTP 客户端：逐字节保留下载体，并处理 chunked 响应。
    fn request(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        headers: &[(&str, &str)],
        authenticated: bool,
    ) -> Response {
        let mut stream = TcpStream::connect(&self.address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .unwrap();
        let host = headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("Host"))
            .map(|(_, value)| *value)
            .unwrap_or(&self.address);
        write!(stream, "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nContent-Length: {}\r\n", body.len()).unwrap();
        if authenticated {
            write!(stream, "X-Kito-Token: {}\r\n", self.token).unwrap();
        }
        for (name, value) in headers {
            if !name.eq_ignore_ascii_case("Host") {
                write!(stream, "{name}: {value}\r\n").unwrap();
            }
        }
        stream.write_all(b"\r\n").unwrap();
        stream.write_all(body).unwrap();
        stream.flush().unwrap();
        // 不关闭写侧：Hyper 可能将半关闭视为请求取消。
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).unwrap();
        let _ = stream.shutdown(Shutdown::Both);
        let split = raw
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .unwrap();
        let head = String::from_utf8_lossy(&raw[..split]);
        let status = head.split_whitespace().nth(1).unwrap().parse().unwrap();
        let mut body = raw[split + 4..].to_vec();
        if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            let mut decoded = Vec::new();
            let mut cursor = body.as_slice();
            loop {
                let end = cursor
                    .windows(2)
                    .position(|bytes| bytes == b"\r\n")
                    .unwrap();
                let length = usize::from_str_radix(
                    std::str::from_utf8(&cursor[..end])
                        .unwrap()
                        .split(';')
                        .next()
                        .unwrap(),
                    16,
                )
                .unwrap();
                if length == 0 {
                    break;
                }
                cursor = &cursor[end + 2..];
                decoded.extend_from_slice(&cursor[..length]);
                cursor = &cursor[length + 2..];
            }
            body = decoded;
        }
        Response { status, body }
    }

    fn get(&self, path: &str) -> Response {
        self.request("GET", path, &[], &[], true)
    }

    fn upload(&self, name: &str, bytes: &[u8]) -> String {
        let response = self.request(
            "POST",
            &format!("/api/uploads?name={name}"),
            bytes,
            &[],
            true,
        );
        assert!(
            (200..300).contains(&response.status),
            "{:?}",
            response.json()
        );
        let result = response.json();
        assert_eq!(result["size"], bytes.len());
        result["id"].as_str().unwrap().to_owned()
    }

    fn download_first(&self, result: &Value) -> Vec<u8> {
        let file = &result["files"][0];
        let id = file["id"].as_str().expect("任务应提供下载文件");
        let response = self.get(&format!("/api/files/{id}"));
        assert_eq!(response.status, 200);
        assert_eq!(file["size"], response.body.len());
        response.body
    }

    fn run(&self, request: Value) -> Value {
        let result = self.run_to_end(request);
        assert_eq!(result["state"], "completed", "{result}");
        result
    }

    fn run_to_end(&self, request: Value) -> Value {
        let response = self.request(
            "POST",
            "/api/jobs",
            &serde_json::to_vec(&request).unwrap(),
            &[("Content-Type", "application/json")],
            true,
        );
        assert_eq!(response.status, 202, "{:?}", response.json());
        let initial = response.json();
        assert_eq!(initial["state"], "running");
        let id = initial["id"].as_str().unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let result = self.get(&format!("/api/jobs/{id}")).json();
            if result["state"] != "running" {
                return result;
            }
            assert!(Instant::now() < deadline, "任务未结束: {result}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn web_help_has_examples_between_usage_and_options() {
    for help in ["-h", "--help"] {
        let output = common::cli(&["web", help]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        let usage = text.find("Usage:").unwrap();
        let examples = text.find("使用案例").unwrap();
        let options = text.find("Options:").unwrap();
        assert!(usage < examples && examples < options, "{text}");
        assert!(text.contains("--no-open") && text.contains("--port"));
    }
}

#[test]
fn web_authentication_and_static_resources_are_isolated() {
    let server = Server::start();
    assert_eq!(server.get("/api/info").status, 200);
    for headers in [
        vec![],
        vec![("Origin", "https://evil.example")],
        vec![("Host", "evil.example")],
    ] {
        let authenticated = !headers.is_empty();
        let response = server.request("GET", "/api/info", &[], &headers, authenticated);
        assert!((400..500).contains(&response.status), "{}", response.status);
        assert!(response.json()["error"].is_string());
    }
    let origin = format!("http://{}", server.address);
    assert_eq!(
        server
            .request("GET", "/api/info", &[], &[("Origin", &origin)], true)
            .status,
        200
    );
    for path in ["/", "/app.js", "/style.css"] {
        let response = server.request("GET", path, &[], &[], false);
        assert!(!String::from_utf8_lossy(&response.body).contains(&server.token));
    }
    for path in [
        "/../Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/api/files/../../Cargo.toml",
    ] {
        let response = server.get(path);
        assert!(response.status >= 400);
        assert!(!String::from_utf8_lossy(&response.body).contains("[package]"));
    }
    for request in [
        json!({"tool":"shell","action":"run","params":{},"files":[]}),
        json!({"tool":"id","action":"uuid","params":{"output":"/tmp/arbitrary"},"files":[]}),
    ] {
        let response = server.request(
            "POST",
            "/api/jobs",
            &serde_json::to_vec(&request).unwrap(),
            &[("Content-Type", "application/json")],
            true,
        );
        assert!(
            (400..500).contains(&response.status),
            "{:?}",
            response.json()
        );
    }
}

#[test]
fn web_png_and_binary_downloads_preserve_bytes_and_clear_session() {
    let server = Server::start();
    let original = common::rgba();
    let source = server._directory.path().join("original.png");
    std::fs::write(&source, &original).unwrap();
    let input = server.upload("original.png", &original);
    let result =
        server.run(json!({"tool":"image","action":"compress","params":{},"files":[input]}));
    let file = result["files"][0]["id"].as_str().unwrap();
    let file_path = format!("/api/files/{file}");
    let download = server.get(&file_path);
    assert_eq!(download.status, 200);
    let mut reader = png::Decoder::new(std::io::Cursor::new(download.body))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        &pixels[..info.buffer_size()],
        &[12, 34, 56, 0, 200, 100, 50, 255]
    );
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert!(server.request("GET", &file_path, &[], &[], false).status >= 400);

    let result = server.run(
        json!({"tool":"codec","action":"base64-decode","params":{"text":"AP9BQgo="},"files":[]}),
    );
    let binary = result["files"][0]["id"].as_str().unwrap();
    assert_eq!(
        server.get(&format!("/api/files/{binary}")).body,
        [0, 255, 65, 66, 10]
    );
    let clear = server.request("DELETE", "/api/session", &[], &[], true);
    assert!((200..300).contains(&clear.status));
    assert!(server.get(&file_path).status >= 400);
    assert!(server.get(&format!("/api/files/{binary}")).status >= 400);
}

#[test]
fn web_text_time_and_uuid_execute_real_commands() {
    let server = Server::start();
    for request in [
        json!({"tool":"text","action":"count","params":{"text":"hello 世界"},"files":[]}),
        json!({"tool":"time","action":"now","params":{},"files":[]}),
        json!({"tool":"id","action":"uuid","params":{},"files":[]}),
    ] {
        let result = server.run(request);
        assert!(
            result["stdout"]
                .as_str()
                .is_some_and(|text| !text.is_empty())
                || !result["report"].is_null(),
            "{result}"
        );
    }
}

#[test]
fn web_text_codec_and_hash_have_exact_results() {
    let server = Server::start();
    let count = server.run(json!({"tool":"text","action":"count","params":{"text":"hello 世界"}}));
    assert_eq!(count["stdout"].as_str().unwrap().trim(), "8");
    for (tool, action, params, expected) in [
        (
            "text",
            "dedupe",
            json!({"text":"a\r\nb\na\n"}),
            b"a\r\nb\n".as_slice(),
        ),
        (
            "text",
            "case",
            json!({"text":"AbC 世界", "mode":"lower"}),
            "abc 世界".as_bytes(),
        ),
        (
            "codec",
            "base64-encode",
            json!({"text":"abc"}),
            b"YWJj".as_slice(),
        ),
        (
            "codec",
            "html-decode",
            json!({"text":"&lt;p&gt;&amp;"}),
            b"<p>&".as_slice(),
        ),
    ] {
        let result = server.run(json!({"tool":tool,"action":action,"params":params}));
        assert_eq!(server.download_first(&result), expected, "{tool}/{action}");
    }
    let url = server.run(json!({"tool":"codec","action":"url-parse","params":{"text":"https://example.com/?a=1&a=2&empty="}}));
    let content = String::from_utf8(server.download_first(&url)).unwrap();
    let query_json = content
        .split_once("Query JSON:\n")
        .expect("URL 应提供可复制 Query JSON")
        .1;
    let query: Value = serde_json::from_str(query_json).unwrap();
    assert_eq!(query, json!({"a":["1","2"],"empty":""}));
    let hash = server.run(json!({"tool":"id","action":"hash","params":{"text":"abc"}}));
    assert_eq!(
        hash["report"]["digest"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(hash["report"]["input_bytes"], "3");
    let epoch = server.run(json!({"tool":"time","action":"from-timestamp","params":{"value":"0","unit":"s","timezone":"UTC"}}));
    assert!(
        epoch["report"]["utc"]
            .as_str()
            .unwrap()
            .starts_with("1970-01-01T00:00:00")
    );
    let uuid = server.run(json!({"tool":"id","action":"uuid","params":{}}));
    let value = uuid::Uuid::parse_str(uuid["report"]["values"][0].as_str().unwrap()).unwrap();
    assert_eq!(value.get_version_num(), 4);
}

#[test]
fn web_sound_downloads_are_real_pcm_and_midi() {
    let server = Server::start();
    for (action, params) in [
        (
            "sfx",
            json!({"preset":"coin","duration":"0.05","sample_rate":"22050"}),
        ),
        (
            "tone",
            json!({"pitch":"A4","duration":"0.05","sample_rate":"22050"}),
        ),
        (
            "beat",
            json!({"preset":"basic","bars":"1","bpm":"400","sample_rate":"22050"}),
        ),
    ] {
        let result = server.run(json!({"tool":"sound","action":action,"params":params}));
        assert_eq!(result["report"]["format"], "wav");
        assert_eq!(result["report"]["sample_rate"], 22050);
        let bytes = server.download_first(&result);
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes(bytes[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(bytes[34..36].try_into().unwrap()), 16);
        assert!(
            bytes[44..].iter().any(|byte| *byte != 0),
            "{action} 应包含实际音频采样"
        );
    }
    let melody = server
        .run(json!({"tool":"sound","action":"melody","params":{"text":"C4 E4 G4","format":"mid"}}));
    let bytes = server.download_first(&melody);
    assert_eq!(&bytes[..4], b"MThd");
    assert_eq!(&bytes[8..14], &[0, 0, 0, 1, 1, 224]);
    assert_eq!(&bytes[14..18], b"MTrk");
    assert!(
        bytes
            .windows(3)
            .any(|event| event[0] == 0x90 && event[1] == 60 && event[2] > 0)
    );
    assert_eq!(melody["report"]["event_count"], 3);
}

#[test]
fn web_image_read_reports_and_builtin_conversion_are_real() {
    let server = Server::start();
    let upload = server.upload("pixels.png", &common::rgba());
    let info = server.run(json!({"tool":"image","action":"info","files":[upload]}));
    assert_eq!(info["report"]["width"], 2);
    assert_eq!(info["report"]["height"], 1);
    let exif = server.run(json!({"tool":"image","action":"exif","files":[upload]}));
    assert_eq!(exif["report"]["has_exif"], false);
    assert_eq!(exif["report"]["fields"], json!([]));
    let converted = server
        .run(json!({"tool":"image","action":"convert","params":{"to":"png"},"files":[upload]}));
    let bytes = server.download_first(&converted);
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(
        &pixels[..frame.buffer_size()],
        &[12, 34, 56, 0, 200, 100, 50, 255]
    );
}

#[test]
fn web_rejects_scripts_and_documents_disguised_as_png() {
    let server = Server::start();
    for (kind, bytes) in [
        (
            "SVG",
            b"<svg xmlns='http://www.w3.org/2000/svg'/ >".as_slice(),
        ),
        ("PDF", b"%PDF-1.7\n".as_slice()),
        ("MVG", b"push graphic-context\nviewbox 0 0 10 10".as_slice()),
        (
            "MSL",
            b"<?xml version='1.0'?><image><read filename='Cargo.toml'/></image>".as_slice(),
        ),
    ] {
        let upload = server.upload("disguised.png", bytes);
        for action in ["info", "convert"] {
            let response = server.request(
                "POST",
                "/api/jobs",
                &serde_json::to_vec(&json!({"tool":"image","action":action,"files":[upload]}))
                    .unwrap(),
                &[("Content-Type", "application/json")],
                true,
            );
            assert_eq!(
                response.status,
                400,
                "{kind}/{action}: {:?}",
                response.json()
            );
            assert!(
                response.json()["error"]
                    .as_str()
                    .unwrap()
                    .contains("像素容器")
            );
        }
    }
}

#[test]
fn web_encrypted_key_roundtrip_keeps_secrets_in_downloads_only() {
    let server = Server::start();
    let capabilities = server.get("/api/info").json();
    if capabilities["backends"]["key"]["available"] != true {
        assert_ne!(
            std::env::var("KITO_REQUIRE_BACKEND").as_deref(),
            Ok("1"),
            "要求真实 OpenSSL 3.x 后端：{capabilities}"
        );
        eprintln!("跳过密钥 Web 集成：本机未提供 OpenSSL 3.x");
        return;
    }
    let passphrase = "test-only-web-passphrase";
    let generated = server.run(json!({"tool":"key","action":"generate","params":{"algorithm":"ed25519","passphrase":passphrase}}));
    assert_eq!(generated["report"]["algorithm"], "ed25519");
    assert_eq!(generated["report"]["encrypted"], true);
    assert_eq!(generated["files"].as_array().unwrap().len(), 2);
    let serialized = serde_json::to_string(&generated).unwrap();
    assert!(!serialized.contains(passphrase));
    assert!(!serialized.contains("-----BEGIN"));
    let mut private = None;
    let mut public = None;
    for file in generated["files"].as_array().unwrap() {
        let response = server.get(&format!("/api/files/{}", file["id"].as_str().unwrap()));
        assert_eq!(response.status, 200);
        assert_eq!(file["size"], response.body.len());
        match file["name"].as_str().unwrap() {
            "private.pem" => private = Some(response.body),
            "public.pem" => public = Some(response.body),
            name => panic!("意外密钥文件 {name}"),
        }
    }
    let private = private.unwrap();
    let public = public.unwrap();
    assert!(private.starts_with(b"-----BEGIN ENCRYPTED PRIVATE KEY-----"));
    assert!(public.starts_with(b"-----BEGIN PUBLIC KEY-----"));
    let private_id = server.upload("private.pem", &private);
    let public_id = server.upload("public.pem", &public);
    let checked = server.run(json!({"tool":"key","action":"check","params":{"passphrase":passphrase},"files":[private_id,public_id]}));
    assert_eq!(checked["report"]["validity"], true);
    assert_eq!(checked["report"]["match"], true);
    assert_eq!(
        checked["report"]["fingerprint"],
        generated["report"]["fingerprint"]
    );
    assert!(checked["files"].as_array().unwrap().is_empty());
    assert!(
        !serde_json::to_string(&checked)
            .unwrap()
            .contains(passphrase)
    );
    let wrong = "test-only-wrong-passphrase";
    let failed = server.run_to_end(json!({"tool":"key","action":"check","params":{"passphrase":wrong},"files":[private_id,public_id]}));
    assert_eq!(failed["state"], "failed");
    assert!(failed["files"].as_array().unwrap().is_empty());
    assert!(failed["report"].is_null());
    let serialized = serde_json::to_string(&failed).unwrap();
    assert!(!serialized.contains(wrong) && !serialized.contains("-----BEGIN"));
}

#[test]
fn web_video_mute_download_is_decodable_and_rejects_disguised_playlists() {
    let server = Server::start();
    let capabilities = server.get("/api/info").json();
    if capabilities["backends"]["video"]["available"] != true {
        assert_ne!(
            std::env::var("KITO_REQUIRE_BACKEND").as_deref(),
            Ok("1"),
            "要求真实 FFmpeg / ffprobe 后端：{capabilities}"
        );
        assert!(
            std::env::var_os("KITO_FFMPEG").is_none() && std::env::var_os("KITO_FFPROBE").is_none(),
            "显式指定的视频后端不可用，不能跳过"
        );
        eprintln!("跳过视频 Web 集成：本机未提供 FFmpeg / ffprobe");
        return;
    }
    let ffmpeg = std::env::var_os("KITO_FFMPEG").unwrap_or_else(|| "ffmpeg".into());
    let ffprobe = std::env::var_os("KITO_FFPROBE").unwrap_or_else(|| "ffprobe".into());
    let fixture = server._directory.path().join("fixture.mp4");
    // 在独立临时目录合成带音轨的短片，不读取用户媒体、声卡或网络来源。
    let generated = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x48:rate=10:duration=0.3",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.3",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&fixture)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let original = std::fs::read(&fixture).unwrap();
    let upload = server.upload("fixture.mp4", &original);
    let result = server.run(json!({"tool":"video","action":"mute","files":[upload]}));
    assert_eq!(result["report"]["verified_decodable"], true);
    let bytes = server.download_first(&result);
    assert!(bytes.len() > 32);
    assert_eq!(&bytes[4..8], b"ftyp");
    let downloaded = server._directory.path().join("downloaded.mp4");
    std::fs::write(&downloaded, bytes).unwrap();
    let probe = Command::new(&ffprobe)
        .args(["-v", "error", "-show_streams", "-of", "json"])
        .arg(&downloaded)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let probe: Value = serde_json::from_slice(&probe.stdout).unwrap();
    let streams = probe["streams"].as_array().unwrap();
    assert_eq!(streams.len(), 1, "mute 应只保留视频轨：{probe}");
    assert_eq!(streams[0]["codec_type"], "video");
    assert_eq!(streams[0]["width"], 64);
    assert_eq!(streams[0]["height"], 48);
    // 再完整解码下载文件，避免只凭容器头或 ffprobe 元数据判断文件完整。
    let decoded = Command::new(&ffmpeg)
        .args(["-v", "error", "-nostdin", "-xerror", "-i"])
        .arg(&downloaded)
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        decoded.status.success(),
        "{}",
        String::from_utf8_lossy(&decoded.stderr)
    );
    assert_eq!(std::fs::read(&fixture).unwrap(), original);

    let playlist = server.upload("disguised.mp4", b"#EXTM3U\n#EXT-X-VERSION:3\n#EXTINF:1,\nfile:///definitely-not-a-kito-fixture.ts\n#EXT-X-ENDLIST\n");
    let failed = server.run_to_end(json!({"tool":"video","action":"mute","files":[playlist]}));
    assert_eq!(failed["state"], "failed");
    assert!(failed["files"].as_array().unwrap().is_empty());
    assert!(failed["report"].is_null());
}
