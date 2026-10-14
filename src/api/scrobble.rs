use super::Query;
use crate::error::Result;
/// 听歌打卡
/// 对应 Node.js module/scrobble.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 听歌打卡
    /// 对应 /scrobble
    pub async fn scrobble(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `id: query.id` / `sourceId: query.sourceid` / `time: query.time`
        // 均为裸取值，缺省时丢键（time 原样传字符串）
        let mut log_json = json!({
            "download": 0,
            "end": "playend",
            "id": query.get_or("id", "0"),
            "sourceId": query.get_or("sourceid", ""),
            "time": query.get_or("time", "0"),
            "type": "song",
            "wifi": 0,
            "source": "list",
            "mainsite": 1,
            "content": ""
        });
        query.drop_absent_params(
            &mut log_json,
            &[("id", "id"), ("sourceId", "sourceid"), ("time", "time")],
        );
        let data = json!({
            "logs": serde_json::to_string(&json!([{
                "action": "play",
                "json": log_json
            }])).unwrap_or_default()
        });
        self.request(
            "/api/feedback/weblog",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
