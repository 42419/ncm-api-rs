use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 获取新版会员任务进度
    /// 对应 /vip/tasks/v1
    pub async fn vip_tasks_v1(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/middle/vip/mission/user/progress/list",
            json!({
                "taskType": "app_vip_task_center",
                "userId": query.get("id"),
            }),
            query.to_option(CryptoType::Xeapi),
        )
        .await
    }
}
