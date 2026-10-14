use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 获取指定维度音乐排行榜详情
    /// 对应 /chart/detail
    pub async fn chart_detail(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "chartCode": query.get_or("chartCode", ""),
            "targetId": query.get_or("targetId", ""),
            "targetType": query.get_or("targetType", ""),
        });
        query.drop_absent_params(
            &mut data,
            &[
                ("chartCode", "chartCode"),
                ("targetId", "targetId"),
                ("targetType", "targetType"),
            ],
        );
        self.request("/api/chart/detail", data, query.to_option(CryptoType::Eapi))
            .await
    }
}
