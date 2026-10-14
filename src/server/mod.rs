/// HTTP 服务模块 - 对应 Node.js 版本的 server.js
///
/// 使用 Axum 框架，将 ApiClient 的所有方法自动映射为 REST API 路由
/// 前端可以像调用 Node.js 版一样通过 HTTP 请求调用
pub mod middleware;
pub mod upload;

use crate::api::Query;
use crate::request::{ApiClient, ApiResponse};
use axum::extract::State;
use axum::http::header;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

// ============================================================
//  服务器配置
// ============================================================

/// 服务器配置
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// 监听地址，默认 "0.0.0.0"
    pub host: String,
    /// 监听端口，默认 3000
    pub port: u16,
    /// CORS 允许的 Origin，None 表示允许所有
    pub cors_origin: Option<String>,
    /// 限流：时间窗口内最大请求数，0 表示不限流
    pub rate_limit: u64,
    /// 限流：时间窗口秒数
    pub rate_limit_window: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 3000,
            cors_origin: None,
            rate_limit: 0,
            rate_limit_window: 60,
        }
    }
}

impl ServerConfig {
    /// 从环境变量读取配置
    /// - `NCM_HOST`: 监听地址
    /// - `NCM_PORT`: 监听端口
    /// - `CORS_ALLOW_ORIGIN`: CORS 允许的 Origin
    /// - `RATE_LIMIT`: 每个时间窗口的最大请求数（默认不限流）
    /// - `RATE_LIMIT_WINDOW`: 限流时间窗口秒数（默认 60）
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("NCM_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("NCM_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            cors_origin: std::env::var("CORS_ALLOW_ORIGIN").ok(),
            rate_limit: std::env::var("RATE_LIMIT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(0),
            rate_limit_window: std::env::var("RATE_LIMIT_WINDOW")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(60),
        }
    }
}

// ============================================================
//  共享状态
// ============================================================

/// Axum 共享状态
#[derive(Clone)]
pub struct AppState {
    pub client: Arc<ApiClient>,
}

// ============================================================
//  查询参数提取
// ============================================================

/// 从 HTTP 请求中提取并合并查询参数
///
/// 合并优先级（后者覆盖前者）：Cookie < URL Query < POST Body
async fn extract_merged_query(
    headers: &HeaderMap,
    uri_query: Option<&str>,
    body: axum::body::Bytes,
    content_type: Option<&str>,
) -> Query {
    let mut query = Query::new();

    // 1. 提取 URL query 参数
    if let Some(qs) = uri_query {
        if let Ok(params) = serde_urlencoded::from_str::<HashMap<String, String>>(qs) {
            for (k, v) in params {
                query.params.insert(k, v);
            }
        }
    }

    // 2. 提取 POST body 参数
    if !body.is_empty() {
        let ct = content_type.unwrap_or("");
        if ct.contains("application/json") {
            if let Ok(map) = serde_json::from_slice::<HashMap<String, Value>>(&body) {
                for (k, v) in map {
                    let s = match &v {
                        Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    query.params.insert(k, s);
                }
            }
        } else {
            // form-urlencoded 或其他格式
            if let Ok(params) = serde_urlencoded::from_bytes::<HashMap<String, String>>(&body) {
                for (k, v) in params {
                    query.params.insert(k, v);
                }
            }
        }
    }

    // 3. 处理 cookie
    // 优先使用参数中传入的 cookie
    if let Some(cookie_param) = query.params.remove("cookie") {
        query.cookie = Some(cookie_param);
    } else if let Some(cookie_header) = headers.get(header::COOKIE) {
        if let Ok(c) = cookie_header.to_str() {
            query.cookie = Some(c.to_string());
        }
    }

    // 4. 传输层选项（realIP / proxy）+ 出站请求选项（util/option.js）
    apply_transport_options(&mut query);

    // 5. 调用方 IP（对应 server.js 里的 `obj[2].ip`）
    apply_client_ip(&mut query, headers);

    query
}

// ============================================================
//  传输层选项（realIP / proxy）
// ============================================================

/// 把 params 里的 `realIP` / `proxy` 提升到 [`Query`] 字段，再套用
/// [`apply_request_options`]。
///
/// 普通路由与 multipart 路由共用，避免两处规则漂移。
///
/// 空串按「未提供」处理——上游是 `options.realIP || options.ip` 与
/// `if (options.proxy)`，空串在 JS 里都是假值。
pub(crate) fn apply_transport_options(query: &mut Query) {
    if let Some(real_ip) = query.params.remove("realIP") {
        if !real_ip.is_empty() {
            query.real_ip = Some(real_ip);
        }
    }

    if let Some(proxy) = query.params.remove("proxy") {
        // 空串会让 `reqwest::Proxy::all("")` 报错，必须过滤
        if !proxy.is_empty() {
            query.proxy = Some(proxy);
        }
    }

    apply_request_options(query);
}

// ============================================================
//  出站请求选项（对应 util/option.js）
// ============================================================

/// 复刻 `util/option.js` 里 `randomCNIP` 的取值
///
/// ```js
/// process.env.ENABLE_RANDOM_CN_IP === 'true'
///   ? !['false', false].includes(query.randomCNIP)
///   : ['true', true].includes(query.randomCNIP)
/// ```
///
/// 两个容易踩的 JS 语义点：
/// - env 开启时**参数缺失也算开启**（`!includes(undefined)` 为真）
/// - env 关闭时只有字面量 `'true'` 算开启，`'1'` **不算**（用的是 includes 精确匹配，
///   不是 `toBoolean`）
pub(crate) fn resolve_random_cn_ip(env_enabled: bool, raw: Option<&str>) -> bool {
    if env_enabled {
        raw != Some("false")
    } else {
        raw == Some("true")
    }
}

/// 把 URL query / 表单里的「出站请求选项」归一化到 [`Query`] 上。
///
/// 复刻上游 `util/option.js` 的取值语义——重点是 JS 的 falsy 判定：
/// `''` 是假值、`'0'` / `'false'` 反而是真值。
///
/// ```js
/// crypto: query.crypto || crypto || '',      // 已在 Query::to_option 处理
/// ua: query.ua || '',
/// e_r: query.e_r || undefined,
/// domain: query.domain || '',
/// checkToken: query.checkToken || checkToken, // 模块侧恒为 false
/// randomCNIP: process.env.ENABLE_RANDOM_CN_IP === 'true'
///   ? !['false', false].includes(query.randomCNIP)
///   : ['true', true].includes(query.randomCNIP),
/// ```
pub(crate) fn apply_request_options(query: &mut Query) {
    // `domain: query.domain || ''`——上游 request.js 用 `options.domain || DOMAIN`，
    // 空串等价于未设置
    if let Some(domain) = query.params.remove("domain") {
        if !domain.is_empty() {
            query.domain = Some(domain);
        }
    }

    // `ua: query.ua || ''`——上游 request.js 用 `options.ua || chooseUserAgent(...)`，
    // 空串会被 `||` 回退掉
    if let Some(ua) = query.params.remove("ua") {
        if !ua.is_empty() {
            query.ua = Some(ua);
        }
    }

    // `e_r: query.e_r || undefined`——空串是 falsy，等价于未提供
    // （request.js 随后会回落到 `ENCRYPT_RESPONSE`）
    if let Some(e_r) = query.params.remove("e_r") {
        if !e_r.is_empty() {
            query.e_r = Some(crate::api::js_to_boolean(&e_r));
        }
    }

    // randomCNIP 的 env 分支：见 resolve_random_cn_ip
    let raw_random_cn_ip = query.params.remove("randomCNIP");
    let env_enabled = std::env::var("ENABLE_RANDOM_CN_IP").as_deref() == Ok("true");
    query.random_cn_ip = resolve_random_cn_ip(env_enabled, raw_random_cn_ip.as_deref());

    // `checkToken: query.checkToken || checkToken`（模块侧恒为 false）
    // JS 真值判定：非空字符串即为真，所以 `'false'` 也是开启
    query.check_token = query
        .params
        .remove("checkToken")
        .map(|value| !value.is_empty())
        .unwrap_or(false);
}

/// 复刻上游 `server.js` 里 `obj[2].ip` 的计算
///
/// 上游对**每个**出站请求都会写入 `X-Real-IP` / `X-Forwarded-For`：
/// - `options.randomCNIP` 为真 → `global.cnIp`
/// - 否则 → 调用方 IP（`trust proxy` 下取 `X-Forwarded-For` 最左侧）；
///   等于 `'::1'` 时回退随机国内 IP
///
/// 上游最终取 `options.realIP || options.ip`，所以显式 `realIP` 优先级最高。
pub(crate) fn apply_client_ip(query: &mut Query, headers: &HeaderMap) {
    // 显式 ?realIP= 优先（对应 `options.realIP || options.ip`）
    if query.real_ip.is_some() {
        return;
    }

    // randomCNIP 交给 request 层按请求生成（上游是进程级 `global.cnIp` 复用），
    // 这里不能再注入 real_ip，否则 request.rs 里 real_ip 会盖掉随机 IP
    if query.random_cn_ip {
        return;
    }

    // 调用方 IP 由中间件写入内部头；头缺失时保持不注入
    let Some(ip) = headers
        .get(middleware::CLIENT_IP_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|ip| !ip.is_empty())
    else {
        return;
    };

    query.real_ip = Some(if ip == "::1" {
        crate::util::ip::generate_random_chinese_ip()
    } else {
        ip.to_string()
    });
}

// ============================================================
//  响应构建
// ============================================================

/// 构建成功响应
fn build_success_response(api_resp: ApiResponse) -> Response {
    let status = axum::http::StatusCode::from_u16(api_resp.status as u16)
        .unwrap_or(axum::http::StatusCode::OK);

    // 对应上游 server.js 的 `if (moduleResponse.redirectUrl) res.redirect(...)`。
    // `song_url_v1_302` 会把目标地址放在 `body.redirectUrl` 并返回 302，
    // 这里据此下发 `Location` 头（不发送 JSON body），与上游行为一致。
    if let Some(redirect_url) = api_resp.body.get("redirectUrl").and_then(|v| v.as_str()) {
        let mut response = status.into_response();
        if let Ok(val) = header::HeaderValue::from_str(redirect_url) {
            response.headers_mut().insert(header::LOCATION, val);
        }
        for cookie_str in &api_resp.cookie {
            if let Ok(val) = header::HeaderValue::from_str(cookie_str) {
                response.headers_mut().append(header::SET_COOKIE, val);
            }
        }
        return response;
    }

    let mut response = (status, Json(api_resp.body)).into_response();

    // 设置 API 返回的 Cookie
    for cookie_str in &api_resp.cookie {
        if let Ok(val) = header::HeaderValue::from_str(cookie_str) {
            response.headers_mut().append(header::SET_COOKIE, val);
        }
    }

    response
}

/// 构建错误响应
fn build_error_response(err: crate::error::NcmError) -> Response {
    use crate::error::NcmError;

    let (status, body) = match &err {
        NcmError::AuthRequired(msg) => (
            axum::http::StatusCode::UNAUTHORIZED,
            json!({ "code": 301, "msg": msg }),
        ),
        NcmError::InvalidParam(msg) => (
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "code": 400, "msg": msg }),
        ),
        NcmError::RateLimited(msg) => (
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            json!({ "code": 503, "msg": msg }),
        ),
        NcmError::Timeout(msg) => (
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            json!({ "code": 504, "msg": msg }),
        ),
        NcmError::Api { code, msg } => {
            let http_status = axum::http::StatusCode::from_u16(*code as u16)
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            (http_status, json!({ "code": code, "msg": msg }))
        }
        _ => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "code": 500, "msg": err.to_string() }),
        ),
    };

    (status, Json(body)).into_response()
}

// ============================================================
//  通用请求处理
// ============================================================

/// 通用 API 请求处理函数
async fn handle_api_request<F>(
    state: &AppState,
    headers: HeaderMap,
    uri: &axum::http::Uri,
    body: axum::body::Bytes,
    api_fn: F,
) -> Response
where
    F: for<'a> FnOnce(
        &'a ApiClient,
        &'a Query,
    ) -> Pin<
        Box<dyn Future<Output = crate::error::Result<ApiResponse>> + Send + 'a>,
    >,
{
    let path = uri.path().to_string();
    let start = std::time::Instant::now();

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok());

    let query = extract_merged_query(&headers, uri.query(), body, content_type).await;

    match api_fn(&state.client, &query).await {
        Ok(resp) => {
            tracing::info!("{} -> {} ({:.1?})", path, resp.status, start.elapsed());
            build_success_response(resp)
        }
        Err(e) => {
            tracing::warn!("{} -> ERROR: {} ({:.1?})", path, e, start.elapsed());
            build_error_response(e)
        }
    }
}

// ============================================================
//  路由注册宏
// ============================================================

/// 批量注册 API 路由的宏
///
/// 将每个 ApiClient 方法映射为 GET + POST 路由
macro_rules! api_routes {
    ($router:expr, $( $method:ident => $route:expr ),* $(,)?) => {{
        let router = $router;
        $(
            let router = router.route(
                $route,
                get(|State(state): State<AppState>, headers: HeaderMap, uri: axum::http::Uri| async move {
                    handle_api_request(&state, headers, &uri, axum::body::Bytes::new(), |client, q| Box::pin(client.$method(q))).await
                })
                .post(|State(state): State<AppState>, headers: HeaderMap, uri: axum::http::Uri, body: axum::body::Bytes| async move {
                    handle_api_request(&state, headers, &uri, body, |client, q| Box::pin(client.$method(q))).await
                }),
            );
        )*
        router
    }};
}

// ============================================================
//  路由注册
// ============================================================

/// 注册所有 API 路由
///
/// 标准路由由 build.rs 自动从 src/api/mod.rs 扫描生成。
/// 以下几类签名不适配通用宏（`api_routes!` 里直接把
/// `client.$method(q)` 当成 Future），需要单独注册：
/// - `/avatar/upload`、`/voice/upload`、`/cloud`、`/playlist/cover/update`：
///   需要 multipart 处理
/// - `/eapi/decrypt`：同步方法，返回值不是 Future
/// - `/inner/version`：不接受 `query` 参数
fn register_routes(router: Router<AppState>) -> Router<AppState> {
    // 自动生成的标准路由（build.rs → api_routes_generated.rs）
    let router = { include!(concat!(env!("OUT_DIR"), "/api_routes_generated.rs")) };

    // 文件上传路由（特殊签名，需要 multipart 处理）
    let router = router
        .route("/avatar/upload", post(upload::handle_avatar_upload))
        .route("/voice/upload", post(upload::handle_voice_upload))
        .route("/cloud", post(upload::handle_cloud_upload))
        .route(
            "/playlist/cover/update",
            post(upload::handle_playlist_cover_update),
        );

    // 签名与通用宏不符的路由，手动注册（上游同样暴露这两个路径）
    let router = router
        .route(
            "/eapi/decrypt",
            get(eapi_decrypt_handler).post(eapi_decrypt_handler),
        )
        .route(
            "/inner/version",
            get(inner_version_handler).post(inner_version_handler),
        );

    router
}

/// `/eapi/decrypt` 处理器
///
/// `ApiClient::eapi_decrypt` 是同步方法（只做本地解密、不发网络请求），
/// 所以这里用 `async move` 包一层再 `Box::pin`。
async fn eapi_decrypt_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    body: axum::body::Bytes,
) -> Response {
    handle_api_request(&state, headers, &uri, body, |client, q| {
        Box::pin(async move { client.eapi_decrypt(q) })
    })
    .await
}

/// `/inner/version` 处理器
///
/// `ApiClient::inner_version` 不接受 `query`（版本号取自编译期常量），
/// 请求参数照常解析后丢弃。
async fn inner_version_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
    body: axum::body::Bytes,
) -> Response {
    handle_api_request(&state, headers, &uri, body, |client, _q| {
        Box::pin(async move { client.inner_version().await })
    })
    .await
}

// ============================================================
//  构建与启动
// ============================================================

/// 构建 Axum 应用（不启动监听）
///
/// 可用于测试或自定义部署
pub fn build_app(client: ApiClient) -> Router {
    let state = AppState {
        client: Arc::new(client),
    };

    let router = Router::new();
    let router = register_routes(router);

    // 根路由
    let router = router.route(
        "/",
        get(|| async {
            Json(json!({
                "code": 200,
                "msg": "NCM API Rust Server is running",
            }))
        }),
    );

    router
        .layer(middleware::cors_layer(None))
        .layer(axum::middleware::from_fn(middleware::client_ip_middleware))
        .with_state(state)
}

/// 构建带自定义配置的 Axum 应用
pub fn build_app_with_config(client: ApiClient, config: &ServerConfig) -> Router {
    let state = AppState {
        client: Arc::new(client),
    };

    let router = Router::new();
    let router = register_routes(router);

    let router = router.route(
        "/",
        get(|| async {
            Json(json!({
                "code": 200,
                "msg": "NCM API Rust Server is running",
            }))
        }),
    );

    let router = router
        .layer(middleware::cors_layer(config.cors_origin.as_deref()))
        .layer(axum::middleware::from_fn(middleware::client_ip_middleware))
        .with_state(state);

    // 限流中间件（rate_limit > 0 时启用）
    if config.rate_limit > 0 {
        let limiter = middleware::RateLimiter::new(config.rate_limit, config.rate_limit_window);
        router.layer(axum::middleware::from_fn_with_state(
            limiter,
            middleware::rate_limit_middleware,
        ))
    } else {
        router
    }
}

/// 启动 HTTP 服务器
pub async fn start_server(config: ServerConfig) {
    let client = ApiClient::new(None);
    let app = build_app_with_config(client, &config);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    tracing::info!("NCM API Server listening on http://{}", addr);

    // 带连接信息启动：client_ip_middleware 依赖 ConnectInfo 拿到调用方 IP
    // （对应 Express 的 `req.ip`）
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .expect("Server error");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- randomCNIP 的 env 分支 ----------

    #[test]
    fn test_random_cn_ip_env_disabled() {
        // env 关闭时只有字面量 'true' 开启
        assert!(resolve_random_cn_ip(false, Some("true")));
        assert!(!resolve_random_cn_ip(false, Some("1")));
        assert!(!resolve_random_cn_ip(false, Some("false")));
        assert!(!resolve_random_cn_ip(false, None));
    }

    #[test]
    fn test_random_cn_ip_env_enabled() {
        // env 开启时默认开启，只有显式 'false' 关闭（参数缺失也算开启）
        assert!(resolve_random_cn_ip(true, None));
        assert!(resolve_random_cn_ip(true, Some("true")));
        assert!(resolve_random_cn_ip(true, Some("1")));
        assert!(resolve_random_cn_ip(true, Some("anything")));
        assert!(!resolve_random_cn_ip(true, Some("false")));
    }

    // ---------- util/option.js 的取值语义 ----------

    #[test]
    fn test_domain_and_ua_empty_is_absent() {
        // 空串在 JS 里是 falsy，等价于未提供
        let mut q = Query::new().param("domain", "").param("ua", "");
        apply_request_options(&mut q);
        assert!(q.domain.is_none());
        assert!(q.ua.is_none());

        let mut q = Query::new()
            .param("domain", "http://127.0.0.1:8080")
            .param("ua", "MyUA/1.0");
        apply_request_options(&mut q);
        assert_eq!(q.domain.as_deref(), Some("http://127.0.0.1:8080"));
        assert_eq!(q.ua.as_deref(), Some("MyUA/1.0"));
    }

    #[test]
    fn test_domain_and_ua_removed_from_params() {
        // 提升到字段后不应再留在 params 里（避免被当成普通请求数据）
        let mut q = Query::new()
            .param("domain", "http://x")
            .param("ua", "UA")
            .param("realIP", "1.2.3.4")
            .param("proxy", "http://p");
        apply_transport_options(&mut q);
        assert!(q.params.is_empty());
    }

    #[test]
    fn test_e_r_follows_to_boolean() {
        // 'true' / '1' 为真，其余为假；空串等价于未提供
        for (raw, expected) in [
            ("true", Some(true)),
            ("1", Some(true)),
            ("false", Some(false)),
            ("0", Some(false)),
            ("yes", Some(false)),
        ] {
            let mut q = Query::new().param("e_r", raw);
            apply_request_options(&mut q);
            assert_eq!(q.e_r, expected, "e_r={raw}");
        }

        let mut q = Query::new().param("e_r", "");
        apply_request_options(&mut q);
        assert_eq!(q.e_r, None);

        let mut q = Query::new();
        apply_request_options(&mut q);
        assert_eq!(q.e_r, None);
    }

    #[test]
    fn test_check_token_is_js_truthy() {
        // JS 真值判定：非空字符串即为真，'false' 也算开启
        for (raw, expected) in [
            ("v2", true),
            ("v3", true),
            ("false", true),
            ("0", true),
            ("", false),
        ] {
            let mut q = Query::new().param("checkToken", raw);
            apply_request_options(&mut q);
            assert_eq!(q.check_token, expected, "checkToken={raw}");
        }

        let mut q = Query::new();
        apply_request_options(&mut q);
        assert!(!q.check_token);
    }

    #[test]
    fn test_transport_empty_proxy_and_real_ip_ignored() {
        // 空串 proxy 会让 reqwest::Proxy::all("") 报错，必须过滤
        let mut q = Query::new().param("proxy", "").param("realIP", "");
        apply_transport_options(&mut q);
        assert!(q.proxy.is_none());
        assert!(q.real_ip.is_none());
    }

    // ---------- server.js 的调用方 IP 注入 ----------

    fn headers_with_client_ip(ip: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            middleware::CLIENT_IP_HEADER,
            axum::http::HeaderValue::from_str(ip).unwrap(),
        );
        headers
    }

    #[test]
    fn test_client_ip_injected_when_absent() {
        let mut q = Query::new();
        apply_client_ip(&mut q, &headers_with_client_ip("203.0.113.7"));
        assert_eq!(q.real_ip.as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn test_explicit_real_ip_wins() {
        // 上游 `options.realIP || options.ip`
        let mut q = Query::new().param("realIP", "1.1.1.1");
        apply_transport_options(&mut q);
        apply_client_ip(&mut q, &headers_with_client_ip("203.0.113.7"));
        assert_eq!(q.real_ip.as_deref(), Some("1.1.1.1"));
    }

    #[test]
    fn test_random_cn_ip_skips_injection() {
        // randomCNIP 打开时不能再注入 real_ip，否则会盖掉随机 IP
        let mut q = Query::new().param("randomCNIP", "true");
        apply_request_options(&mut q);
        assert!(q.random_cn_ip);
        apply_client_ip(&mut q, &headers_with_client_ip("203.0.113.7"));
        assert!(q.real_ip.is_none());
    }

    #[test]
    fn test_loopback_becomes_random_cn_ip() {
        // 上游：`if (ip == '::1') ip = global.cnIp`
        let mut q = Query::new();
        apply_client_ip(&mut q, &headers_with_client_ip("::1"));
        let ip = q.real_ip.expect("::1 应被替换成随机国内 IP");
        assert_ne!(ip, "::1");
        assert!(
            ip.parse::<std::net::Ipv4Addr>().is_ok(),
            "应替换为 IPv4，实际 {ip}"
        );
    }

    #[test]
    fn test_missing_client_ip_header_keeps_none() {
        // 中间件拿不到 IP 时会删掉该头，此时不应注入
        let mut q = Query::new();
        apply_client_ip(&mut q, &HeaderMap::new());
        assert!(q.real_ip.is_none());
    }

    #[test]
    fn test_loopback_v4_is_not_replaced() {
        // 与上游一致：只认 `::1`，`127.0.0.1` 原样透传
        let mut q = Query::new();
        apply_client_ip(&mut q, &headers_with_client_ip("127.0.0.1"));
        assert_eq!(q.real_ip.as_deref(), Some("127.0.0.1"));
    }
}
