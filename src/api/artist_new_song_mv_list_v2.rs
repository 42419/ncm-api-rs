use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 获取关注歌手的新歌曲和 MV
    /// 对应 /artist/new/song/mv/list/v2
    pub async fn artist_new_song_mv_list_v2(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/sub/artist/new/works/song-mv/list/v2",
            json!({
                "startTimestamp": query
                    .get("startTimestamp")
                    .or_else(|| query.get("before"))
                    .map(str::to_string)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis().to_string()),
                "sourceType": query.get_i64("sourceType", 1),
                "limit": query.get_i64("limit", 10),
                "firstRequest": query.get_bool("firstRequest", true),
            }),
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
