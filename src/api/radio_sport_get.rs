use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 跑步漫游
    /// 对应 /radio/sport/get
    pub async fn radio_sport_get(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({ "bpm": query.get_i64("bpm", 50) });
        self.request(
            "/api/radio/sport/get",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
