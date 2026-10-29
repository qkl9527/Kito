//! 静态资源随可执行文件发布，不读取工作目录，不存在任意文件访问入口。
use axum::{
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};

pub async fn index() -> Response {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_bytes!("../../web/index.html").as_slice(),
    )
        .into_response()
}

pub async fn asset(Path(path): Path<String>) -> Response {
    let (content_type, bytes): (&str, &[u8]) = match path.as_str() {
        "styles.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../../web/styles.css"),
        ),
        "app.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../../web/app.js"),
        ),
        "assets/lumi.svg" => ("image/svg+xml", include_bytes!("../../web/assets/lumi.svg")),
        "assets/tiko.svg" => ("image/svg+xml", include_bytes!("../../web/assets/tiko.svg")),
        "assets/codi.svg" => ("image/svg+xml", include_bytes!("../../web/assets/codi.svg")),
        "assets/sumi.svg" => ("image/svg+xml", include_bytes!("../../web/assets/sumi.svg")),
        "assets/kavi.svg" => ("image/svg+xml", include_bytes!("../../web/assets/kavi.svg")),
        "assets/vido.svg" => ("image/svg+xml", include_bytes!("../../web/assets/vido.svg")),
        "assets/echo.svg" => ("image/svg+xml", include_bytes!("../../web/assets/echo.svg")),
        "assets/crew.svg" => ("image/svg+xml", include_bytes!("../../web/assets/crew.svg")),
        "assets/landscape.svg" => (
            "image/svg+xml",
            include_bytes!("../../web/assets/landscape.svg"),
        ),
        _ => return (StatusCode::NOT_FOUND, "资源不存在").into_response(),
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}
