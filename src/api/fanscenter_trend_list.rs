use super::Query;
use crate::error::Result;
/// 粉丝来源
/// 对应 Node.js module/fanscenter_trend_list.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{json, Value};

impl ApiClient {
    /// 粉丝来源
    /// 对应 /fanscenter/trend/list
    pub async fn fanscenter_trend_list(&self, query: &Query) -> Result<ApiResponse> {
        // 上游默认值：startTime = Date.now() - 7*24*3600*1000，endTime = Date.now()
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let start_time = match query.get("startTime").filter(|s| !s.is_empty()) {
            Some(v) => Value::String(v.to_string()),
            None => json!(now_ms - 7 * 24 * 3600 * 1000),
        };
        let end_time = match query.get("endTime").filter(|s| !s.is_empty()) {
            Some(v) => Value::String(v.to_string()),
            None => json!(now_ms),
        };
        let data = json!({
            "startTime": start_time,
            "endTime": end_time,
            "type": query.get_or("type", "0")
        });
        self.request(
            "/api/fanscenter/trend/list",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
