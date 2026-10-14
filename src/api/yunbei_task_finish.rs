use super::Query;
use crate::error::Result;
/// 云贝完成任务
/// 对应 Node.js module/yunbei_task_finish.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 云贝完成任务
    /// 对应 /yunbei/task/finish
    pub async fn yunbei_task_finish(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "userTaskId": query.get_or("userTaskId", ""),
            "depositCode": query.get_or("depositCode", "0")
        });
        query.drop_absent_params(&mut data, &[("userTaskId", "userTaskId")]);
        self.request(
            "/api/usertool/task/point/receive",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
