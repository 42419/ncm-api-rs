//! 构建脚本 - 自动从 src/api/mod.rs 扫描模块声明，生成路由注册代码
//!
//! 类似 Node.js 版自动扫描 module/ 目录注册路由

use std::fs;
use std::io::Write;
use std::path::Path;

/// 路由覆盖表：Rust 模块名 → 上游 `server.js` 实际注册的 HTTP 路由。
///
/// 上游 `server.js` 的默认规则是
/// `'/' + fileName.replace(/\.js$/i, '').replace(/_/g, '/')`——把文件名里
/// **所有**下划线换成斜杠，且不做任何命名风格转换。Rust 侧模块名是下划线
/// 风格，绝大多数情况下两者能对上（`song_url_v1` → `/song/url/v1`），
/// 但下面两类对不上，必须显式覆盖：
///
/// 1. `server.js` 里 `special` 声明的模块，路由就是 `/<模块名>`，不替换
///    下划线。上游只有这三项，别往里加别的——`personal_fm_mode.js` 不在
///    `special` 里，它的路由是 `/personal/fm/mode`。
/// 2. 上游文件名带驼峰，路由里原样保留大小写（`digitalAlbum_detail.js` →
///    `/digitalAlbum/detail`）。模块名推导不出大小写，只能靠本表；漏掉会
///    让前端按上游路由调用时 404。
const ROUTE_OVERRIDES: &[(&str, &str)] = &[
    // —— server.js 的 special：路由不替换下划线 ——
    ("daily_signin", "/daily_signin"),
    ("fm_trash", "/fm_trash"),
    ("personal_fm", "/personal_fm"),
    // —— 上游文件名为驼峰，路由保留驼峰原名 ——
    ("digital_album_detail", "/digitalAlbum/detail"),
    ("digital_album_ordering", "/digitalAlbum/ordering"),
    ("digital_album_purchased", "/digitalAlbum/purchased"),
    ("digital_album_sales", "/digitalAlbum/sales"),
    ("dj_radio_top", "/djRadio/top"),
    ("verify_get_qr", "/verify/getQr"),
];

/// 需要特殊处理的路由，不走通用宏
/// - 上传路由：`avatar_upload` / `voice_upload` / `cloud` /
///   `playlist_cover_update` 都需要 multipart 处理，签名与通用宏不符
///   （见 `src/server/upload.rs`）
/// - 非标准签名：`eapi_decrypt`（同步方法）、`inner_version`（无 query 参数）
const EXCLUDED_MODULES: &[&str] = &[
    "avatar_upload",
    "voice_upload",
    "cloud",
    "playlist_cover_update",
    "eapi_decrypt",
    "inner_version",
];

fn method_to_route(method: &str) -> String {
    match ROUTE_OVERRIDES.iter().find(|(name, _)| *name == method) {
        Some((_, route)) => (*route).to_string(),
        None => format!("/{}", method.replace('_', "/")),
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src/api/mod.rs");

    let mod_rs = fs::read_to_string("src/api/mod.rs").expect("Failed to read src/api/mod.rs");

    // 提取所有 `mod xxx;` 声明
    let mut methods: Vec<String> = Vec::new();
    for line in mod_rs.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("mod ") {
            if let Some(name) = rest.strip_suffix(';') {
                let name = name.trim();
                if !EXCLUDED_MODULES.contains(&name) {
                    methods.push(name.to_string());
                }
            }
        }
    }

    // 覆盖表里的模块名必须真实存在，否则只是静默失效（打错字、或该模块被
    // EXCLUDED_MODULES 挡掉），路由会悄悄退回默认规则，很难发现。
    for (name, _) in ROUTE_OVERRIDES {
        assert!(
            methods.iter().any(|m| m == name),
            "ROUTE_OVERRIDES 中的 `{name}` 未出现在 src/api/mod.rs 的可注册模块里（拼写错误？被 EXCLUDED_MODULES 排除了？）"
        );
    }

    // 生成路由注册代码
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("api_routes_generated.rs");
    let mut f = fs::File::create(&dest_path).expect("Failed to create generated file");

    writeln!(f, "// 自动生成的路由注册代码 - 请勿手动修改").unwrap();
    writeln!(f, "// 由 build.rs 从 src/api/mod.rs 扫描生成").unwrap();
    writeln!(f, "//").unwrap();
    writeln!(f, "// 共 {} 个标准路由", methods.len()).unwrap();
    writeln!(f).unwrap();
    writeln!(f, "api_routes!(router,").unwrap();

    for method in methods.iter() {
        let route = method_to_route(method);
        writeln!(f, "    {} => \"{}\",", method, route).unwrap();
    }

    writeln!(f, ")").unwrap();
}
