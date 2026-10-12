use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：获取标签列表
    /// 对应 /sati/tag/list
    pub async fn sati_tag_list(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/voice/sati/tag/list",
            json!({}),
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
