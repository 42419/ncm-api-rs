use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：获取标签下的资源列表
    /// 对应 /sati/resource/list
    pub async fn sati_resource_list(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({
            "tag": query.get_or("tag", ""),
            "firstQuery": false,
        });
        self.request(
            "/api/voice/sati/resource/list",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
