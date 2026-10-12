use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：获取特定时间场景下的推荐资源
    /// 对应 /sati/timescene/resources/get
    pub async fn sati_timescene_resources_get(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({ "firstQuery": false });
        self.request(
            "/api/voice/sati/timescene/resources/get",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
