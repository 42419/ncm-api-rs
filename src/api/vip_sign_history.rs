use super::Query;
use crate::error::Result;
/// 黑胶乐签打卡历史
/// 对应 Node.js module/vip_sign_history.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 黑胶乐签打卡历史
    /// 对应 /vip/sign/history
    pub async fn vip_sign_history(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({ "type": "0" });
        self.request(
            "/api/vipnewcenter/app/minidesk/music/sign/pc",
            data,
            query.to_option(CryptoType::Xeapi),
        )
        .await
    }
}
