use super::Query;
use crate::error::Result;
/// 首页 Block Page
/// 对应 Node.js module/homepage_block_page.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{Map, Value};

impl ApiClient {
    /// 首页内容
    /// 对应 /homepage/block/page
    pub async fn homepage_block_page(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `{ refresh: query.refresh || false, cursor: query.cursor }`：
        // refresh 缺省/空串时为布尔 false，否则原样传字符串；
        // cursor 缺省时该键会被 JSON.stringify 丢弃
        let mut data = Map::new();
        match query.get("refresh").filter(|s| !s.is_empty()) {
            Some(v) => {
                data.insert("refresh".to_string(), Value::String(v.to_string()));
            }
            None => {
                data.insert("refresh".to_string(), Value::Bool(false));
            }
        }
        if let Some(cursor) = query.get("cursor") {
            data.insert("cursor".to_string(), Value::String(cursor.to_string()));
        }
        self.request(
            "/api/homepage/block/page",
            Value::Object(data),
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
