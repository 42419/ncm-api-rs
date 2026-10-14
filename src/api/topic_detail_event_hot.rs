use super::Query;
use crate::error::Result;
/// 话题详情热门动态
/// 对应 Node.js module/topic_detail_event_hot.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 话题详情热门动态
    /// 对应 /topic/detail/event/hot
    pub async fn topic_detail_event_hot(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "actid": query.get_or("actid", "")
        });
        query.drop_absent_params(&mut data, &[("actid", "actid")]);
        self.request(
            "/api/act/event/hot",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
