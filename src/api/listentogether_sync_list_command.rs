use super::Query;
use crate::error::Result;
/// 一起听 更新播放列表
/// 对应 Node.js module/listentogether_sync_list_command.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 一起听 更新播放列表
    /// 对应 /listentogether/sync/list/command
    pub async fn listentogether_sync_list_command(&self, query: &Query) -> Result<ApiResponse> {
        let random_list_str = query.get_or("randomList", "");
        let display_list_str = query.get_or("displayList", "");
        let random_list: Vec<&str> = random_list_str.split(',').collect();
        let display_list: Vec<&str> = display_list_str.split(',').collect();
        // 上游 playlistParam 里 `commandType` / `version[0].userId` /
        // `version[0].version` 均为裸取值，缺省时丢键
        let mut playlist_param = json!({
            "commandType": query.get_or("commandType", ""),
            "version": [{
                "userId": query.get_or("userId", ""),
                "version": query.get_or("version", "")
            }],
            "anchorSongId": "",
            "anchorPosition": -1,
            "randomList": random_list,
            "displayList": display_list
        });
        query.drop_absent_params(&mut playlist_param, &[("commandType", "commandType")]);
        if let Some(ver) = playlist_param.get_mut("version").and_then(|v| v.get_mut(0)) {
            query.drop_absent_params(ver, &[("userId", "userId"), ("version", "version")]);
        }
        let mut data = json!({
            "roomId": query.get_or("roomId", ""),
            "playlistParam": playlist_param.to_string()
        });
        query.drop_absent_params(&mut data, &[("roomId", "roomId")]);
        self.request(
            "/api/listen/together/sync/list/command/report",
            data,
            query.to_option(CryptoType::Eapi),
        )
        .await
    }
}
