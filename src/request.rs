/// 请求模块 - 对应 Node.js 版本的 util/request.js
///
/// 核心功能：构造加密请求、Cookie 管理、UA 伪装
use crate::crypto;
use crate::error::{NcmError, Result};
use crate::util;
use crate::util::config::*;
use crate::util::cookie::{cookie_obj_to_string, cookie_to_json};
use crate::util::device::{generate_device_id, generate_wnmcid, random_hex};
use crate::util::ip::generate_random_chinese_ip;

use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, COOKIE, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

/// 特殊状态码集合（视为 200）
static SPECIAL_STATUS_CODES: LazyLock<std::collections::HashSet<i64>> =
    LazyLock::new(|| [201, 302, 400, 502, 800, 801, 802, 803].into());

/// 全局设备 ID（进程生命周期内固定）
static DEVICE_ID: LazyLock<String> = LazyLock::new(generate_device_id);
/// 全局 WNMCID
static WNMCID: LazyLock<String> = LazyLock::new(generate_wnmcid);
/// Cookie Domain 移除正则（编译一次，全局复用）
static DOMAIN_REGEX: LazyLock<regex_lite::Regex> =
    LazyLock::new(|| regex_lite::Regex::new(r"\s*Domain=[^;]+;?").unwrap());

/// 安全创建 HeaderValue，无效字符会被过滤
fn header_value(s: &str) -> HeaderValue {
    HeaderValue::from_str(s).unwrap_or_else(|_| {
        // 过滤掉非 ASCII 可见字符
        let safe: String = s
            .chars()
            .filter(|c| c.is_ascii() && !c.is_ascii_control())
            .collect();
        HeaderValue::from_str(&safe).unwrap_or_else(|_| HeaderValue::from_static(""))
    })
}

/// 上游 `util/request.js` 里反作弊 token 的落点分两种：
///
/// - `weapi` / `xeapi`：`headers['X-antiCheatToken'] = token`，是**独立 HTTP 头**；
/// - `eapi` / `api`：`header['X-antiCheatToken'] = token`，进 eapi header cookie 对象，
///   最终拼进 `Cookie` 头（那个分支另行处理，不走这里）。
///
/// Node 侧 `token` 是 `'v2'` / `'v3'` 时动态拉取、否则用静态常量；Rust 侧统一简化为
/// 静态 `CHECK_TOKEN`（与 `Query::check_token` 的文档说明一致）。
fn insert_anti_cheat_token(headers: &mut HeaderMap, check_token: bool) {
    if check_token {
        headers.insert("X-antiCheatToken", HeaderValue::from_static(CHECK_TOKEN));
    }
}

/// 加密类型
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CryptoType {
    Weapi,
    #[default]
    Eapi,
    Linuxapi,
    Xeapi,
    Api, // 明文
}

impl CryptoType {
    pub fn as_str(&self) -> &str {
        match self {
            CryptoType::Weapi => "weapi",
            CryptoType::Eapi => "eapi",
            CryptoType::Linuxapi => "linuxapi",
            CryptoType::Xeapi => "xeapi",
            CryptoType::Api => "api",
        }
    }
}

impl From<&str> for CryptoType {
    fn from(s: &str) -> Self {
        match s {
            "weapi" => CryptoType::Weapi,
            "linuxapi" => CryptoType::Linuxapi,
            "xeapi" => CryptoType::Xeapi,
            "api" => CryptoType::Api,
            _ => CryptoType::Eapi,
        }
    }
}

/// 请求选项
#[derive(Debug, Clone, Default)]
pub struct RequestOption {
    pub crypto: CryptoType,
    pub cookie: Option<String>,
    pub ua: Option<String>,
    pub proxy: Option<String>,
    pub real_ip: Option<String>,
    pub random_cn_ip: bool,
    pub e_r: Option<bool>,
    pub domain: Option<String>,
    pub check_token: bool,
}

/// API 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    pub status: i64,
    pub body: Value,
    #[serde(default)]
    pub cookie: Vec<String>,
}

/// API 客户端
#[derive(Debug, Clone)]
pub struct ApiClient {
    pub(crate) client: reqwest::Client,
    cookie: Option<String>,
    anonymous_token: Option<String>,
    /// 自定义设备 ID，None 则使用全局默认值
    device_id: Option<String>,
    /// XEAPI 动态公钥缓存
    ///
    /// 对应 Node 侧 tmpdir 下的 `xeapi_public_key` 文件：Node 需先由
    /// `generateConfig.js` 写好文件，否则请求会抛 `xeapi public key is missing`；
    /// Rust 侧改为首次使用时按需拉取。
    xeapi_public_key: Arc<RwLock<Option<crypto::XeapiPublicKeyState>>>,
    /// XEAPI 会话 (id, key)，来自响应头 `x-encr-ssid` / `x-encr-sskey`
    xeapi_session: Arc<RwLock<Option<(String, String)>>>,
}

impl ApiClient {
    /// 创建新的 API 客户端
    pub fn new(cookie: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .expect("Failed to create HTTP client");

        Self::from_client(client, cookie)
    }

    /// 创建带代理的 API 客户端
    pub fn with_proxy(cookie: Option<String>, proxy_url: &str) -> Result<Self> {
        let proxy = reqwest::Proxy::all(proxy_url)
            .map_err(|e| NcmError::Unknown(format!("Invalid proxy URL: {}", e)))?;
        let client = reqwest::Client::builder()
            .proxy(proxy)
            .build()
            .map_err(NcmError::Http)?;

        Ok(Self::from_client(client, cookie))
    }

    fn from_client(client: reqwest::Client, cookie: Option<String>) -> Self {
        Self {
            client,
            cookie,
            anonymous_token: None,
            device_id: None,
            xeapi_public_key: Arc::new(RwLock::new(None)),
            xeapi_session: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置 cookie
    pub fn set_cookie(&mut self, cookie: String) {
        self.cookie = Some(cookie);
    }

    /// 设置匿名 token
    pub fn set_anonymous_token(&mut self, token: String) {
        self.anonymous_token = Some(token);
    }

    /// 设置自定义设备 ID
    ///
    /// 不同客户端实例可使用不同设备 ID，避免共享全局 ID 触发风控
    pub fn set_device_id(&mut self, device_id: String) {
        self.device_id = Some(device_id);
    }

    /// 获取当前设备 ID（自定义优先，否则使用全局默认）
    fn get_device_id(&self) -> &str {
        self.device_id.as_deref().unwrap_or(&DEVICE_ID)
    }

    /// 获取 XEAPI 动态公钥；缓存为空时按需拉取
    async fn ensure_xeapi_public_key(&self) -> Result<crypto::XeapiPublicKeyState> {
        {
            let cached = self.xeapi_public_key.read().await;
            if let Some(state) = cached.as_ref() {
                return Ok(state.clone());
            }
        }

        let current = self.xeapi_public_key.read().await.clone();
        let state = util::xeapi_key::fetch_xeapi_public_key(
            &self.client,
            current.as_ref(),
            self.get_device_id(),
        )
        .await
        .map_err(NcmError::Crypto)?;

        *self.xeapi_public_key.write().await = Some(state.clone());
        Ok(state)
    }

    /// 刷新 XEAPI 动态公钥（丢弃缓存后重新拉取）
    pub async fn refresh_xeapi_public_key(&self) -> Result<crypto::XeapiPublicKeyState> {
        *self.xeapi_public_key.write().await = None;
        self.ensure_xeapi_public_key().await
    }

    /// 发起 API 请求 - 核心方法
    pub async fn request(
        &self,
        uri: &str,
        data: Value,
        options: RequestOption,
    ) -> Result<ApiResponse> {
        let mut headers = HeaderMap::new();

        // IP 伪装
        let ip = options.real_ip.clone().or_else(|| {
            if options.random_cn_ip {
                Some(generate_random_chinese_ip())
            } else {
                None
            }
        });

        if let Some(ref ip) = ip {
            if let (Ok(real_ip), Ok(fwd)) = (HeaderValue::from_str(ip), HeaderValue::from_str(ip)) {
                headers.insert("X-Real-IP", real_ip);
                headers.insert("X-Forwarded-For", fwd);
            }
        }

        // Cookie 处理
        let cookie_str = options
            .cookie
            .as_deref()
            .or(self.cookie.as_deref())
            .unwrap_or("");
        let mut cookie_map = cookie_to_json(cookie_str);

        // 注入必要的 cookie 字段
        let ntes_nuid = random_hex(16);
        let os = get_os_config(cookie_map.get("os").map(|s| s.as_str()).unwrap_or("pc"));
        let now_ts = chrono::Utc::now().timestamp_millis().to_string();

        cookie_map
            .entry("__remember_me".to_string())
            .or_insert_with(|| "true".to_string());
        cookie_map
            .entry("ntes_kaola_ad".to_string())
            .or_insert_with(|| "1".to_string());
        cookie_map
            .entry("_ntes_nnid".to_string())
            .or_insert_with(|| format!("{},{}", ntes_nuid, now_ts));
        cookie_map
            .entry("_ntes_nuid".to_string())
            .or_insert(ntes_nuid);
        cookie_map
            .entry("WNMCID".to_string())
            .or_insert_with(|| WNMCID.clone());
        cookie_map
            .entry("WEVNSM".to_string())
            .or_insert_with(|| "1.0.0".to_string());
        cookie_map
            .entry("osver".to_string())
            .or_insert_with(|| os.osver.to_string());
        cookie_map
            .entry("deviceId".to_string())
            .or_insert_with(|| self.get_device_id().to_string());
        cookie_map
            .entry("os".to_string())
            .or_insert_with(|| os.os.to_string());
        cookie_map
            .entry("channel".to_string())
            .or_insert_with(|| os.channel.to_string());
        cookie_map
            .entry("appver".to_string())
            .or_insert_with(|| os.appver.to_string());

        // 非登录接口注入 NMTID
        if !uri.contains("login") {
            cookie_map
                .entry("NMTID".to_string())
                .or_insert_with(|| random_hex(8));
        }

        // 未登录时注入匿名 token
        if !cookie_map.contains_key("MUSIC_U") {
            if let Some(ref token) = self.anonymous_token {
                cookie_map
                    .entry("MUSIC_A".to_string())
                    .or_insert_with(|| token.clone());
            }
        }

        headers.insert(COOKIE, header_value(&cookie_obj_to_string(&cookie_map)));

        // 确定加密方式
        let crypto_type = if options.crypto == CryptoType::Eapi && !ENCRYPT {
            CryptoType::Api
        } else {
            options.crypto.clone()
        };

        let mut data = data;
        let url: String;
        let encrypt_data: HashMap<String, String>;
        let domain = options.domain.as_deref().unwrap_or("");

        let csrf_token = cookie_map.get("__csrf").cloned().unwrap_or_default();

        match crypto_type {
            CryptoType::Weapi => {
                let ref_domain = if domain.is_empty() { DOMAIN } else { domain };
                headers.insert(REFERER, header_value(ref_domain));
                let ua = options
                    .ua
                    .as_deref()
                    .unwrap_or_else(|| choose_user_agent("weapi", "pc"));
                headers.insert(USER_AGENT, header_value(ua));

                data["csrf_token"] = Value::String(csrf_token);

                // 上游：user-agent 之后、加密之前写独立头
                insert_anti_cheat_token(&mut headers, options.check_token);

                encrypt_data = crypto::weapi(&data);
                url = format!("{}/weapi/{}", ref_domain, &uri[5..]);
            }
            CryptoType::Linuxapi => {
                let ua = options
                    .ua
                    .as_deref()
                    .unwrap_or_else(|| choose_user_agent("linuxapi", "linux"));
                headers.insert(USER_AGENT, header_value(ua));

                let ref_domain = if domain.is_empty() { DOMAIN } else { domain };
                let linux_data = serde_json::json!({
                    "method": "POST",
                    "url": format!("{}{}", ref_domain, uri),
                    "params": data,
                });
                encrypt_data = crypto::linuxapi(&linux_data);
                url = format!("{}/api/linux/forward", ref_domain);
            }
            CryptoType::Xeapi => {
                // XEAPI 依赖动态公钥：Node 侧由 generateConfig.js 预先把公钥写入
                // tmpdir/xeapi_public_key，缺失时 request.js 直接抛错；Rust 侧按需拉取
                let public_key_state = self.ensure_xeapi_public_key().await?;

                let now_secs = chrono::Utc::now().timestamp().to_string();
                let device_id = cookie_map.get("deviceId").cloned().unwrap_or_default();

                // Node: cookie.os === 'android' ? cookie.os : 'android'
                let is_android = cookie_map.get("os").map(|os| os.as_str()) == Some("android");
                let xeapi_os = "android";
                let xeapi_appver = if is_android {
                    cookie_map
                        .get("appver")
                        .cloned()
                        .unwrap_or_else(|| "9.1.65".to_string())
                } else {
                    "9.1.65".to_string()
                };
                let xeapi_osver = if is_android {
                    cookie_map
                        .get("osver")
                        .cloned()
                        .unwrap_or_else(|| "16".to_string())
                } else {
                    "16".to_string()
                };
                let xeapi_buildver = cookie_map
                    .get("buildver")
                    .cloned()
                    .unwrap_or_else(|| now_secs[..10].to_string());

                let xeapi_domain = if domain.is_empty() {
                    XEAPI_DOMAIN
                } else {
                    domain
                };
                let sdevice_id = cookie_map
                    .get("sDeviceId")
                    .cloned()
                    .unwrap_or_else(|| device_id.clone());

                headers.insert(
                    USER_AGENT,
                    header_value(
                        options
                            .ua
                            .as_deref()
                            .unwrap_or_else(|| choose_user_agent("api", "android")),
                    ),
                );
                headers.insert("X-Client-Enc-State", HeaderValue::from_static("ENCRYPTED"));
                headers.insert("x-aeapi", HeaderValue::from_static("true"));
                headers.insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/x-www-form-urlencoded;charset=utf-8"),
                );
                headers.insert("x-deviceid", header_value(&device_id));
                headers.insert("x-os", header_value(xeapi_os));
                headers.insert("x-osver", header_value(&xeapi_osver));
                headers.insert("x-appver", header_value(&xeapi_appver));
                headers.insert("x-sdeviceid", header_value(&sdevice_id));
                headers.insert("x-buildver", header_value(&xeapi_buildver));
                if let Some(music_u) = cookie_map.get("MUSIC_U") {
                    headers.insert("x-music-u", header_value(music_u));
                }
                // 上游：xeapi 同样是独立头（写在 Cookie 之前）
                insert_anti_cheat_token(&mut headers, options.check_token);

                let mut xeapi_cookie = cookie_map.clone();
                xeapi_cookie.insert("os".to_string(), xeapi_os.to_string());
                xeapi_cookie.insert("osver".to_string(), xeapi_osver.clone());
                xeapi_cookie.insert("appver".to_string(), xeapi_appver.clone());
                xeapi_cookie.insert("buildver".to_string(), xeapi_buildver.clone());
                xeapi_cookie.insert("deviceId".to_string(), device_id.clone());
                xeapi_cookie.insert("sDeviceId".to_string(), sdevice_id.clone());
                headers.insert(COOKIE, header_value(&cookie_obj_to_string(&xeapi_cookie)));

                // 会话密钥来自上一次响应头，存在则复用（Node 里是模块级全局变量）
                let session = self.xeapi_session.read().await.clone();
                let (session_id, session_key) = match session {
                    Some((id, key)) => (Some(id), Some(key)),
                    None => (None, None),
                };

                let encrypted = crypto::xeapi(
                    uri,
                    &data,
                    &crypto::XeapiEncryptOptions {
                        public_key_state: &public_key_state,
                        session_id: session_id.as_deref(),
                        session_key: session_key.as_deref(),
                        os: xeapi_os,
                        content_type: None,
                        method: None,
                    },
                )
                .map_err(NcmError::Crypto)?;

                encrypt_data = HashMap::from([
                    ("B".to_string(), encrypted.b),
                    ("S".to_string(), encrypted.s),
                    ("R".to_string(), encrypted.r),
                ]);
                url = format!("{}/xeapi/{}", xeapi_domain, &uri[5..]);
            }
            CryptoType::Eapi | CryptoType::Api => {
                // 构造 eapi header cookie
                let now_secs = chrono::Utc::now().timestamp().to_string();
                let request_id = format!(
                    "{}_{:04}",
                    chrono::Utc::now().timestamp_millis(),
                    rand::random::<u16>() % 1000
                );

                let mut header_map: HashMap<String, String> = HashMap::new();
                header_map.insert(
                    "osver".to_string(),
                    cookie_map.get("osver").cloned().unwrap_or_default(),
                );
                header_map.insert(
                    "deviceId".to_string(),
                    cookie_map.get("deviceId").cloned().unwrap_or_default(),
                );
                header_map.insert(
                    "os".to_string(),
                    cookie_map.get("os").cloned().unwrap_or_default(),
                );
                header_map.insert(
                    "appver".to_string(),
                    cookie_map.get("appver").cloned().unwrap_or_default(),
                );
                header_map.insert(
                    "versioncode".to_string(),
                    cookie_map
                        .get("versioncode")
                        .cloned()
                        .unwrap_or_else(|| "140".to_string()),
                );
                header_map.insert(
                    "mobilename".to_string(),
                    cookie_map.get("mobilename").cloned().unwrap_or_default(),
                );
                header_map.insert(
                    "buildver".to_string(),
                    cookie_map
                        .get("buildver")
                        .cloned()
                        .unwrap_or_else(|| now_secs[..10].to_string()),
                );
                header_map.insert(
                    "resolution".to_string(),
                    cookie_map
                        .get("resolution")
                        .cloned()
                        .unwrap_or_else(|| "1920x1080".to_string()),
                );
                header_map.insert("__csrf".to_string(), csrf_token.clone());
                header_map.insert(
                    "channel".to_string(),
                    cookie_map.get("channel").cloned().unwrap_or_default(),
                );
                header_map.insert("requestId".to_string(), request_id);

                if options.check_token {
                    header_map.insert("X-antiCheatToken".to_string(), CHECK_TOKEN.to_string());
                }

                if let Some(music_u) = cookie_map.get("MUSIC_U") {
                    header_map.insert("MUSIC_U".to_string(), music_u.clone());
                }
                if let Some(music_a) = cookie_map.get("MUSIC_A") {
                    header_map.insert("MUSIC_A".to_string(), music_a.clone());
                }

                let header_cookie_str = header_map
                    .iter()
                    .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                    .collect::<Vec<_>>()
                    .join("; ");

                headers.insert(COOKIE, header_value(&header_cookie_str));

                let ua = options
                    .ua
                    .as_deref()
                    .unwrap_or_else(|| choose_user_agent("api", "iphone"));
                headers.insert(USER_AGENT, header_value(ua));

                let api_domain = if domain.is_empty() {
                    API_DOMAIN
                } else {
                    domain
                };

                if crypto_type == CryptoType::Eapi {
                    // 注入 header 和 e_r
                    let header_value = serde_json::to_value(&header_map).unwrap();
                    data["header"] = header_value;

                    let e_r = options.e_r.unwrap_or(ENCRYPT_RESPONSE);
                    data["e_r"] = Value::Bool(e_r);

                    encrypt_data = crypto::eapi(uri, &data);
                    url = format!("{}/eapi/{}", api_domain, &uri[5..]);
                } else {
                    // api 明文
                    encrypt_data = if let Value::Object(map) = &data {
                        map.iter()
                            .map(|(k, v)| {
                                (
                                    k.clone(),
                                    match v {
                                        Value::String(s) => s.clone(),
                                        _ => v.to_string(),
                                    },
                                )
                            })
                            .collect()
                    } else {
                        HashMap::new()
                    };
                    url = format!("{}{}", api_domain, uri);
                }
            }
        }

        // 构造 POST body
        let body = serde_urlencoded::to_string(&encrypt_data)
            .map_err(|e| NcmError::Unknown(e.to_string()))?;

        // 设置 Content-Type（XEAPI 分支已自行写入带 charset 的版本，不覆盖）
        headers
            .entry(CONTENT_TYPE)
            .or_insert(HeaderValue::from_static(
                "application/x-www-form-urlencoded",
            ));

        // 构造请求并发送（按需使用代理）
        let response = if let Some(ref proxy_url) = options.proxy {
            let proxy = reqwest::Proxy::all(proxy_url)
                .map_err(|e| NcmError::Unknown(format!("Invalid proxy URL: {}", e)))?;
            let proxy_client = reqwest::Client::builder()
                .proxy(proxy)
                .build()
                .map_err(NcmError::Http)?;
            proxy_client
                .post(&url)
                .headers(headers)
                .body(body)
                .send()
                .await?
        } else {
            self.client
                .post(&url)
                .headers(headers)
                .body(body)
                .send()
                .await?
        };

        // 处理响应 cookie
        let resp_cookies: Vec<String> = response
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| {
                // 移除 Domain 属性
                DOMAIN_REGEX.replace_all(s, "").to_string()
            })
            .collect();

        // 解析响应体
        let e_r = options.e_r.unwrap_or(false);
        let status_code = response.status().as_u16() as i64;

        // XEAPI 会话密钥由响应头下发，需在消费响应体之前取出
        let xeapi_session_headers = if crypto_type == CryptoType::Xeapi {
            let session_id = response
                .headers()
                .get("x-encr-ssid")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            let session_key = response
                .headers()
                .get("x-encr-sskey")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string);
            match (session_id, session_key) {
                (Some(id), Some(key)) => Some((id, key)),
                _ => None,
            }
        } else {
            None
        };

        let body: Value = if crypto_type == CryptoType::Xeapi {
            let bytes = response.bytes().await?;
            if let Some((session_id, session_key)) = xeapi_session_headers {
                *self.xeapi_session.write().await = Some((session_id, session_key));
            }
            crypto::xeapi_res_decrypt(&bytes).unwrap_or(Value::Null)
        } else if crypto_type == CryptoType::Eapi && e_r {
            let bytes = response.bytes().await?;
            let hex_str = hex::encode_upper(&bytes);
            crypto::eapi_res_decrypt(&hex_str).unwrap_or(Value::Null)
        } else {
            let text = response.text().await?;
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };

        let mut status = body
            .get("code")
            .and_then(|c| {
                c.as_i64()
                    .or_else(|| c.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or(status_code);

        // 特殊状态码视为 200
        if SPECIAL_STATUS_CODES.contains(&status) {
            status = 200;
        }

        // 状态码范围检查
        if !(100..600).contains(&status) {
            status = 400;
        }

        let answer = ApiResponse {
            status,
            body,
            cookie: resp_cookies,
        };

        if status == 200 {
            Ok(answer)
        } else {
            let msg = answer
                .body
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            Err(NcmError::from_api(status, msg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 上游 `util/request.js` 里 weapi / xeapi 分支写的是
    /// `headers['X-antiCheatToken'] = token` —— 独立 HTTP 头。
    /// 这个位置在 `check_token` 透传打通前是死代码，回归时务必锚住。
    #[test]
    fn test_anti_cheat_token_is_standalone_header_only_when_enabled() {
        let mut headers = HeaderMap::new();

        insert_anti_cheat_token(&mut headers, false);
        assert!(
            headers.get("X-antiCheatToken").is_none(),
            "未开启 checkToken 时不应带独立头"
        );

        insert_anti_cheat_token(&mut headers, true);
        assert_eq!(
            headers.get("X-antiCheatToken").unwrap().to_str().unwrap(),
            CHECK_TOKEN,
            "应写入静态 CHECK_TOKEN"
        );
        // 独立头不能顺手污染 Cookie；往 Cookie 里塞是 eapi / api 分支的走法
        assert!(
            headers.get(COOKIE).is_none(),
            "weapi / xeapi 的 token 不应落到 Cookie 头"
        );
    }
}
