use super::Query;
use crate::error::Result;
/// 乐谱列表
/// 对应 Node.js module/sheet_list.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 乐谱列表
    /// 对应 /sheet/list
    pub async fn sheet_list(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "id": query.get_or("id", ""),
            "abTest": query.get_or("ab", "b")
        });
        query.drop_absent_params(&mut data, &[("id", "id")]);
        self.request(
            "/api/music/sheet/list/v1",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
