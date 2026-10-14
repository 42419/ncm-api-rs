//! NOS 对象存储上传辅助
//!
//! 对应上游 `plugins/songUpload.js` 里与 LBS 地址查询、bucket 相关的部分——
//! `module/cloud_upload_token.js` 与 `module/cloud.js` 都会用到同一套逻辑与常量。

use serde_json::Value;
use std::time::Duration;

/// 云盘上传 bucket。上游 `plugins/songUpload.js` 与
/// `module/cloud_upload_token.js` 里都写死为这个值。
pub const CLOUD_UPLOAD_BUCKET: &str = "jd-musicrep-privatecloud-audio-public";

/// LBS 上传地址查询接口（上游写死）。
const LBS_URL: &str = "https://wanproxy.127.net/lbs?version=1.0";

/// 查询 NOS 上传服务器地址。
///
/// 对应上游对
/// `https://wanproxy.127.net/lbs?version=1.0&bucketname=<bucket>` 发起的
/// `axios({ method: 'get', timeout: 10000 })`。axios 默认会尝试把响应体当 JSON
/// 解析、失败则退回原始字符串，这里复刻同样的行为。
///
/// 返回 `Err(message)` 对应上游 catch 到异常时的 `error.message`。
pub async fn fetch_lbs(client: &reqwest::Client) -> Result<Value, String> {
    let url = format!("{LBS_URL}&bucketname={CLOUD_UPLOAD_BUCKET}");
    let resp = client
        .get(&url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

/// 从 LBS 响应里取上传服务器地址，对应上游的 `lbs.upload[0]`。
///
/// 缺失或为空串时返回 `None`——上游 `!lbs.upload || !lbs.upload[0]`（空串是
/// falsy）会走「获取上传服务器地址无效」的分支。
pub fn pick_upload_host(lbs: &Value) -> Option<&str> {
    lbs.get("upload")
        .and_then(|upload| upload.get(0))
        .and_then(|host| host.as_str())
        .filter(|host| !host.is_empty())
}
