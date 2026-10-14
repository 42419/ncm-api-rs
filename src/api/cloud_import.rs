use super::Query;
use crate::error::Result;
/// 云盘导入歌曲
/// 对应 Node.js module/cloud_import.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{json, Value};

impl ApiClient {
    /// 云盘导入歌曲（检查 + 导入，两步复合调用）
    /// 对应 /cloud/import
    ///
    /// 上游 `module/cloud_import.js` 的语义：
    /// 1. 先 `query.id = query.id || -2`、`artist` / `album` 缺省为「未知」
    /// 2. 请求 `/api/cloud/upload/check/v2`
    /// 3. 再请求 `/api/cloud/user/song/import`，其 `songId` **取自第 2 步响应**
    ///    的 `data[0].songId`（不是请求参数）
    pub async fn cloud_import(&self, query: &Query) -> Result<ApiResponse> {
        // —— 第 1 步：上传检查 ——
        // 上游 `query.id = query.id || -2`（空串是 falsy），所以 songId 恒有值；
        // `md5` / `bitrate` / `fileSize` 是裸取值，缺省时丢键
        let song_id_param = match query.get("id") {
            Some(id) if !id.is_empty() => json!(id),
            _ => json!(-2),
        };
        let mut check_song = json!({
            "md5": query.get_or("md5", ""),
            "songId": song_id_param,
            "bitrate": query.get_or("bitrate", ""),
            "fileSize": query.get_or("fileSize", "")
        });
        query.drop_absent_params(
            &mut check_song,
            &[
                ("md5", "md5"),
                ("bitrate", "bitrate"),
                ("fileSize", "fileSize"),
            ],
        );
        let check_data = json!({
            "uploadType": 0,
            "songs": json!([check_song]).to_string()
        });

        let check_res = self
            .request(
                "/api/cloud/upload/check/v2",
                check_data,
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 上游 `res.body.data[0].songId`——裸取值，缺失时该键会被丢掉
        let resolved_song_id = check_res
            .body
            .get("data")
            .and_then(Value::as_array)
            .and_then(|list| list.first())
            .and_then(|item| item.get("songId"))
            .cloned();

        // —— 第 2 步：导入 ——
        // `bitrate` / `song` 裸取值（缺省丢键）；`artist` / `album` 有默认值；
        // `fileName` 是 `query.song + '.' + query.fileType`——两者缺省时都会
        // 拼成字面量 `undefined`
        let file_type = query.get("fileType").unwrap_or("undefined");
        let mut import_song = json!({
            "bitrate": query.get_or("bitrate", ""),
            "song": query.get_or("song", ""),
            "artist": query.get_or("artist", "未知"),
            "album": query.get_or("album", "未知"),
            "fileName": format!(
                "{}.{}",
                query.get("song").unwrap_or("undefined"),
                file_type
            )
        });
        if let Some(song_id) = resolved_song_id {
            if let Some(obj) = import_song.as_object_mut() {
                obj.insert("songId".to_string(), song_id);
            }
        }
        query.drop_absent_params(
            &mut import_song,
            &[("bitrate", "bitrate"), ("song", "song")],
        );

        let import_data = json!({
            "uploadType": 0,
            "songs": json!([import_song]).to_string()
        });

        self.request(
            "/api/cloud/user/song/import",
            import_data,
            query.to_option(CryptoType::default()),
        )
        .await
    }
}
