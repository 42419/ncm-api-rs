use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 举报歌曲评论
    /// 对应 /comment/report
    pub async fn comment_report(&self, query: &Query) -> Result<ApiResponse> {
        let data = json!({
            "threadId": format!("R_SO_4_{}", query.get_or("id", "")),
            "commentId": query.get_or("cid", ""),
            "reason": query.get_or("reason", ""),
        });
        self.request(
            "/api/report/reportcomment",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
