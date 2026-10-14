/// 文件上传路由处理
///
/// avatar_upload / voice_upload / cloud / playlist_cover_update 需要 multipart
/// 表单处理，签名与普通 API 不同，单独实现
use super::{build_error_response, build_success_response, AppState};
use crate::api::Query;
use axum::extract::{Multipart, State};
use axum::response::Response;

/// 头像上传处理
///
/// POST /avatar/upload
/// Content-Type: multipart/form-data
///
/// 表单字段:
/// - `imgFile`: 图片文件（必需）
/// - 其他文本字段作为 Query 参数
pub async fn handle_avatar_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut query = Query::new();
    let mut img_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "imgFile" {
            if let Some(fname) = field.file_name() {
                query
                    .params
                    .insert("img_name".to_string(), fname.to_string());
            }
            match field.bytes().await {
                Ok(bytes) => img_data = Some(bytes.to_vec()),
                Err(e) => {
                    return build_error_response(crate::error::NcmError::Unknown(format!(
                        "Failed to read upload data: {}",
                        e
                    )));
                }
            }
        } else {
            let value = field.text().await.unwrap_or_default();
            if name == "cookie" {
                query.cookie = Some(value);
            } else {
                query.params.insert(name, value);
            }
        }
    }

    let Some(data) = img_data else {
        return build_error_response(crate::error::NcmError::Unknown(
            "Missing imgFile field".to_string(),
        ));
    };

    let start = std::time::Instant::now();
    match state.client.avatar_upload(&query, data).await {
        Ok(resp) => {
            tracing::info!(
                "/avatar/upload -> {} ({:.1?})",
                resp.status,
                start.elapsed()
            );
            build_success_response(resp)
        }
        Err(e) => {
            tracing::warn!("/avatar/upload -> ERROR: {} ({:.1?})", e, start.elapsed());
            build_error_response(e)
        }
    }
}

/// 音频上传处理
///
/// POST /voice/upload
/// Content-Type: multipart/form-data
///
/// 表单字段:
/// - `file`: 音频文件（必需）
/// - 其他文本字段作为 Query 参数
pub async fn handle_voice_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut query = Query::new();
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = String::from("audio.mp3");
    let mut file_mimetype: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" || name == "songFile" {
            if let Some(fname) = field.file_name() {
                file_name = fname.to_string();
            }
            file_mimetype = field.content_type().map(|s| s.to_string());
            match field.bytes().await {
                Ok(bytes) => file_data = Some(bytes.to_vec()),
                Err(e) => {
                    return build_error_response(crate::error::NcmError::Unknown(format!(
                        "Failed to read upload data: {}",
                        e
                    )));
                }
            }
        } else {
            let value = field.text().await.unwrap_or_default();
            if name == "cookie" {
                query.cookie = Some(value);
            } else {
                query.params.insert(name, value);
            }
        }
    }

    let Some(data) = file_data else {
        return build_error_response(crate::error::NcmError::Unknown(
            "Missing file field".to_string(),
        ));
    };

    let start = std::time::Instant::now();
    match state
        .client
        .voice_upload(&query, &file_name, data, file_mimetype.as_deref())
        .await
    {
        Ok(resp) => {
            tracing::info!("/voice/upload -> {} ({:.1?})", resp.status, start.elapsed());
            build_success_response(resp)
        }
        Err(e) => {
            tracing::warn!("/voice/upload -> ERROR: {} ({:.1?})", e, start.elapsed());
            build_error_response(e)
        }
    }
}

/// 云盘整文件上传处理
///
/// POST /cloud
/// Content-Type: multipart/form-data
///
/// 表单字段:
/// - `songFile`: 音频文件（必需，对应上游的 `query.songFile`）
/// - 其他文本字段作为 Query 参数
///
/// 上游 `module/cloud.js` 开头有一句
/// `Buffer.from(query.songFile.name, 'latin1').toString('utf-8')`：那是为了
/// 修正 multer（Node）把文件名按 latin1 解码造成的乱码。Rust 这边
/// multer/axum 已经给出正确的 UTF-8 文件名，再做一次 latin1→UTF-8 转换反而会
/// 把中文文件名弄坏，因此这里不做该转换（两边最终结果都是正确的 UTF-8）。
pub async fn handle_cloud_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut query = Query::new();
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name = String::new();
    let mut file_mimetype: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "songFile" {
            if let Some(fname) = field.file_name() {
                file_name = fname.to_string();
            }
            file_mimetype = field.content_type().map(|s| s.to_string());
            match field.bytes().await {
                Ok(bytes) => file_data = Some(bytes.to_vec()),
                Err(e) => {
                    return build_error_response(crate::error::NcmError::Unknown(format!(
                        "Failed to read upload data: {}",
                        e
                    )));
                }
            }
        } else {
            let value = field.text().await.unwrap_or_default();
            if name == "cookie" {
                query.cookie = Some(value);
            } else {
                query.params.insert(name, value);
            }
        }
    }

    let Some(data) = file_data else {
        return build_error_response(crate::error::NcmError::Unknown(
            "Missing songFile field".to_string(),
        ));
    };

    let start = std::time::Instant::now();
    match state
        .client
        .cloud(&query, &file_name, &data, file_mimetype.as_deref())
        .await
    {
        Ok(resp) => {
            tracing::info!("/cloud -> {} ({:.1?})", resp.status, start.elapsed());
            build_success_response(resp)
        }
        Err(e) => {
            tracing::warn!("/cloud -> ERROR: {} ({:.1?})", e, start.elapsed());
            build_error_response(e)
        }
    }
}

/// 歌单封面上传处理
///
/// POST /playlist/cover/update
/// Content-Type: multipart/form-data
///
/// 表单字段:
/// - `imgFile`: 图片文件（对应上游 `query.imgFile`）。**允许缺失**：上游
///   `if (!query.imgFile)` 会返回 400 `imgFile is required`，这里交给
///   `playlist_cover_update` 复刻同一响应
/// - 其他文本字段作为 Query 参数
pub async fn handle_playlist_cover_update(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Response {
    let mut query = Query::new();
    let mut img_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();

        if name == "imgFile" {
            if let Some(fname) = field.file_name() {
                query
                    .params
                    .insert("img_name".to_string(), fname.to_string());
            }
            if let Some(content_type) = field.content_type() {
                query
                    .params
                    .insert("img_mimetype".to_string(), content_type.to_string());
            }
            match field.bytes().await {
                Ok(bytes) => img_data = Some(bytes.to_vec()),
                Err(e) => {
                    return build_error_response(crate::error::NcmError::Unknown(format!(
                        "Failed to read upload data: {}",
                        e
                    )));
                }
            }
        } else {
            let value = field.text().await.unwrap_or_default();
            if name == "cookie" {
                query.cookie = Some(value);
            } else {
                query.params.insert(name, value);
            }
        }
    }

    let start = std::time::Instant::now();
    match state
        .client
        .playlist_cover_update(&query, img_data.as_deref())
        .await
    {
        Ok(resp) => {
            tracing::info!(
                "/playlist/cover/update -> {} ({:.1?})",
                resp.status,
                start.elapsed()
            );
            build_success_response(resp)
        }
        Err(e) => {
            tracing::warn!(
                "/playlist/cover/update -> ERROR: {} ({:.1?})",
                e,
                start.elapsed()
            );
            build_error_response(e)
        }
    }
}
