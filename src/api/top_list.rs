use super::Query;
use crate::error::Result;
/// 排行榜
/// 对应 Node.js module/top_list.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 排行榜
    /// 对应 /top/list
    pub async fn top_list(&self, query: &Query) -> Result<ApiResponse> {
        // 上游只支持 id 调用：`if (query.idx)` 为真时直接返回 500。
        // 注意 JS 里空串是 falsy，而 `'0'` 是 truthy。
        if query.get("idx").map(|idx| !idx.is_empty()).unwrap_or(false) {
            return Ok(super::reject(
                500,
                json!({ "code": 500, "msg": "不支持此方式调用,只支持id调用" }),
            ));
        }

        let mut data = json!({
            "id": query.get_or("id", ""),
            "n": "500",
            "s": "0"
        });
        query.drop_absent_params(&mut data, &[("id", "id")]);
        self.request(
            "/api/playlist/v4/detail",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
