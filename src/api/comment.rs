use super::Query;
use crate::error::Result;
/// 发送/删除/回复评论
/// 对应 Node.js module/comment.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::config::RESOURCE_TYPE_MAP;
use serde_json::{json, Map, Value};

/// 动作名，对应上游 `{1:'add', 0:'delete', 2:'reply'}[query.t]`
///
/// `t` 缺失或不在 `{0,1,2}` 时 JS 侧映射结果是 `undefined`，会直接拼进
/// 请求路径（`/api/resource/comments/undefined`），这里如实保留该行为。
fn comment_action(query: &Query) -> &'static str {
    match query.get("t") {
        Some("1") => "add",
        Some("0") => "delete",
        Some("2") => "reply",
        _ => "undefined",
    }
}

/// 构造评论请求体，对应上游 module/comment.js 的 data 组装
fn comment_data(query: &Query) -> Value {
    let mapped = query
        .get("type")
        .and_then(|t| RESOURCE_TYPE_MAP.get(t).copied());

    let mut data = Map::new();
    if mapped == Some("A_EV_2_") {
        // 上游：动态评论直接用 query.threadId 覆盖 threadId；
        // 未传 threadId 时该键会被 JSON.stringify 丢弃
        if let Some(thread_id) = query.get("threadId") {
            data.insert("threadId".to_string(), json!(thread_id));
        }
    } else {
        // 上游 `query.type + query.id`：JS 里 undefined 参与字符串相加会变成
        // 字面量 "undefined"，所以未收录的 type、缺失的 id 都会被拼进去
        let thread_id = format!(
            "{}{}",
            mapped.unwrap_or("undefined"),
            query.get("id").unwrap_or("undefined")
        );
        data.insert("threadId".to_string(), json!(thread_id));
    }

    match comment_action(query) {
        // 上游是直接赋值，undefined 会被 JSON.stringify 丢弃这个键
        "add" => {
            if let Some(content) = query.get("content") {
                data.insert("content".to_string(), json!(content));
            }
        }
        "delete" => {
            if let Some(comment_id) = query.get("commentId") {
                data.insert("commentId".to_string(), json!(comment_id));
            }
        }
        "reply" => {
            if let Some(comment_id) = query.get("commentId") {
                data.insert("commentId".to_string(), json!(comment_id));
            }
            if let Some(content) = query.get("content") {
                data.insert("content".to_string(), json!(content));
            }
        }
        _ => {}
    }

    Value::Object(data)
}

impl ApiClient {
    /// 发送/删除/回复评论
    /// 对应 /comment
    /// t: 1=发送, 0=删除, 2=回复
    pub async fn comment(&self, query: &Query) -> Result<ApiResponse> {
        let action = comment_action(query);
        self.request(
            &format!("/api/resource/comments/{}", action),
            comment_data(query),
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_comment_prefers_explicit_thread_id() {
        // type=6 映射为 A_EV_2_，此时 threadId 应直接取 query.threadId
        let query = Query::new()
            .param("type", "6")
            .param("id", "999")
            .param("threadId", "A_EV_2_1234")
            .param("t", "1")
            .param("content", "hi");
        let data = comment_data(&query);
        assert_eq!(data["threadId"], "A_EV_2_1234");
        assert_eq!(data["content"], "hi");
    }

    #[test]
    fn test_dynamic_comment_without_thread_id_omits_key() {
        // 上游 data.threadId = undefined，JSON.stringify 会丢弃该键
        let query = Query::new().param("type", "6").param("id", "999");
        let data = comment_data(&query);
        assert!(data.get("threadId").is_none());
    }

    #[test]
    fn test_regular_comment_uses_mapped_prefix() {
        let query = Query::new().param("type", "0").param("id", "123");
        let data = comment_data(&query);
        assert_eq!(data["threadId"], "R_SO_4_123");
    }

    #[test]
    fn test_unknown_type_follows_js_undefined_concat() {
        // 上游 `resourceTypeMap["999"] + "123"` = "undefined123"
        let query = Query::new().param("type", "999").param("id", "123");
        assert_eq!(comment_data(&query)["threadId"], "undefined123");
    }

    #[test]
    fn test_missing_type_and_id_follow_js_undefined_concat() {
        let query = Query::new();
        assert_eq!(comment_data(&query)["threadId"], "undefinedundefined");
    }

    #[test]
    fn test_missing_t_maps_to_undefined_action() {
        // 上游 `{1:'add',0:'delete',2:'reply'}[undefined]` = undefined，
        // 拼出的路径就是 /api/resource/comments/undefined
        assert_eq!(comment_action(&Query::new()), "undefined");

        let odd = Query::new().param("t", "7");
        assert_eq!(comment_action(&odd), "undefined");
        // 此时三个 if 分支都不命中，data 里只有 threadId
        assert_eq!(comment_data(&odd).as_object().unwrap().len(), 1);
    }

    #[test]
    fn test_absent_optional_fields_are_omitted() {
        // 上游对 content / commentId 是直接赋值，undefined 会被丢弃
        let add = Query::new()
            .param("type", "0")
            .param("id", "1")
            .param("t", "1");
        assert!(comment_data(&add).get("content").is_none());

        let reply = Query::new()
            .param("type", "0")
            .param("id", "1")
            .param("t", "2");
        assert!(comment_data(&reply).get("commentId").is_none());
        assert!(comment_data(&reply).get("content").is_none());
    }

    #[test]
    fn test_actions() {
        let add = Query::new()
            .param("type", "0")
            .param("id", "1")
            .param("t", "1")
            .param("content", "c");
        assert_eq!(comment_action(&add), "add");
        assert_eq!(comment_data(&add)["content"], "c");

        let del = Query::new()
            .param("type", "0")
            .param("id", "1")
            .param("t", "0")
            .param("commentId", "42");
        assert_eq!(comment_action(&del), "delete");
        assert_eq!(comment_data(&del)["commentId"], "42");
        assert!(comment_data(&del).get("content").is_none());

        let reply = Query::new()
            .param("type", "0")
            .param("id", "1")
            .param("t", "2")
            .param("commentId", "42")
            .param("content", "r");
        assert_eq!(comment_action(&reply), "reply");
        assert_eq!(comment_data(&reply)["commentId"], "42");
        assert_eq!(comment_data(&reply)["content"], "r");
    }
}
