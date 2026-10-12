use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 获取指定维度音乐排行榜列表
    /// 对应 /chart/song/detail
    pub async fn chart_song_detail(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({
            "chartCode": query.get_or("chartCode", ""),
            "targetId": query.get_or("targetId", ""),
            "targetType": query.get_or("targetType", ""),
        });
        self.request(
            "/api/chart/song/detail",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
