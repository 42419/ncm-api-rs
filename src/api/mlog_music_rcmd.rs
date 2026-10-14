use super::Query;
use crate::error::Result;
/// 歌曲相关视频
/// 对应 Node.js module/mlog_music_rcmd.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 歌曲相关视频
    /// 对应 /mlog/music/rcmd
    pub async fn mlog_music_rcmd(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `extInfo: JSON.stringify({ songId: query.songid })`：
        // songid 缺省时丢键，整个对象为空
        let mut ext_info = json!({});
        if let Some(songid) = query.get("songid") {
            ext_info["songId"] = json!(songid);
        }
        let data = json!({
            "id": query.get_or("mvid", "0").parse::<i64>().unwrap_or(0),
            "type": 2,
            "rcmdType": 20,
            "limit": query.get_or("limit", "10").parse::<i64>().unwrap_or(10),
            "extInfo": ext_info.to_string()
        });
        self.request(
            "/api/mlog/rcmd/feed/list",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
