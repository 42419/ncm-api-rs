use super::{copy_if_present, reject, Query};
use crate::error::Result;
/// 云盘上传 Token 与上传地址
/// 对应 Node.js module/cloud_upload_token.js（快照：api-enhanced v4.34.3）
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::file_helper::{cloud_token_ext, sanitize_filename_inline};
use crate::util::nos::{fetch_lbs, pick_upload_host, CLOUD_UPLOAD_BUCKET};
use serde_json::{json, Map, Value};

impl ApiClient {
    /// 获取云盘上传 Token 与上传地址
    /// 对应 /cloud/upload/token
    ///
    /// query 参数：`md5`、`fileSize`、`filename`（三者必传）、
    /// `bitrate`（默认 `999000`）
    ///
    /// 上游是一次复合调用：上传检查 → 分配 NOS token → 查询 LBS 上传地址，
    /// 最后把三步的结果合并返回（`data` 里带 `uploadUrl` 供调用方直接上传）。
    pub async fn cloud_upload_token(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `const { md5, fileSize, filename, bitrate = 999000 } = query`：
        // 解构默认值只在未传（`undefined`）时生效，传空串不会走默认值。
        let md5 = query.get("md5").unwrap_or("");
        let file_size = query.get("fileSize").unwrap_or("");
        let filename = query.get("filename").unwrap_or("");
        let bitrate = query.get("bitrate").unwrap_or("999000");

        // 上游 `if (!md5 || !fileSize || !filename)`——JS 里空串是 falsy，也要拦
        if md5.is_empty() || file_size.is_empty() || filename.is_empty() {
            return Ok(reject(
                400,
                json!({ "code": 400, "msg": "缺少必要参数: md5, fileSize, filename" }),
            ));
        }

        let ext = cloud_token_ext(filename);

        // 上传检查（createOption(query)，crypto 为空 → 上游回落到 eapi）
        let check_res = self
            .request(
                "/api/cloud/upload/check",
                json!({
                    "bitrate": bitrate,
                    "ext": "",
                    "length": file_size,
                    "md5": md5,
                    "songId": "0",
                    "version": 1,
                }),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 分配 NOS token（createOption(query, 'weapi')）
        let token_res = self
            .request(
                "/api/nos/token/alloc",
                json!({
                    "bucket": CLOUD_UPLOAD_BUCKET,
                    "ext": ext,
                    "filename": sanitize_filename_inline(filename),
                    "local": false,
                    "nos_product": 3,
                    "type": "audio",
                    "md5": md5,
                }),
                query.to_option(CryptoType::Weapi),
            )
            .await?;

        // 上游 `if (!tokenRes.body.result || !tokenRes.body.result.objectKey)`
        // ——两处都是 JS 的 falsy 判定，空串同样要报错
        let result = token_res.body.get("result").filter(|v| v.is_object());
        let object_key = result
            .and_then(|r| r.get("objectKey"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let (Some(result), Some(object_key)) = (result, object_key) else {
            // `result` 仍借用着 `token_res.body`，这里显式 clone 出 detail
            let detail = token_res.body.clone();
            return Ok(reject(
                500,
                json!({ "code": 500, "msg": "获取上传token失败", "detail": detail }),
            ));
        };

        // 上游对网络异常与响应结构异常给了不同的文案，这里分开报告
        let lbs = match fetch_lbs(&self.client).await {
            Ok(lbs) => lbs,
            Err(message) => {
                return Ok(reject(
                    500,
                    json!({ "code": 500, "msg": "获取上传服务器地址失败", "detail": message }),
                ))
            }
        };
        let Some(upload_host) = pick_upload_host(&lbs) else {
            let detail = lbs.clone();
            return Ok(reject(
                500,
                json!({ "code": 500, "msg": "获取上传服务器地址无效", "detail": detail }),
            ));
        };

        let mut data = Map::new();
        copy_if_present(&check_res.body, "needUpload", &mut data);
        copy_if_present(&check_res.body, "songId", &mut data);
        // 上游把 result 里的字段改名后输出
        if let Some(v) = result.get("token") {
            data.insert("uploadToken".to_string(), v.clone());
        }
        if let Some(v) = result.get("objectKey") {
            data.insert("objectKey".to_string(), v.clone());
        }
        if let Some(v) = result.get("resourceId") {
            data.insert("resourceId".to_string(), v.clone());
        }
        data.insert(
            "uploadUrl".to_string(),
            Value::String(format!(
                "{upload_host}/{CLOUD_UPLOAD_BUCKET}/{}?offset=0&complete=true&version=1.0",
                // 上游 `objectKey.replace(/\//g, '%2F')`
                object_key.replace('/', "%2F"),
            )),
        );
        data.insert(
            "bucket".to_string(),
            Value::String(CLOUD_UPLOAD_BUCKET.to_string()),
        );
        data.insert("md5".to_string(), Value::String(md5.to_string()));
        data.insert("fileSize".to_string(), Value::String(file_size.to_string()));
        data.insert("filename".to_string(), Value::String(filename.to_string()));

        Ok(ApiResponse {
            status: 200,
            body: json!({ "code": 200, "data": Value::Object(data) }),
            cookie: check_res.cookie,
        })
    }
}
