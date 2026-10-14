use crate::crypto::XeapiPublicKeyState;
/// XEAPI 动态公钥获取 - 对应 Node.js 版本的 util/xeapiKey.js
///
/// Node 侧由 `generateConfig.js` 在启动时拉取公钥并写入 `os.tmpdir()/xeapi_public_key`，
/// `util/request.js` 只负责读取；Rust 侧改为按需拉取 + 进程内缓存（见 `ApiClient`）。
use crate::crypto::{form_urlencode, js_value_to_string, xeapi_decrypt_public_key, xeapi_sign};
use crate::util::config::{XEAPI_PUBLIC_KEY_URL, XEAPI_USER_AGENT};
use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, USER_AGENT};

/// 生成 16 位数字 nonce（对应 Node 的 generateNonce）
fn generate_nonce() -> String {
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| char::from(b'0' + rng.gen_range(0..10)))
        .collect()
}

/// 拉取 XEAPI 动态公钥
///
/// - `client`: 复用的 HTTP 客户端（Node 侧此处 `proxy: false`，不使用代理）
/// - `current`: 当前已缓存的公钥，用于携带 `currentKeyVersion` 并继承 `sk`
/// - `device_id`: 设备 ID
pub async fn fetch_xeapi_public_key(
    client: &reqwest::Client,
    current: Option<&XeapiPublicKeyState>,
    device_id: &str,
) -> Result<XeapiPublicKeyState, String> {
    let nonce = generate_nonce();
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let signature = xeapi_sign(&timestamp, &nonce);
    let current_key_version = current.map(|state| state.version.as_str()).unwrap_or("");

    // 字段顺序与 Node 的 URLSearchParams 一致
    let pairs: [(&str, String); 11] = [
        ("appVersion", "9.1.65".to_string()),
        ("currentKeyVersion", current_key_version.to_string()),
        ("deviceId", device_id.to_string()),
        ("nonce", nonce.clone()),
        ("os", "android".to_string()),
        ("requestType", "active".to_string()),
        ("signature", signature),
        ("t1", String::new()),
        ("t2", String::new()),
        ("timestamp", timestamp.clone()),
        ("uid", String::new()),
    ];
    let body = pairs
        .iter()
        .map(|(key, value)| format!("{}={}", form_urlencode(key), form_urlencode(value)))
        .collect::<Vec<_>>()
        .join("&");

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(XEAPI_USER_AGENT));
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-www-form-urlencoded"),
    );
    // Node: Cookie 为 `deviceId ? \`deviceId=${encodeURIComponent(deviceId)}\` : ''`
    let cookie = if device_id.is_empty() {
        String::new()
    } else {
        format!("deviceId={}", urlencoding::encode(device_id))
    };
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.insert(COOKIE, value);
    }

    let response = client
        .post(XEAPI_PUBLIC_KEY_URL)
        .headers(headers)
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = response.text().await.map_err(|e| e.to_string())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;

    let code = json.get("code").and_then(|code| code.as_i64());
    let data = json.get("data");
    let encrypted_data = data
        .and_then(|data| data.get("encryptedData"))
        .and_then(|value| value.as_str());

    if code != Some(200) || encrypted_data.is_none() {
        return Err("XEAPI 公钥请求失败".to_string());
    }
    let encrypted_data = encrypted_data.expect("已确认 encryptedData 存在");

    // 校验响应签名
    let response_timestamp = data
        .and_then(|data| data.get("timestamp"))
        .map(js_value_to_string)
        .unwrap_or_default();
    let response_signature = data
        .and_then(|data| data.get("signature"))
        .and_then(|value| value.as_str());

    match response_signature {
        Some(signature) if signature == xeapi_sign(&response_timestamp, &nonce) => {}
        _ => return Err("XEAPI 公钥响应签名不匹配".to_string()),
    }

    let mut state = xeapi_decrypt_public_key(encrypted_data)
        .ok_or_else(|| "XEAPI 公钥响应解密失败".to_string())?;

    // Node: if (!publicKey.sk && currentPublicKey.sk) publicKey.sk = currentPublicKey.sk
    if state.sk.is_none() {
        if let Some(current) = current {
            state.sk = current.sk.clone();
        }
    }
    if state.sk.is_none() {
        return Err("XEAPI 公钥响应缺少 sk".to_string());
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_nonce_is_16_digits() {
        for _ in 0..32 {
            let nonce = generate_nonce();
            assert_eq!(nonce.len(), 16);
            assert!(nonce.chars().all(|c| c.is_ascii_digit()));
        }
    }
}
