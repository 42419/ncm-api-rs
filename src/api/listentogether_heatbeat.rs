use super::Query;
use crate::error::Result;
/// 一起听 发送心跳
/// 对应 Node.js module/listentogether_heatbeat.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 一起听 发送心跳
    /// 对应 /listentogether/heatbeat
    pub async fn listentogether_heatbeat(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "roomId": query.get_or("roomId", ""),
            "songId": query.get_or("songId", ""),
            "playStatus": query.get_or("playStatus", ""),
            "progress": query.get_or("progress", "")
        });
        query.drop_absent_params(
            &mut data,
            &[
                ("playStatus", "playStatus"),
                ("progress", "progress"),
                ("roomId", "roomId"),
                ("songId", "songId"),
            ],
        );
        self.request(
            "/api/listen/together/heartbeat",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
