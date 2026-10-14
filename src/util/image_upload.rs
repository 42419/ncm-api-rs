//! 图片上传辅助
//!
//! 对应上游 `plugins/upload.js`：申请 NOS token → 把图片 POST 到
//! `nosup-hz1.127.net/yyimgs/...` → 返回 `{ url_pre, imgId }`。
//! `module/avatar_upload.js` 与 `module/playlist_cover_update.js` 共用这一套。

use crate::api::Query;
use crate::error::{NcmError, Result};
use crate::request::{ApiClient, CryptoType};
use serde_json::{json, Value};

/// 上传结果，对应上游 `plugins/upload.js` 返回的 `{ url_pre, imgId }`
#[derive(Debug, Clone)]
pub struct ImageUploadInfo {
    /// `https://p1.music.126.net/<objectKey>`
    pub url_pre: String,
    /// 上游 `res.body.result.docId`；键缺失时等价于 JS 的 `undefined`
    pub img_id: Value,
}

/// 上传图片到 NOS，等价于上游 `plugins/upload.js`。
///
/// - `filename`：对应 `query.imgFile.name`
/// - `file_data`：对应 `getUploadData(query.imgFile)`（整块读入内存）
/// - `mimetype`：对应 `query.imgFile.mimetype`，缺省按 `image/jpeg`
pub async fn upload_image(
    client: &ApiClient,
    query: &Query,
    filename: &str,
    file_data: &[u8],
    mimetype: Option<&str>,
) -> Result<ImageUploadInfo> {
    // 申请 NOS token（上游 `createOption(query, 'weapi')`）
    let token_res = client
        .request(
            "/api/nos/token/alloc",
            json!({
                "bucket": "yyimgs",
                "ext": "jpg",
                "filename": filename,
                "local": false,
                "nos_product": 0,
                "return_body": "{\"code\":200,\"size\":\"$(ObjectSize)\"}",
                "type": "other",
            }),
            query.to_option(CryptoType::Weapi),
        )
        .await?;

    let result = &token_res.body["result"];
    let object_key = result["objectKey"].as_str().unwrap_or_default();
    let token = result["token"].as_str().unwrap_or_default();

    let upload_url = format!(
        "https://nosup-hz1.127.net/yyimgs/{}?offset=0&complete=true&version=1.0",
        object_key
    );

    let response = client
        .client
        .post(&upload_url)
        .header("x-nos-token", token)
        .header("Content-Type", mimetype.unwrap_or("image/jpeg"))
        .body(file_data.to_vec())
        .send()
        .await?;

    // 上游走 axios，非 2xx 会抛异常（默认 validateStatus 只认 2xx）
    if !response.status().is_success() {
        return Err(NcmError::Unknown(format!(
            "Request failed with status code {}",
            response.status().as_u16()
        )));
    }

    Ok(ImageUploadInfo {
        url_pre: format!("https://p1.music.126.net/{}", object_key),
        img_id: result["docId"].clone(),
    })
}
