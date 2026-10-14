use super::Query;
use crate::error::Result;
/// 通用 API 代理接口
/// 对应 Node.js module/api.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::cookie::{cookie_obj_to_string, cookie_to_json};
use serde_json::{json, Value};

impl ApiClient {
    /// 通用 API 代理接口，允许调用任意 API 路径
    /// 对应 /api
    pub async fn api(&self, query: &Query) -> Result<ApiResponse> {
        let uri = query.get_or("uri", "");
        let data_str = query.get_or("data", "{}");
        let mut data: Value = serde_json::from_str(&data_str).unwrap_or_else(|_| json!({}));
        let crypto_str = query.get_or("crypto", "");
        let crypto = CryptoType::from(crypto_str.as_str());

        let mut option = query.to_option(crypto);

        // 上游：若 `data.cookie` 是字符串，则 `cookieToJson` 后写回 `data.cookie`，
        // 同时把 `query.cookie` 也设为该对象（请求层最终用其拼 Cookie 头）
        if let Some(cookie_str) = data.get("cookie").and_then(|v| v.as_str()) {
            let cookie_map = cookie_to_json(cookie_str);
            if let Ok(cookie_obj) = serde_json::to_value(&cookie_map) {
                data["cookie"] = cookie_obj;
            }
            option.cookie = Some(cookie_obj_to_string(&cookie_map));
        }

        self.request(&uri, data, option).await
    }
}
