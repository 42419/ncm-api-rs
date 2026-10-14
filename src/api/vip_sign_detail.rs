use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 黑胶乐签打卡详情
    /// 对应 /vip/sign/detail
    pub async fn vip_sign_detail(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({ "type": "1" });
        if let Some(timestamp) = query.get("timestamp") {
            data["signDayTime"] = json!(timestamp);
        }
        self.request(
            "/api/vipnewcenter/app/level/user/checkin/history/detail",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
