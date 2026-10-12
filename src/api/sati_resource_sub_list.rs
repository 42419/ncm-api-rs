use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：获取已收藏资源列表
    /// 对应 /sati/resource/sub/list
    pub async fn sati_resource_sub_list(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/voice/sati/resource/sub/list",
            json!({}),
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
