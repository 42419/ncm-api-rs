use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 黑胶乐签打卡详情
    /// 对应 /vip/sign/detail
    pub async fn vip_sign_detail(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({
            "signDayTime": query.get("timestamp"),
            "type": "1",
        });
        self.request(
            "/api/vipnewcenter/app/level/user/checkin/history/detail",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
