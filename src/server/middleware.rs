/// 中间件配置 - CORS 和限流
use axum::http::{HeaderValue, Method};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// 创建 CORS 中间件层
///
/// - `origin`: 允许的 Origin，None 表示允许所有
///   支持逗号分隔多个源，例如 `"https://a.com,https://b.com"`
pub fn cors_layer(origin: Option<&str>) -> CorsLayer {
    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
        .allow_credentials(false);

    match origin {
        Some(o) => {
            let origins: Vec<String> = o
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if origins.is_empty() || origins.iter().any(|s| s == "*") {
                layer.allow_origin(Any)
            } else if origins.len() == 1 {
                if let Ok(val) = origins[0].parse::<HeaderValue>() {
                    layer
                        .allow_credentials(true)
                        .allow_origin(AllowOrigin::exact(val))
                } else {
                    layer.allow_origin(Any)
                }
            } else {
                layer
                    .allow_credentials(true)
                    .allow_origin(AllowOrigin::predicate(move |value, _| {
                        value
                            .to_str()
                            .map(|v| origins.iter().any(|o| o == v))
                            .unwrap_or(false)
                    }))
            }
        }
        None => layer.allow_origin(Any),
    }
}

// ============================================================
//  简易限流中间件
// ============================================================

use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// 限流器状态
#[derive(Clone)]
pub struct RateLimiter {
    /// IP -> (请求计数, 窗口起始时间)
    state: Arc<Mutex<HashMap<String, (u64, Instant)>>>,
    /// 时间窗口内最大请求数
    max_requests: u64,
    /// 时间窗口（秒）
    window_secs: u64,
}

impl RateLimiter {
    /// 创建限流器
    ///
    /// - `max_requests`: 时间窗口内允许的最大请求数
    /// - `window_secs`: 时间窗口秒数
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    /// 检查是否允许请求，返回 true 表示放行
    async fn check(&self, ip: &str) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let entry = state.entry(ip.to_string()).or_insert((0, now));

        // 窗口过期，重置计数
        if now.duration_since(entry.1).as_secs() >= self.window_secs {
            entry.0 = 0;
            entry.1 = now;
        }

        entry.0 += 1;
        entry.0 <= self.max_requests
    }
}

/// 限流中间件处理函数
///
/// 用法：
/// ```ignore
/// let limiter = RateLimiter::new(120, 60); // 每分钟 120 次
/// app.layer(axum::middleware::from_fn_with_state(limiter, rate_limit_middleware))
/// ```
pub async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    // 从连接信息或 X-Forwarded-For 获取 IP（与出站 IP 注入共用同一套规则）
    let ip = resolve_client_ip(&req).unwrap_or_else(|| "unknown".to_string());

    if !limiter.check(&ip).await {
        tracing::warn!("Rate limited: {}", ip);
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(json!({
                "code": 429,
                "msg": "Too many requests, please slow down",
            })),
        )
            .into_response();
    }

    next.run(req).await
}

// ============================================================
//  调用方 IP（对应 Express `app.set('trust proxy', true)` 下的 `req.ip`）
// ============================================================

/// 内部头：把解析出的调用方 IP 传给下游处理器
///
/// 用 `x-ncm-` 前缀避免与真实客户端头混淆；中间件每次都会覆盖或删除它，
/// 外部伪造无效。
pub(crate) const CLIENT_IP_HEADER: &str = "x-ncm-client-ip";

/// 解析调用方 IP
///
/// 复刻 Express `app.set('trust proxy', true)` 下 `req.ip` 的取值：
/// `X-Forwarded-For` **最左侧**（最远端调用方）优先，否则用 socket 对端地址；
/// 两者都会剥掉 IPv4-mapped 的 `::ffff:` 前缀。
///
/// 注意与上游一致：**不做**回环地址归一（`127.0.0.1` 不等于 `::1`，
/// 只有 `::1` 才会在 `server.js` 里被换成随机国内 IP）。
pub(crate) fn resolve_client_ip(req: &Request<axum::body::Body>) -> Option<String> {
    let forwarded = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(',').next().unwrap_or("").trim().to_string())
        .filter(|value| !value.is_empty());

    forwarded
        .or_else(|| {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|ci| ci.0.ip().to_string())
        })
        .map(|ip| match ip.strip_prefix("::ffff:") {
            Some(stripped) => stripped.to_string(),
            None => ip,
        })
}

/// 把调用方 IP 写入内部头，供 `extract_merged_query` 使用
///
/// 对应上游 `server.js` 里 `obj[2].ip` 的来源（`req.ip`）。
/// 拿不到 IP 时**删除**该头，避免外部伪造的 `x-ncm-client-ip` 被采信。
pub async fn client_ip_middleware(mut req: Request<axum::body::Body>, next: Next) -> Response {
    match resolve_client_ip(&req).and_then(|ip| HeaderValue::from_str(&ip).ok()) {
        Some(value) => {
            req.headers_mut().insert(CLIENT_IP_HEADER, value);
        }
        None => {
            req.headers_mut().remove(CLIENT_IP_HEADER);
        }
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

    fn request_with_xff(value: Option<&str>) -> Request<Body> {
        let mut builder = Request::builder();
        if let Some(value) = value {
            builder = builder.header("x-forwarded-for", value);
        }
        builder.body(Body::empty()).unwrap()
    }

    #[test]
    fn test_forwarded_takes_leftmost() {
        // 对应 app.set('trust proxy', true)：全部信任 → 取最左侧（最远端调用方）
        let req = request_with_xff(Some("1.2.3.4, 5.6.7.8, 9.9.9.9"));
        assert_eq!(resolve_client_ip(&req).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn test_forwarded_is_trimmed() {
        let req = request_with_xff(Some("  1.2.3.4  ,5.6.7.8"));
        assert_eq!(resolve_client_ip(&req).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn test_ipv4_mapped_prefix_stripped() {
        let req = request_with_xff(Some("::ffff:1.2.3.4"));
        assert_eq!(resolve_client_ip(&req).as_deref(), Some("1.2.3.4"));
    }

    #[test]
    fn test_empty_forwarded_falls_through() {
        // 空串是假值，不应被当成有效 IP
        let req = request_with_xff(Some(""));
        assert_eq!(resolve_client_ip(&req), None);
    }

    #[test]
    fn test_no_header_no_connect_info() {
        // 测试里没有 ConnectInfo（只有 start_server 才带），应为 None
        let req = request_with_xff(None);
        assert_eq!(resolve_client_ip(&req), None);
    }
}
