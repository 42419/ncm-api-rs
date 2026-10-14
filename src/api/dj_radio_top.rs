use super::Query;
use crate::error::Result;
/// 电台排行榜获取
/// 对应 Node.js module/djRadio_top.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 电台排行榜获取
    /// 对应 /djRadio/top
    pub async fn dj_radio_top(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `djRadioId: query.djRadioId || null`：缺省或空串时发送 null
        let data = json!({
            "djRadioId": query.get("djRadioId").filter(|s| !s.is_empty()),
            "sortIndex": query.get_or("sortIndex", "1").parse::<i64>().unwrap_or(1),
            "dataGapDays": query.get_or("dataGapDays", "7").parse::<i64>().unwrap_or(7),
            "dataType": query.get_or("dataType", "3").parse::<i64>().unwrap_or(3)
        });
        self.request(
            "/api/expert/worksdata/works/top/get",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
