use super::{reject, Query};
use crate::error::Result;
/// 歌单封面更新
/// 对应 Node.js module/playlist_cover_update.js + plugins/upload.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::image_upload::upload_image;
use serde_json::json;

impl ApiClient {
    /// 歌单封面更新
    /// 对应 /playlist/cover/update
    ///
    /// `img_data` 对应上游 multipart 的 `imgFile`：
    /// - `None` 表示未上传文件，等价于上游 `if (!query.imgFile)` 的 400 分支
    /// - 文件名 / MIME 通过 query 的 `img_name` / `img_mimetype` 传入
    ///   （默认 `cover.jpg` / `image/jpeg`），与 `avatar_upload` 约定一致
    pub async fn playlist_cover_update(
        &self,
        query: &Query,
        img_data: Option<&[u8]>,
    ) -> Result<ApiResponse> {
        let Some(file_data) = img_data else {
            return Ok(reject(
                400,
                json!({ "code": 400, "msg": "imgFile is required" }),
            ));
        };

        let img_name = query.get_or("img_name", "cover.jpg");
        let img_mimetype = query.get("img_mimetype");

        // 上游 `const uploadInfo = await uploadPlugin(query, request)`
        let info = upload_image(self, query, &img_name, file_data, img_mimetype).await?;

        // 上游 `{ id: query.id, coverImgId: uploadInfo.imgId }`——`id` 为裸取值，
        // 缺省时 `JSON.stringify` 会丢掉该键
        let mut data = json!({
            "id": query.get_or("id", ""),
            "coverImgId": info.img_id,
        });
        query.drop_absent_params(&mut data, &[("id", "id")]);

        let res = self
            .request(
                "/api/playlist/cover/update",
                data,
                query.to_option(CryptoType::Weapi),
            )
            .await?;

        // 合并结果：上游 `data: { ...uploadInfo, ...res.body }`
        let mut body = res.body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("url_pre".to_string(), json!(info.url_pre));
            obj.insert("imgId".to_string(), info.img_id);
        }

        Ok(ApiResponse {
            status: 200,
            body,
            cookie: res.cookie,
        })
    }
}
