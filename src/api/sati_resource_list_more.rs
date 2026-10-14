use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 助眠解压：获取同类推荐
    /// 对应 /sati/resource/list/more
    pub async fn sati_resource_list_more(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({ "id": query.get_or("id", "") });
        query.drop_absent_params(&mut data, &[("id", "id")]);
        self.request(
            "/api/voice/sati/resource/list/more/v1",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
