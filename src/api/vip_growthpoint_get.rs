use super::Query;
use crate::error::Result;
/// 领取会员成长值
/// 对应 Node.js module/vip_growthpoint_get.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 领取会员成长值
    /// 对应 /vip/growthpoint/get
    pub async fn vip_growthpoint_get(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "taskIds": query.get_or("ids", "")
        });
        query.drop_absent_params(&mut data, &[("taskIds", "ids")]);
        self.request(
            "/api/vipnewcenter/app/level/task/reward/get",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
