use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：收藏或取消收藏资源
    /// 对应 /sati/resource/sub
    pub async fn sati_resource_sub(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({
            "id": query.get_or("id", ""),
            "cancel": query.get_bool("cancel", false),
        });
        self.request(
            "/api/voice/sati/resource/sub",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
