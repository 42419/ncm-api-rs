use super::{is_js_falsy, reject, Query};
use crate::error::Result;
/// 云盘上传完成
/// 对应 Node.js module/cloud_upload_complete.js（快照：api-enhanced v4.34.3）
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::file_helper::strip_last_extension;
use serde_json::{json, Map, Value};

impl ApiClient {
    /// 云盘上传完成（提交云盘信息 + 发布）
    /// 对应 /cloud/upload/complete
    ///
    /// query 参数：`songId`、`resourceId`、`md5`、`filename`（四者必传）、
    /// `song`、`artist`、`album`、`bitrate`（默认 `999000`）
    ///
    /// 上游是一次复合调用：先 `/api/upload/cloud/info/v2` 提交云盘信息，
    /// 成功后再 `/api/cloud/pub/v2` 发布。
    pub async fn cloud_upload_complete(&self, query: &Query) -> Result<ApiResponse> {
        // 解构默认值只在未传（`undefined`）时生效，传空串不会走默认值
        let song_id = query.get("songId").unwrap_or("");
        let resource_id = query.get("resourceId").unwrap_or("");
        let md5 = query.get("md5").unwrap_or("");
        let filename = query.get("filename").unwrap_or("");
        let bitrate = query.get("bitrate").unwrap_or("999000");

        // 上游 `if (!songId || !resourceId || !md5 || !filename)`——空串也是 falsy
        if song_id.is_empty() || resource_id.is_empty() || md5.is_empty() || filename.is_empty() {
            return Ok(reject(
                400,
                json!({ "code": 400, "msg": "缺少必要参数: songId, resourceId, md5, filename" }),
            ));
        }

        // 上游 `song || filename.replace(/\.[^.]+$/, '')`——空串会继续回退到文件名
        let song_name = match query.get("song").filter(|s| !s.is_empty()) {
            Some(song) => song.to_string(),
            None => strip_last_extension(filename).to_string(),
        };

        let res2 = self
            .request(
                "/api/upload/cloud/info/v2",
                json!({
                    "md5": md5,
                    "songid": song_id,
                    "filename": filename,
                    "song": song_name,
                    // 上游 `album || '未知专辑'`、`artist || '未知艺术家'`
                    "album": query.get("album").filter(|s| !s.is_empty()).unwrap_or("未知专辑"),
                    "artist": query
                        .get("artist")
                        .filter(|s| !s.is_empty())
                        .unwrap_or("未知艺术家"),
                    "bitrate": bitrate,
                    "resourceId": resource_id,
                }),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `if (res2.body.code !== 200)`——严格比较：缺失、或不是数字 200 都算失败
        if res2.body.get("code") != Some(&json!(200)) {
            // `res2.status || 500`、`res2.body.code || 500`、`res2.body.msg || '...'`
            // 都是 JS 的 `||`，所以 0 / 空串同样会回退到默认值
            let status = if res2.status == 0 { 500 } else { res2.status };
            let code = match res2.body.get("code") {
                Some(v) if !is_js_falsy(v) => v.clone(),
                _ => json!(500),
            };
            let msg = match res2.body.get("msg") {
                Some(v) if !is_js_falsy(v) => v.clone(),
                _ => json!("上传云盘信息失败"),
            };
            return Ok(reject(
                status,
                json!({ "code": code, "msg": msg, "detail": res2.body }),
            ));
        }

        // 上游 `{ songid: res2.body.songId }`：songId 缺失时该键会被丢掉
        let mut pub_data = Map::new();
        if let Some(v) = res2.body.get("songId") {
            pub_data.insert("songid".to_string(), v.clone());
        }

        let res3 = self
            .request(
                "/api/cloud/pub/v2",
                Value::Object(pub_data),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `data: { songId: res2.body.songId, ...res3.body }`：
        // 先放 res2 的 songId，再用 res3.body 覆盖同名键
        let mut data = Map::new();
        if let Some(v) = res2.body.get("songId") {
            data.insert("songId".to_string(), v.clone());
        }
        if let Some(obj) = res3.body.as_object() {
            for (k, v) in obj {
                data.insert(k.clone(), v.clone());
            }
        }

        Ok(ApiResponse {
            status: 200,
            body: json!({ "code": 200, "data": Value::Object(data) }),
            cookie: res2.cookie,
        })
    }
}
