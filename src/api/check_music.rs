use super::{js_parse_int, Query};
use crate::error::Result;
/// 检查音乐是否可用
/// 对应 Node.js module/check_music.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{json, Value};

/// 复刻上游 `response.body.code == 200`（JS 宽松比较，数字与数字字符串都算）。
fn is_code_200(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Number(n)) => n.as_f64() == Some(200.0),
        Some(Value::String(s)) => s.trim().parse::<f64>() == Ok(200.0),
        _ => false,
    }
}

impl ApiClient {
    /// 检查音乐是否可用
    /// 对应 /check/music
    pub async fn check_music(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `ids: '[' + parseInt(query.id) + ']'`——`query.id` 缺失或非数字时
        // 拼出的是字面量 `[NaN]`
        let ids_value = js_parse_int(query.get("id").unwrap_or(""));
        let ids = format!(
            "[{}]",
            ids_value
                .map(|v| v.to_string())
                .unwrap_or_else(|| "NaN".to_string())
        );

        // 上游 `br: parseInt(query.br || 999000)`——空串是 falsy 会回退到 999000；
        // parseInt 得到 NaN 时，`JSON.stringify` 会把它写成 `null`
        let br_source = query
            .get("br")
            .filter(|s| !s.is_empty())
            .unwrap_or("999000");
        let br = match js_parse_int(br_source) {
            Some(v) => json!(v),
            None => Value::Null,
        };

        let data = json!({ "ids": ids, "br": br });
        let res = self
            .request(
                "/api/song/enhance/player/url",
                data,
                query.to_option(CryptoType::Weapi),
            )
            .await?;

        // 上游 `.then()` 后处理：`body.code == 200 && body.data[0].code == 200`
        // 视为可播放，否则统一返回 code 200 并带 `success` / `message`
        // （上游对 `data[0]` 缺失没有兜底，会抛 TypeError；这里按「不可播放」处理）
        let playable = is_code_200(res.body.get("code"))
            && is_code_200(
                res.body
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|list| list.first())
                    .and_then(|song| song.get("code")),
            );

        let body = if playable {
            json!({ "code": 200, "success": true, "message": "ok" })
        } else {
            json!({ "code": 200, "success": false, "message": "亲爱的,暂无版权" })
        };

        Ok(ApiResponse {
            status: res.status,
            body,
            cookie: res.cookie,
        })
    }
}
