use super::Query;
use crate::error::{NcmError, Result};
use crate::request::{ApiClient, ApiResponse, CryptoType};
use rand::Rng;
use serde_json::json;

impl ApiClient {
    /// 提交歌曲播放状态
    /// 对应 /relay/play/state/submit
    pub async fn relay_play_state_submit(&self, query: &Query) -> Result<ApiResponse> {
        let id = match query.get("id") {
            Some(id) if !id.is_empty() => id,
            _ => return Err(NcmError::InvalidParam("缺少必要参数：id".to_string())),
        };
        let session_id = query
            .get("sessionId")
            .map(str::to_string)
            .unwrap_or_else(|| {
                const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                let mut rng = rand::thread_rng();
                (0..12)
                    .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
                    .collect()
            });
        let play_state = json!({
            "resource": {
                "id": id,
                "type": query.get_or("type", "song"),
            },
            "progress": query.get("progress").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0),
            "sessionId": session_id,
            "playMode": query.get_or("playMode", "list_loop"),
        });
        self.request(
            "/api/relay/play/state/submit",
            json!({ "playStateSubmitReq": play_state.to_string() }),
            query.to_option(CryptoType::Weapi),
        )
        .await
    }
}
