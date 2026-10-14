use super::Query;
use crate::error::Result;
/// 会员任务 - 新版
/// 对应 Node.js module/vip_tasks_v1.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 会员任务列表（新版）
    /// 对应 /vip/tasks/v1
    pub async fn vip_tasks_v1(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "taskType": "app_vip_task_center",
        });
        // Node 侧 `userId: query.id` 在 id 缺失时会被 JSON.stringify 丢弃，
        // 因此这里同样按“存在才写入”处理。
        if let Some(id) = query.get("id") {
            data["userId"] = json!(id);
        }
        self.request(
            "/api/middle/vip/mission/user/progress/list",
            data,
            query.to_option(CryptoType::Xeapi),
        )
        .await
    }
}
