use super::{is_js_falsy, reject, Query};
use crate::error::Result;
/// 云盘整文件上传
/// 对应 Node.js module/cloud.js + plugins/songUpload.js（快照：api-enhanced v4.34.3）
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::file_helper::{get_file_extension, sanitize_filename};
use crate::util::nos::{fetch_lbs, pick_upload_host, CLOUD_UPLOAD_BUCKET};
use md5::{Digest, Md5};
use serde_json::{json, Map, Value};

/// 上游 `const bitrate = 999000`（数字常量，参与请求时转成字符串）
const BITRATE: &str = "999000";

/// 上游对象取值 `obj.key` 的等价写法（缺失即 `undefined`）
fn get<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)
}

/// 上游的真值判定 `if (x)`（`0` / `""` / `null` 都是假）
fn is_truthy(value: Option<&Value>) -> bool {
    value.map(|v| !is_js_falsy(v)).unwrap_or(false)
}

impl ApiClient {
    /// 上传整首音乐到云盘
    /// 对应 /cloud
    ///
    /// 上游接收 multipart 的 `songFile` 字段，然后串起一整条链路：
    /// 上传检查 → 解析音频元数据 → 分配 NOS token →（需要时）上传到 NOS
    /// → 提交云盘信息 → 发布。
    ///
    /// Rust 侧是纯库，文件读取由调用方负责：
    /// - `file_name`: 原始文件名（如 `"song.mp3"`）
    /// - `file_data`: 文件完整二进制数据（上游流式上传，这里是整块读入内存）
    /// - `file_mimetype`: MIME 类型，如 `"audio/mpeg"`，缺省按 `audio/mpeg`
    ///
    /// **与上游的已知差异**：上游用 `music-metadata` 解析 title / album / artist
    /// 填进云盘信息，Rust 侧暂未引入音频元数据解析，因此恒为上游 try/catch 的
    /// 兜底值（`song` 用清洗后的文件名、album / artist 用「未知专辑」
    /// 「未知艺术家」）。链路、请求字段与返回结构一致，但上传后的标签不如上游完整。
    pub async fn cloud(
        &self,
        query: &Query,
        file_name: &str,
        file_data: &[u8],
        file_mimetype: Option<&str>,
    ) -> Result<ApiResponse> {
        let mimetype = file_mimetype.unwrap_or("audio/mpeg");
        let file_size = file_data.len();
        let file_md5 = format!("{:x}", Md5::digest(file_data));

        // 上游 `getFileExtension(query.songFile.name)` / `sanitizeFilename(...)`
        let ext = get_file_extension(file_name);
        let filename = sanitize_filename(file_name);

        // —— 上传检查 ——
        let check_res = self
            .request(
                "/api/cloud/upload/check",
                json!({
                    "bitrate": BITRATE,
                    "ext": "",
                    "length": file_size,
                    "md5": file_md5.as_str(),
                    "songId": "0",
                    "version": 1,
                }),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // —— 分配上传 token（bucket 为空串）——
        let token_res = self
            .request(
                "/api/nos/token/alloc",
                json!({
                    "bucket": "",
                    "ext": ext.as_str(),
                    "filename": filename.as_str(),
                    "local": false,
                    "nos_product": 3,
                    "type": "audio",
                    "md5": file_md5.as_str(),
                }),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `!tokenRes.body.result || !tokenRes.body.result.resourceId`
        let resource_id = get(&token_res.body, "result")
            .and_then(|result| get(result, "resourceId"))
            .filter(|v| !is_js_falsy(v))
            .cloned();
        let Some(resource_id) = resource_id else {
            let detail = token_res.body.clone();
            return Ok(reject(
                500,
                json!({ "code": 500, "msg": "获取上传token失败", "detail": detail }),
            ));
        };

        // 上游 `if (res.body.needUpload) { await uploadPlugin(...) }`
        if is_truthy(get(&check_res.body, "needUpload")) {
            if let Some(resp) = self
                .upload_song_file(query, &ext, &filename, file_data, mimetype, &file_md5)
                .await?
            {
                return Ok(resp);
            }
        }

        // —— 提交云盘信息 ——
        // 上游 `song: songName || filename`、`album: album || '未知专辑'`、
        // `artist: artist || '未知艺术家'`。songName / album / artist 来自
        // music-metadata 解析结果，解析失败时被 try/catch 吞掉保持空串，于是
        // 三条都会回退——Rust 侧未实现元数据解析，所以恒为回退值。
        let mut info_data = Map::new();
        info_data.insert("md5".to_string(), Value::String(file_md5.clone()));
        // 上游 `songid: res.body.songId`，缺失时该键会被 JSON.stringify 丢掉
        if let Some(v) = get(&check_res.body, "songId") {
            info_data.insert("songid".to_string(), v.clone());
        }
        info_data.insert("filename".to_string(), Value::String(file_name.to_string()));
        info_data.insert("song".to_string(), Value::String(filename.clone()));
        info_data.insert("album".to_string(), Value::String("未知专辑".to_string()));
        info_data.insert(
            "artist".to_string(),
            Value::String("未知艺术家".to_string()),
        );
        info_data.insert("bitrate".to_string(), Value::String(BITRATE.to_string()));
        info_data.insert("resourceId".to_string(), resource_id);

        let res2 = self
            .request(
                "/api/upload/cloud/info/v2",
                Value::Object(info_data),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `if (res2.body.code !== 200)`——严格比较
        if get(&res2.body, "code") != Some(&json!(200)) {
            let status = if res2.status == 0 { 500 } else { res2.status };
            let code = match get(&res2.body, "code") {
                Some(v) if !is_js_falsy(v) => v.clone(),
                _ => json!(500),
            };
            let msg = match get(&res2.body, "msg") {
                Some(v) if !is_js_falsy(v) => v.clone(),
                _ => json!("上传云盘信息失败"),
            };
            let detail = res2.body.clone();
            return Ok(reject(
                status,
                json!({ "code": code, "msg": msg, "detail": detail }),
            ));
        }

        // —— 发布 ——
        let mut pub_data = Map::new();
        if let Some(v) = get(&res2.body, "songId") {
            pub_data.insert("songid".to_string(), v.clone());
        }
        let res3 = self
            .request(
                "/api/cloud/pub/v2",
                Value::Object(pub_data),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `body: { ...res.body, ...res3.body }`——直接展开合并，不额外包 code
        let mut body = Map::new();
        if let Some(obj) = check_res.body.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }
        if let Some(obj) = res3.body.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }

        Ok(ApiResponse {
            status: 200,
            body: Value::Object(body),
            cookie: check_res.cookie,
        })
    }

    /// 对应上游 `plugins/songUpload.js`：分配上传 token → 查询 LBS 地址 →
    /// 把整块数据 POST 到对象存储。
    ///
    /// 返回 `Ok(None)` 表示上传成功；`Ok(Some(resp))` 对应上游
    /// `Promise.reject(uploadError)`（`cloud.js` 会原样抛给客户端），调用方
    /// 应直接把该响应返回。
    async fn upload_song_file(
        &self,
        query: &Query,
        ext: &str,
        filename: &str,
        file_data: &[u8],
        mimetype: &str,
        file_md5: &str,
    ) -> Result<Option<ApiResponse>> {
        let upload_token_res = self
            .request(
                "/api/nos/token/alloc",
                json!({
                    "bucket": CLOUD_UPLOAD_BUCKET,
                    "ext": ext,
                    "filename": filename,
                    "local": false,
                    "nos_product": 3,
                    "type": "audio",
                    "md5": file_md5,
                }),
                query.to_option(CryptoType::Weapi),
            )
            .await?;

        // 上游 `!tokenRes.body.result || !tokenRes.body.result.objectKey`
        let result = get(&upload_token_res.body, "result").filter(|v| v.is_object());
        let object_key = result
            .and_then(|r| get(r, "objectKey"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let token = result
            .and_then(|r| get(r, "token"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let (Some(object_key), Some(token)) = (object_key, token) else {
            let detail = upload_token_res.body.clone();
            return Ok(Some(reject(
                500,
                json!({ "code": 500, "msg": "获取上传token失败", "detail": detail }),
            )));
        };

        let lbs = match fetch_lbs(&self.client).await {
            Ok(lbs) => lbs,
            Err(message) => {
                return Ok(Some(reject(
                    500,
                    json!({ "code": 500, "msg": "获取上传服务器地址失败", "detail": message }),
                )))
            }
        };
        let Some(upload_host) = pick_upload_host(&lbs) else {
            let detail = lbs.clone();
            return Ok(Some(reject(
                500,
                json!({ "code": 500, "msg": "获取上传服务器地址无效", "detail": detail }),
            )));
        };

        let upload_url = format!(
            "{upload_host}/{CLOUD_UPLOAD_BUCKET}/{}?offset=0&complete=true&version=1.0",
            // 上游 `objectKey.replace(/\//g, '%2F')`
            object_key.replace('/', "%2F"),
        );

        // 上游还显式设了 Content-Length，reqwest 会按 body 长度自动带上，无需手写
        let upload_result = self
            .client
            .post(&upload_url)
            .header("x-nos-token", token)
            .header("Content-MD5", file_md5)
            .header("Content-Type", mimetype)
            .body(file_data.to_vec())
            .send()
            .await;

        let upload_resp = match upload_result {
            Ok(resp) => resp,
            Err(e) => {
                return Ok(Some(reject(
                    500,
                    json!({ "code": 500, "msg": "文件上传失败", "detail": e.to_string() }),
                )))
            }
        };

        if !upload_resp.status().is_success() {
            // 上游走 axios 的 error 分支：status 取响应码、detail 取响应体
            let status = i64::from(upload_resp.status().as_u16());
            let detail = upload_resp.text().await.unwrap_or_default();
            return Ok(Some(reject(
                status,
                json!({ "code": status, "msg": "文件上传失败", "detail": detail }),
            )));
        }

        Ok(None)
    }
}
