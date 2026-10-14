use super::Query;
use crate::error::Result;
/// 获取专辑歌曲的音质
/// 对应 Node.js module/album_privilege.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 获取专辑歌曲的音质
    /// 对应 /album/privilege
    pub async fn album_privilege(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "id": query.get_or("id", "")
        });
        query.drop_absent_params(&mut data, &[("id", "id")]);
        self.request(
            "/api/album/privilege",
            data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
