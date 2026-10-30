use super::Query;
use crate::error::Result;
/// 听歌打卡
/// 对应 Node.js module/scrobble.js
use crate::request::{ApiClient, ApiResponse, CryptoType};
use serde_json::json;

impl ApiClient {
    /// 听歌打卡
    /// 对应 /scrobble
    pub async fn scrobble(&self, query: &Query) -> Result<ApiResponse> {
        let cookie = match query.cookie.as_deref() {
            Some(cookie) if cookie.contains("os=") => regex_lite::Regex::new(r"os=[^;]+")
                .unwrap()
                .replace_all(cookie, "os=osx")
                .to_string(),
            Some(cookie) => format!("{cookie}; os=osx"),
            None => "os=osx".to_string(),
        };
        let startplay_data = json!({
            "logs": serde_json::to_string(&json!([{
                "action": "startplay",
                "json": {
                    "id": query.get_or("id", "0"),
                    "type": "song",
                    "mainsite": "1",
                    "mainsiteWeb": "1",
                    "content": format!("id={}", query.get_or("sourceid", "")),
                }
            }])).unwrap_or_default()
        });
        let play_data = json!({
            "logs": serde_json::to_string(&json!([{
                "action": "play",
                "json": {
                    "download": 0,
                    "end": "playend",
                    "id": query.get_or("id", "0"),
                    "sourceId": query.get_or("sourceid", ""),
                    "time": query.get_or("time", "0").parse::<i64>().unwrap_or(0),
                    "type": "song",
                    "wifi": 0,
                    "source": "list",
                    "mainsite": "1",
                    "mainsiteWeb": "1",
                    "content": format!("id={}", query.get_or("sourceid", ""))
                }
            }])).unwrap_or_default()
        });
        let mut options = query.to_option(CryptoType::Eapi);
        options.cookie = Some(cookie);
        options.domain = Some("https://clientlog.music.163.com".to_string());
        let startplay = self
            .request("/api/feedback/weblog", startplay_data, options.clone())
            .await?;
        let play = self
            .request("/api/feedback/weblog", play_data, options)
            .await?;
        Ok(ApiResponse {
            status: 200,
            body: json!({
                "code": 200,
                "data": "success",
                "details": { "startplay": startplay.body, "play": play.body },
            }),
            cookie: startplay.cookie,
        })
    }
}
