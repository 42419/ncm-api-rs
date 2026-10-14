use super::Query;
use crate::error::Result;
/// 云盘歌曲匹配
/// 对应 Node.js module/cloud_match.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 云盘歌曲匹配纠正
    /// 对应 /cloud/match
    pub async fn cloud_match(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "userId": query.get_or("uid", ""),
            "songId": query.get_or("sid", ""),
            "adjustSongId": query.get_or("asid", "")
        });
        query.drop_absent_params(
            &mut data,
            &[
                ("adjustSongId", "asid"),
                ("songId", "sid"),
                ("userId", "uid"),
            ],
        );
        self.request(
            "/api/cloud/user/song/match",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
