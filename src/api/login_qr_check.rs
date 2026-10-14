use super::Query;
use crate::error::Result;
/// 二维码检测扫码状态
/// 对应 Node.js module/login_qr_check.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 二维码检测扫码状态
    /// 对应 /login/qr/check
    pub async fn login_qr_check(&self, query: &Query) -> Result<ApiResponse> {
        let mut data = json!({
            "key": query.get_or("key", ""),
            "type": 3
        });
        query.drop_absent_params(&mut data, &[("key", "key")]);

        let res = self
            .request(
                "/api/login/qrcode/client/login",
                data,
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游把 set-cookie 用 ';' 拼进响应体，同时保留 cookie 数组，
        // 并把 status 固定为 200。
        // （上游的 catch 分支引用了 try 块内的 `result`，实际会抛 ReferenceError，
        //   因此这里不复刻「出错也返回 200 空体」的行为，而是照常上抛。）
        let mut body = if res.body.is_object() {
            res.body.clone()
        } else {
            json!({})
        };
        if let Some(obj) = body.as_object_mut() {
            obj.insert("cookie".to_string(), json!(res.cookie.join(";")));
        }

        Ok(ApiResponse {
            status: 200,
            body,
            cookie: res.cookie,
        })
    }
}
