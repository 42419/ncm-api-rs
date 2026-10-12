use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 灰色歌曲的其他版本推荐
    /// 对应 /song/copyright/rcmd
    pub async fn song_copyright_rcmd(&self, query: &Query) -> Result<ApiResponse> {
        let song_id = query
            .get("songid")
            .unwrap_or_else(|| query.get("id").unwrap_or(""));
        let data = json!({ "songid": song_id });
        self.request(
            "/api/song/copyright/rcmd",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
