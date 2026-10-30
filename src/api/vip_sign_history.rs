use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 黑胶乐签打卡历史与状态
    /// 对应 /vip/sign/history
    pub async fn vip_sign_history(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/vipnewcenter/app/minidesk/music/sign/pc",
            json!({ "type": query.get_or("type", "0") }),
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
