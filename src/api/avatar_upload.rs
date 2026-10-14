use super::Query;
use crate::error::Result;
/// 上传头像
/// 对应 Node.js module/avatar_upload.js + plugins/upload.js
///
/// 注意: 此端点需要调用者提供图片文件的二进制数据。
/// 由于 Rust SDK 是纯库，文件读取由调用者负责。
use crate::request::{ApiClient, ApiResponse, CryptoType};
use crate::util::image_upload::upload_image;
use serde_json::json;

impl ApiClient {
    /// 上传头像图片
    ///
    /// query 参数:
    /// - `img_data`: 不通过 Query 传递，由专用参数传入
    /// - `img_name`: 图片文件名
    /// - `img_mimetype`: 图片 MIME 类型，默认 "image/jpeg"
    ///
    /// 步骤与上游 `plugins/upload.js` + `module/avatar_upload.js` 一致:
    /// 1. 申请 NOS 上传 token
    /// 2. 上传文件到 NOS
    /// 3. 调用头像更新接口
    pub async fn avatar_upload(&self, query: &Query, img_data: Vec<u8>) -> Result<ApiResponse> {
        let img_name = query.get_or("img_name", "avatar.jpg");
        let img_mimetype = query.get_or("img_mimetype", "image/jpeg");

        // 上游 `const uploadInfo = await uploadPlugin(query, request)`
        let info = upload_image(self, query, &img_name, &img_data, Some(&img_mimetype)).await?;

        // 上游 `request('/api/user/avatar/upload/v1', { imgid: uploadInfo.imgId }, createOption(query))`
        let update_res = self
            .request(
                "/api/user/avatar/upload/v1",
                json!({ "imgid": info.img_id }),
                query.to_option(CryptoType::default()),
            )
            .await?;

        // 合并结果：上游 `data: { ...uploadInfo, ...res.body }`
        let mut body = update_res.body.clone();
        if let Some(obj) = body.as_object_mut() {
            obj.insert("url_pre".to_string(), json!(info.url_pre));
            obj.insert("imgId".to_string(), info.img_id);
        }

        Ok(ApiResponse {
            status: 200,
            body,
            cookie: update_res.cookie,
        })
    }
}
