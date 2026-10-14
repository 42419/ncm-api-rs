use super::Query;
use crate::error::Result;
/// 话题详情
/// 对应 Node.js module/topic_detail.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 话题详情
    /// 对应 /topic/detail
    pub async fn topic_detail(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "actid": query.get_or("actid", "")
        });
        query.drop_absent_params(&mut data, &[("actid", "actid")]);
        self.request("/api/act/detail", data, query.to_option(CryptoType::Weapi))
            .await
    }
}
