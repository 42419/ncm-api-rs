use super::Query;
use crate::error::{NcmError, Result};
/// 收藏/取消收藏歌曲到歌单
/// 对应 Node.js module/playlist_tracks.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::{json, Value};

/// 上游请求路径（正常与 512 重试用的是同一个）
const URL: &str = "/api/playlist/manipulate/tracks";

/// 构造请求体，对应上游 `{ op: query.op, pid: query.pid, trackIds, imme: 'true' }`。
///
/// `op` / `pid` 都是裸取值，缺省时会被 `JSON.stringify` 丢掉。
fn build_data(query: &Query, tracks: &[String]) -> Value {
    let mut data = json!({
        "op": query.get_or("op", ""),
        "pid": query.get_or("pid", ""),
        "trackIds": serde_json::to_string(tracks).unwrap_or_default(),
        "imme": "true",
    });
    query.drop_absent_params(&mut data, &[("op", "op"), ("pid", "pid")]);
    data
}

impl ApiClient {
    /// 收藏/删除歌曲到歌单
    /// 对应 /playlist/tracks
    ///
    /// 复刻上游的重试逻辑：请求抛错且 `body.code === 512`（歌单曲目超量）时，
    /// 用 `[...tracks, ...tracks]` 重发一次。
    pub async fn playlist_tracks(&self, query: &Query) -> Result<ApiResponse> {
        // 上游 `query.tracks.split(',')`——不做 trim / 过滤
        let tracks: Vec<String> = query
            .get_or("tracks", "")
            .split(',')
            .map(|s| s.to_string())
            .collect();

        let option = query.to_option(CryptoType::default());
        match self
            .request(URL, build_data(query, &tracks), option.clone())
            .await
        {
            Ok(res) => Ok(res),
            Err(NcmError::Api { code: 512, .. }) => {
                let doubled: Vec<String> = tracks.iter().chain(tracks.iter()).cloned().collect();
                self.request(URL, build_data(query, &doubled), option).await
            }
            Err(other) => Err(other),
        }
    }
}
