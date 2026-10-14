use super::Query;
use crate::error::Result;
/// 更新歌单标签
/// 对应 Node.js module/playlist_tags_update.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 更新歌单标签
    /// 对应 /playlist/tags/update
    pub async fn playlist_tags_update(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "id": query.get_or("id", ""),
            "tags": query.get_or("tags", "")
        });
        query.drop_absent_params(&mut data, &[("id", "id"), ("tags", "tags")]);
        self.request(
            "/api/playlist/tags/update",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
