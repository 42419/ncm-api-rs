use super::Query;
use crate::error::Result;
/// 歌曲播放链接 v1
/// 对应 Node.js module/song_url_v1.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 歌曲播放链接
    /// 对应 /song/url/v1
    ///
    /// **与上游的已知差异**：上游 `unblock === 'true'` 时会走
    /// `@neteasecloudmusicapienhanced/unblockmusic-utils` 的解灰分支（与
    /// `/song/url/match` 同一个依赖），该依赖在 Rust 侧不可用，因此此分支
    /// 未实现——请求照常走 `/api/song/enhance/player/url/v1`。
    pub async fn song_url_v1(&self, query: &Query) -> Result<ApiResponse> {
        let id = query.get_or("id", "0");
        // 上游 `level: query.level` 是裸取值：缺省时该键会被 JSON.stringify 丢掉
        let level = query.get("level");
        let mut data = json!({
            "ids": format!("[{}]", id),
            "encodeType": "flac"
        });
        if let Some(level) = level {
            data["level"] = json!(level);
        }
        // 上游 `if (data.level == 'sky') data.immerseType = 'c51'`
        if level == Some("sky") {
            data["immerseType"] = json!("c51");
        }
        self.request(
            "/api/song/enhance/player/url/v1",
            data,
            query.to_option(CryptoType::Xeapi),
        )
        .await
    }
}
