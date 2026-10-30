use super::Query;
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 从云盘获取歌曲下载链接
    /// 对应 /song/cloud/download
    pub async fn song_cloud_download(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/cloud/dowonload",
            json!({ "songId": query.get("id") }),
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
