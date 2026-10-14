use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：收藏或取消收藏资源
    /// 对应 /sati/resource/sub
    pub async fn sati_resource_sub(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "id": query.get_or("id", ""),
            "cancel": query.get_bool("cancel", false),
        });
        query.drop_absent_params(&mut data, &[("id", "id")]);
        self.request(
            "/api/voice/sati/resource/sub",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
