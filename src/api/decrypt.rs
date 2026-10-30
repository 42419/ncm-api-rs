use super::Query;
use crate::crypto::{eapi_req_decrypt, eapi_res_decrypt, xeapi_res_decrypt};
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

impl ApiClient {
    /// 按加密类型解密请求或响应数据。
    /// 对应 /decrypt
    pub fn decrypt(&self, query: &Query) -> Result<ApiResponse> {
        let crypto = query.get_or("crypto", "eapi");
        let data = query
            .get("data")
            .or_else(|| query.get("hexString"))
            .unwrap_or("");
        if data.is_empty() {
            return Ok(ApiResponse {
                status: 400,
                body: json!({"code": 400, "message": "data is required"}),
                cookie: vec![],
            });
        }

        let is_request = query.get_or("isReq", "true") != "false";
        let result = match crypto.as_str() {
            "eapi" => {
                let hex = data
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>();
                if is_request {
                    eapi_req_decrypt(&hex).map(|(url, body)| json!({"url": url, "data": body}))
                } else {
                    eapi_res_decrypt(&hex)
                }
            }
            "xeapi" if !is_request => STANDARD
                .decode(data)
                .ok()
                .and_then(|bytes| xeapi_res_decrypt(&bytes).ok()),
            "xeapi" => {
                return Ok(ApiResponse {
                    status: 400,
                    body: json!({"code": 400, "message": "xeapi request decryption is not supported"}),
                    cookie: vec![],
                });
            }
            "weapi" if !is_request => {
                let hex = data
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect::<String>();
                eapi_res_decrypt(&hex)
            }
            "weapi" => {
                return Ok(ApiResponse {
                    status: 400,
                    body: json!({"code": 400, "message": "weapi request decryption is not supported"}),
                    cookie: vec![],
                });
            }
            "api" => serde_json::from_str::<Value>(data).ok(),
            "linuxapi" => None,
            _ => {
                return Ok(ApiResponse {
                    status: 400,
                    body: json!({"code": 400, "message": format!("unknown crypto type: {crypto}")}),
                    cookie: vec![],
                });
            }
        };

        Ok(ApiResponse {
            status: if result.is_some() { 200 } else { 400 },
            body: result
                .map(|value| json!({"code": 200, "data": value}))
                .unwrap_or_else(|| json!({"code": 400, "message": "decryption failed"})),
            cookie: vec![],
        })
    }
}
