use super::Query;
use crate::error::Result;
/// 评论统计数据
/// 对应 Node.js module/comment_info_list.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 评论统计数据
    /// 对应 /comment/info/list
    pub async fn comment_info_list(&self, query: &Query) -> Result<ApiResponse> {
        let resource_type = query.get_or("type", "0");
        let type_id = crate::util::config::RESOURCE_TYPE_MAP
            .get(resource_type.as_str())
            .map(|prefix| {
                prefix
                    .trim_end_matches('_')
                    .rsplit('_')
                    .next()
                    .unwrap_or("0")
                    .to_string()
            })
            .unwrap_or_else(|| "0".to_string());

        // 上游 `String(query.ids || query.id || '')`：ids 为空时回退到 id
        let ids_raw = query
            .get("ids")
            .filter(|s| !s.is_empty())
            .or_else(|| query.get("id").filter(|s| !s.is_empty()))
            .unwrap_or("")
            .to_string();
        let ids: Vec<String> = ids_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let data = json!({
            "resourceType": type_id,
            "resourceIds": serde_json::to_string(&ids).unwrap()
        });
        self.request(
            "/api/resource/commentInfo/list",
            data,
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
