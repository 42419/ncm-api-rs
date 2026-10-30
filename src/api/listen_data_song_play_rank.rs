use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 听歌足迹歌曲播放排行
    /// 对应 /listen/data/song/play/rank
    pub async fn listen_data_song_play_rank(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/content/activity/listen/data/song/play/rank",
            json!({
                "type": query.get_or("type", "month"),
                "endTime": query.get("endTime"),
            }),
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
