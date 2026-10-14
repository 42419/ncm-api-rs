use super::Query;
use crate::error::Result;
/// 评论点赞
/// 对应 Node.js module/comment_like.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::config::RESOURCE_TYPE_MAP;
use serde_json::{json, Map, Value};

/// 复刻上游 `query.t == 1 ? 'like' : 'unlike'`。
///
/// JS 的 `==` 是宽松比较：字符串 `'1'`、`' 1 '`、`'1.0'` 都与数字 1 相等。
fn comment_action(t: Option<&str>) -> &'static str {
    match t {
        Some(value) if value.trim().parse::<f64>() == Ok(1.0) => "like",
        _ => "unlike",
    }
}

/// 构造请求体，对应上游 module/comment_like.js 的 data 组装。
fn comment_like_data(query: &Query) -> Value {
    // 上游 `query.type = resourceTypeMap[query.type]`：未收录时是 undefined，
    // 而 `undefined + query.id` 在 JS 里会拼成字面量 "undefined<id>"
    let mapped = query
        .get("type")
        .and_then(|t| RESOURCE_TYPE_MAP.get(t).copied());

    let mut data = Map::new();
    if mapped == Some("A_EV_2_") {
        // 上游 `if (query.type == 'A_EV_2_') data.threadId = query.threadId`
        // —— 裸取值，缺省时该键会被 JSON.stringify 丢掉
        if let Some(thread_id) = query.get("threadId") {
            data.insert("threadId".to_string(), json!(thread_id));
        }
    } else {
        data.insert(
            "threadId".to_string(),
            json!(format!(
                "{}{}",
                mapped.unwrap_or("undefined"),
                query.get("id").unwrap_or("undefined")
            )),
        );
    }

    // 上游 `commentId: query.cid`——裸取值，缺省丢键
    if let Some(cid) = query.get("cid") {
        data.insert("commentId".to_string(), json!(cid));
    }

    Value::Object(data)
}

impl ApiClient {
    /// 评论点赞/取消点赞
    /// 对应 /comment/like
    pub async fn comment_like(&self, query: &Query) -> Result<ApiResponse> {
        let path = comment_action(query.get("t"));
        self.request(
            &format!("/api/v1/comment/{}", path),
            comment_like_data(query),
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_follows_js_loose_equality() {
        assert_eq!(comment_action(Some("1")), "like");
        assert_eq!(comment_action(Some(" 1 ")), "like");
        assert_eq!(comment_action(Some("1.0")), "like");
        assert_eq!(comment_action(Some("0")), "unlike");
        assert_eq!(comment_action(Some("true")), "unlike");
        assert_eq!(comment_action(None), "unlike");
    }

    #[test]
    fn test_dynamic_comment_uses_explicit_thread_id() {
        let query = Query::new()
            .param("type", "6")
            .param("id", "999")
            .param("threadId", "A_EV_2_1234")
            .param("cid", "42");
        let data = comment_like_data(&query);
        assert_eq!(data["threadId"], "A_EV_2_1234");
        assert_eq!(data["commentId"], "42");
    }

    #[test]
    fn test_dynamic_comment_without_thread_id_omits_key() {
        let query = Query::new().param("type", "6").param("id", "999");
        let data = comment_like_data(&query);
        assert!(data.get("threadId").is_none());
    }

    #[test]
    fn test_regular_comment_uses_mapped_prefix() {
        let query = Query::new().param("type", "0").param("id", "123");
        assert_eq!(comment_like_data(&query)["threadId"], "R_SO_4_123");
    }

    #[test]
    fn test_unknown_type_follows_js_undefined_concat() {
        let query = Query::new().param("type", "999").param("id", "123");
        assert_eq!(comment_like_data(&query)["threadId"], "undefined123");
    }

    #[test]
    fn test_missing_cid_omits_key() {
        let query = Query::new().param("type", "0").param("id", "123");
        assert!(comment_like_data(&query).get("commentId").is_none());
    }
}
