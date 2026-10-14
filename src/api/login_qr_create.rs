use super::Query;
use crate::error::Result;
/// 二维码生成
/// 对应 Node.js module/login_qr_create.js
///
/// 注意: Node.js 版本使用 qrcode 库生成二维码图片，Rust 版本仅返回 URL，
/// 不包含 qrimg 生成（需要调用方自行处理）
use crate::request::{ApiClient, ApiResponse};
use crate::util::cookie::cookie_to_json;
use serde_json::json;

/// 复刻上游 `util/index.js` 的 `generateChainId(cookie)`：
/// `v1_<deviceId>_web_login_<毫秒时间戳>`，其中 `deviceId` 取 cookie 里的
/// `sDeviceId`，缺失时用 `unknown-<0..1e6 的随机数>`。
fn generate_chain_id(cookie: &str) -> String {
    let random_num = rand::random::<u32>() % 1_000_000;
    let device_id = cookie_to_json(cookie)
        .get("sDeviceId")
        .cloned()
        .unwrap_or_else(|| format!("unknown-{}", random_num));
    format!(
        "v1_{}_web_login_{}",
        device_id,
        chrono::Utc::now().timestamp_millis()
    )
}

impl ApiClient {
    /// 二维码生成
    /// 对应 /login/qr/create
    pub async fn login_qr_create(&self, query: &Query) -> Result<ApiResponse> {
        let key = query.get("key").unwrap_or("");
        let mut url = format!("https://music.163.com/login?codekey={}", key);

        // 上游 `if (platform === 'web') url += '&chainId=' + generateChainId(cookie)`
        let platform = query.get_or("platform", "pc");
        if platform == "web" {
            let chain_id = generate_chain_id(query.cookie.as_deref().unwrap_or(""));
            url.push_str(&format!("&chainId={}", chain_id));
        }

        Ok(ApiResponse {
            status: 200,
            body: json!({
                "code": 200,
                "data": {
                    "qrurl": url,
                    // 上游 `query.qrimg ? await QRCode.toDataURL(url) : ''`——
                    // 生成二维码图片需要额外依赖，Rust 侧恒返回空串
                    "qrimg": ""
                }
            }),
            cookie: vec![],
        })
    }
}
