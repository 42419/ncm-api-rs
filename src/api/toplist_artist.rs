use super::Query;
use crate::error::Result;
/// 排行榜歌手
/// 对应 Node.js module/toplist_artist.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 排行榜歌手
    /// 对应 /toplist/artist
    pub async fn toplist_artist(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `limit: 100` / `offset: 0` 是写死的常量，不读 query
        let data = json!({
            "type": query.get_or("type", "1").parse::<i64>().unwrap_or(1),
            "limit": 100,
            "offset": 0,
            "total": true
        });
        self.request(
            "/api/toplist/artist",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
