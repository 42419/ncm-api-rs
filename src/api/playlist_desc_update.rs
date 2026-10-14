use super::Query;
use crate::error::Result;
/// 更新歌单描述
/// 对应 Node.js module/playlist_desc_update.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 更新歌单描述
    /// 对应 /playlist/desc/update
    pub async fn playlist_desc_update(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "id": query.get_or("id", ""),
            "desc": query.get_or("desc", "")
        });
        query.drop_absent_params(&mut data, &[("desc", "desc"), ("id", "id")]);
        self.request(
            "/api/playlist/desc/update",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
