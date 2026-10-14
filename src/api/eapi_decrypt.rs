use super::Query;
/// EAPI 解密
/// 对应 Node.js module/eapi_decrypt.js
use crate::crypto::{eapi_req_decrypt, eapi_res_decrypt, strip_js_whitespace};
use crate::error::Result;
use crate::request::{ApiClient, ApiResponse};
use serde_json::json;

impl ApiClient {
    /// EAPI 解密
    /// 对应 /eapi/decrypt
    ///
    /// 参数：
    /// - hexString: 要解密的十六进制字符串
    /// - isReq: 是否为请求解密（默认 true），设为 "false" 则为响应解密
    pub fn eapi_decrypt(&self, query: &Query) -> Result<ApiResponse> {
        let hex_string = match query.get("hexString") {
            Some(s) => s,
            None => {
                return Ok(ApiResponse {
                    status: 400,
                    body: json!({
                        "code": 400,
                        "message": "hex string is required"
                    }),
                    cookie: vec![],
                });
            }
        };

        let is_req = query.get_or("isReq", "true") != "false";
        // 上游为 `hexString.replace(/\s/g, '')`，需按 JS 的 \s 语义剥离
        let pure_hex = strip_js_whitespace(hex_string);

        let data = if is_req {
            // 上游 eapiReqDecrypt 返回 { url, data }
            match eapi_req_decrypt(&pure_hex) {
                Some((url, body)) => json!({ "url": url, "data": body }),
                None => json!(null),
            }
        } else {
            match eapi_res_decrypt(&pure_hex) {
                Some(val) => val,
                None => json!(null),
            }
        };

        Ok(ApiResponse {
            status: 200,
            body: json!({
                "code": 200,
                "data": data
            }),
            cookie: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// eapi 请求密文（url-36cd479b6b5-json-36cd479b6b5-md5），
    /// 已用 openssl `enc -d -aes-128-ecb` 独立复核。
    const EAPI_REQ_HEX: &str = "7D398AA5036D61F11B22021C618C242421D51F26B6A0246E121BFC7B69A3481F62C92B909AF39DBF41E3032D9B8D0AD4255A14BDCC5BC706FCD0882A2B24200CED6B7441C4CAA692CC4F3A374162751C8F60B959A222F7C46E6094FC8DF43285";

    fn client() -> ApiClient {
        ApiClient::new(None)
    }

    #[test]
    fn test_request_decrypt_returns_url_and_data() {
        let client = client();
        let query = Query::new().param("hexString", EAPI_REQ_HEX);
        let resp = client.eapi_decrypt(&query).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body["data"]["url"], "/api/song/detail");
        assert_eq!(resp.body["data"]["data"], json!({ "id": 123 }));
        // 上游 eapiReqDecrypt 返回 {url, data}，不是 {url, body}
        assert!(resp.body["data"].get("body").is_none());
    }

    #[test]
    fn test_whitespace_is_stripped() {
        let client = client();
        let spaced = format!("\n  {}\t", EAPI_REQ_HEX);
        let query = Query::new().param("hexString", &spaced);
        let resp = client.eapi_decrypt(&query).unwrap();
        assert_eq!(resp.body["data"]["url"], "/api/song/detail");
    }

    #[test]
    fn test_missing_hex_string() {
        let client = client();
        let resp = client.eapi_decrypt(&Query::new()).unwrap();
        assert_eq!(resp.status, 400);
        assert_eq!(resp.body["message"], "hex string is required");
    }
}
