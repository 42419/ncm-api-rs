use super::Query;
use crate::crypto::{
    aes_ecb_decrypt_hex_raw, b64_decode, eapi_req_decrypt, eapi_res_decrypt, strip_js_whitespace,
    xeapi_res_decrypt, LINUXAPI_KEY,
};
use crate::error::Result;
/// 通用解密
/// 对应 Node.js module/decrypt.js
use crate::request::{ApiClient, ApiResponse};
use serde_json::{json, Value};

impl ApiClient {
    /// 通用解密
    /// 对应 /decrypt
    ///
    /// 参数：
    /// - `crypto`：加密方式，可选 `eapi` / `weapi` / `linuxapi` / `xeapi` / `api`，默认 `eapi`
    /// - `data`（或 `hexString`）：待解密数据
    /// - `isReq`：是否为请求方向解密（默认 `true`），设为 `"false"` 则为响应方向
    pub async fn decrypt(&self, query: &Query) -> Result<ApiResponse> {
        let crypto = query.get_or("crypto", "eapi");
        // 对应 Node 侧 `query.data || query.hexString || ''`：
        // 空字符串在 JS 的 `||` 下同样会继续回退，因此这里一并过滤掉空值。
        let data = query
            .get("data")
            .filter(|s| !s.is_empty())
            .or_else(|| query.get("hexString").filter(|s| !s.is_empty()))
            .unwrap_or("");
        let is_req = query.get_or("isReq", "true") != "false";

        if data.is_empty() {
            return Ok(bad_request("data is required"));
        }

        // 以下分支在上游是 `return`，不经过 try/catch，因此文案与
        // “解密失败: …”不同，需要单独处理。
        match crypto.as_str() {
            "weapi" if is_req => {
                return Ok(bad_request(
                    "weapi 请求解密需要 RSA 私钥，暂不支持；仅支持 weapi 返回数据解密（e_r=true 时与 eapi 相同）",
                ));
            }
            "xeapi" if is_req => {
                return Ok(bad_request(
                    "xeapi 请求解密涉及 X25519 ECDH 密钥交换，流程复杂，暂不支持；仅支持 xeapi 返回数据解密",
                ));
            }
            "eapi" | "weapi" | "linuxapi" | "xeapi" | "api" => {}
            other => {
                return Ok(bad_request(format!("未知加密方式: {}", other)));
            }
        }

        match decrypt_payload(&crypto, data, is_req) {
            Ok(result) => Ok(ApiResponse {
                status: 200,
                body: json!({ "code": 200, "data": result }),
                cookie: vec![],
            }),
            Err(e) => Ok(bad_request(format!("解密失败: {}", e))),
        }
    }
}

/// 按上游 switch 分支执行实际解密；错误由调用方统一包装成 `解密失败: …`
fn decrypt_payload(crypto: &str, data: &str, is_req: bool) -> std::result::Result<Value, String> {
    match crypto {
        "eapi" => {
            let pure_hex = strip_js_whitespace(data);
            if is_req {
                match eapi_req_decrypt(&pure_hex) {
                    Some((url, body)) => Ok(json!({ "url": url, "data": body })),
                    None => Ok(json!(null)),
                }
            } else {
                Ok(eapi_res_decrypt(&pure_hex).unwrap_or(json!(null)))
            }
        }
        // weapi 的响应解密：e_r=true 时服务端同样使用 eapi 加密
        "weapi" => {
            let pure_hex = strip_js_whitespace(data);
            Ok(eapi_res_decrypt(&pure_hex).unwrap_or(json!(null)))
        }
        "linuxapi" => {
            if is_req {
                let pure_hex = strip_js_whitespace(data);
                let decrypted = aes_ecb_decrypt_hex_raw(&pure_hex, LINUXAPI_KEY)?;
                let text = String::from_utf8(decrypted)
                    .map_err(|_| "解密结果不是合法的 UTF-8".to_string())?;
                serde_json::from_str(&text).map_err(|e| e.to_string())
            } else {
                serde_json::from_str(data).map_err(|e| e.to_string())
            }
        }
        "xeapi" => {
            let raw = b64_decode(&strip_js_whitespace(data))?;
            Ok(xeapi_res_decrypt(&raw).unwrap_or(json!(null)))
        }
        "api" => serde_json::from_str(data).map_err(|e| e.to_string()),
        // 已在前置 match 中排除，这里仅为穷尽性
        other => Err(format!("未知加密方式: {}", other)),
    }
}

fn bad_request(message: impl Into<String>) -> ApiResponse {
    ApiResponse {
        status: 400,
        body: json!({ "code": 400, "message": message.into() }),
        cookie: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 以下密文由上游 api-enhanced v4.34.3 的 util/crypto.js 行为生成
    //（AES-128-ECB + Pkcs7），并已用 openssl `enc -d -aes-128-ecb` 独立复核。
    const EAPI_REQ_HEX: &str = "7D398AA5036D61F11B22021C618C242421D51F26B6A0246E121BFC7B69A3481F62C92B909AF39DBF41E3032D9B8D0AD4255A14BDCC5BC706FCD0882A2B24200CED6B7441C4CAA692CC4F3A374162751C8F60B959A222F7C46E6094FC8DF43285";
    const EAPI_RES_HEX: &str = "DCC52B3013E9B66C038F8E027E580ECE28D2580181CD6590401207D8D6FCE455";
    const XEAPI_RES_B64: &str = "3MUrMBPptmwDj44CflgOznH2iO6O5m4Qv4Ko94OYxHuS9pi4zhf7XYnPepgRRyAN";
    const LINUXAPI_REQ_HEX: &str = "554652792E24D57614CCD2E7D216AE73EAFB2D1B7254C3F1F8662C1EE0E011F673342CAB6D703870EC51892547579002";

    fn client() -> ApiClient {
        ApiClient::new(None)
    }

    #[test]
    fn test_eapi_request_returns_url_and_data() {
        let out = decrypt_payload("eapi", EAPI_REQ_HEX, true).unwrap();
        assert_eq!(out["url"], "/api/song/detail");
        assert_eq!(out["data"], json!({ "id": 123 }));
        // 上游 eapiReqDecrypt 返回的是 {url, data}，不是 {url, body}
        assert!(out.get("body").is_none());
    }

    #[test]
    fn test_eapi_request_tolerates_whitespace() {
        let spaced = format!("\n\t{}  \r\n", EAPI_REQ_HEX);
        let out = decrypt_payload("eapi", &spaced, true).unwrap();
        assert_eq!(out["url"], "/api/song/detail");
    }

    #[test]
    fn test_eapi_response() {
        let out = decrypt_payload("eapi", EAPI_RES_HEX, false).unwrap();
        assert_eq!(out, json!({ "code": 200, "data": { "id": 456 } }));
    }

    #[test]
    fn test_weapi_response_reuses_eapi_decryption() {
        let out = decrypt_payload("weapi", EAPI_RES_HEX, false).unwrap();
        assert_eq!(out, json!({ "code": 200, "data": { "id": 456 } }));
    }

    #[test]
    fn test_linuxapi_request_decrypts_hex_to_json() {
        let out = decrypt_payload("linuxapi", LINUXAPI_REQ_HEX, true).unwrap();
        assert_eq!(out, json!({ "code": 200, "msg": "linuxapi vector" }));
    }

    #[test]
    fn test_linuxapi_response_passes_through_json() {
        let out = decrypt_payload("linuxapi", r#"{"code":200}"#, false).unwrap();
        assert_eq!(out, json!({ "code": 200 }));
    }

    #[test]
    fn test_xeapi_response_decrypts_base64() {
        let out = decrypt_payload("xeapi", XEAPI_RES_B64, false).unwrap();
        assert_eq!(out, json!({ "code": 200, "data": { "xeapi": true } }));
    }

    #[test]
    fn test_api_is_plain_json() {
        let out = decrypt_payload("api", r#"{"code":200}"#, true).unwrap();
        assert_eq!(out, json!({ "code": 200 }));
    }

    #[test]
    fn test_malformed_input_is_error() {
        assert!(decrypt_payload("api", "not json", false).is_err());
        assert!(decrypt_payload("linuxapi", "zzzz", true).is_err());
    }

    #[tokio::test]
    async fn test_decrypt_requires_data() {
        let client = client();
        let query = Query::new();
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 400);
        assert_eq!(resp.body["message"], "data is required");
    }

    #[tokio::test]
    async fn test_decrypt_accepts_empty_data_falls_back_to_hex_string() {
        // 空字符串在 JS 的 `||` 下会继续回退到 hexString
        let client = client();
        let query = Query::new()
            .param("data", "")
            .param("hexString", EAPI_RES_HEX)
            .param("isReq", "false");
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(
            resp.body["data"],
            json!({ "code": 200, "data": { "id": 456 } })
        );
    }

    #[tokio::test]
    async fn test_decrypt_rejects_weapi_request_direction() {
        let client = client();
        let query = Query::new().param("crypto", "weapi").param("data", "00");
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 400);
        assert!(resp.body["message"].as_str().unwrap().contains("RSA 私钥"));
    }

    #[tokio::test]
    async fn test_decrypt_rejects_xeapi_request_direction() {
        let client = client();
        let query = Query::new().param("crypto", "xeapi").param("data", "00");
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 400);
        assert!(resp.body["message"].as_str().unwrap().contains("X25519"));
    }

    #[tokio::test]
    async fn test_decrypt_rejects_unknown_crypto() {
        let client = client();
        let query = Query::new().param("crypto", "foo").param("data", "00");
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 400);
        assert_eq!(resp.body["message"], "未知加密方式: foo");
    }

    #[tokio::test]
    async fn test_decrypt_wraps_failures() {
        let client = client();
        let query = Query::new()
            .param("crypto", "api")
            .param("data", "not json");
        let resp = client.decrypt(&query).await.unwrap();
        assert_eq!(resp.status, 400);
        assert!(resp.body["message"]
            .as_str()
            .unwrap()
            .starts_with("解密失败: "));
    }
}
