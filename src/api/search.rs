use super::Query;
use crate::error::Result;
/// 搜索
/// 对应 Node.js module/search.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 搜索
    /// 对应 /search
    pub async fn search(&self, query: &Query) -> Result<ApiResponse> {
        let search_type = query.get_or("type", "1").parse::<i64>().unwrap_or(1);

        if search_type == 2000 {
            let mut data = json!({
                "keyword": query.get_or("keywords", ""),
                "scene": "normal",
                "limit": query.get_or("limit", "30").parse::<i64>().unwrap_or(30),
                "offset": query.get_or("offset", "0").parse::<i64>().unwrap_or(0)
            });
            query.drop_absent_params(&mut data, &[("keyword", "keywords")]);
            self.request(
                "/api/search/voice/get",
                data,
                query.to_option(CryptoType::default()),
            )
            .await
        } else {
            let mut data = json!({
                "s": query.get_or("keywords", ""),
                "type": search_type,
                "limit": query.get_or("limit", "30").parse::<i64>().unwrap_or(30),
                "offset": query.get_or("offset", "0").parse::<i64>().unwrap_or(0)
            });
            query.drop_absent_params(&mut data, &[("s", "keywords")]);
            self.request(
                "/api/search/get",
                data,
                query.to_option(CryptoType::default()),
            )
            .await
        }
    }
}
