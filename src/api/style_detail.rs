use super::Query;
use crate::error::Result;
/// 曲风详情
/// 对应 Node.js module/style_detail.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 曲风详情
    /// 对应 /style/detail
    pub async fn style_detail(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "tagId": query.get_or("tagId", "")
        });
        query.drop_absent_params(&mut data, &[("tagId", "tagId")]);
        self.request(
            "/api/style-tag/home/head",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
