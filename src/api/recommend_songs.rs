use super::Query;
use crate::error::Result;
/// 每日推荐歌曲
/// 对应 Node.js module/recommend_songs.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{Map, Value};

impl ApiClient {
    /// 每日推荐歌曲（需要登录）
    /// 对应 /recommend/songs
    pub async fn recommend_songs(&self, query: &Query) -> Result<ApiResponse> {
        self.request(
            "/api/v3/discovery/recommend/songs",
            recommend_songs_data(query),
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}

/// 构造 `/recommend/songs` 的请求体
///
/// 上游为 `const data = { afresh: query.afresh }`（v4.32.1 起，`3f679c0`）。
/// 未传 `afresh` 时 `query.afresh` 是 `undefined`，`JSON.stringify` 会丢掉
/// 该键，因此这里只在传入时才写入——不能固定写 `null`，也不能因为值是空串
/// 就当成没传（JS 里 `""` 不是 `undefined`，`{"afresh":""}` 是会被发出去的）。
fn recommend_songs_data(query: &Query) -> Value {
    let mut data = Map::new();
    if let Some(afresh) = query.get("afresh") {
        data.insert("afresh".to_string(), Value::String(afresh.to_string()));
    }
    Value::Object(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 期望值对应上游 `JSON.stringify({ afresh: query.afresh })`
    #[test]
    fn test_afresh_absent_key_is_dropped() {
        // undefined 在 JSON.stringify 里会被丢掉
        assert_eq!(recommend_songs_data(&Query::new()), json!({}));
    }

    #[test]
    fn test_afresh_present_is_kept() {
        assert_eq!(
            recommend_songs_data(&Query::new().param("afresh", "1")),
            json!({ "afresh": "1" })
        );
    }

    #[test]
    fn test_afresh_empty_string_is_kept() {
        // 空串不是 undefined（也不是 `||` 的回退条件），键要保留
        assert_eq!(
            recommend_songs_data(&Query::new().param("afresh", "")),
            json!({ "afresh": "" })
        );
    }
}
