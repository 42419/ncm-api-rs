//! 文件处理辅助 - 对应 Node.js util/fileHelper.js
//!
//! 只移植本仓库用到的部分。上游这些函数都被 `module/cloud*.js`、
//! `module/voice_upload.js` 与 `plugins/songUpload.js` 共用，所以放在 util 层，
//! 避免各模块各写一份导致语义漂移（例如「扩展名是否小写化」在两处就不一样）。

use crate::crypto::strip_js_whitespace;

/// 对应上游 `util/fileHelper.js` 的 `getFileExtension`：
/// 取最后一个点之后的部分并**小写化**，没有扩展名时回落到 `"mp3"`。
///
/// 注意 `module/cloud_upload_token.js` 里另有一段内联的取扩展名逻辑
/// （见 [`cloud_token_ext`]），那个是**不**小写化的，两者不要混用。
pub fn get_file_extension(filename: &str) -> String {
    if filename.is_empty() {
        return "mp3".to_string();
    }
    if filename.contains('.') {
        return filename.rsplit('.').next().unwrap_or("mp3").to_lowercase();
    }
    "mp3".to_string()
}

/// 对应上游 `module/cloud_upload_token.js` 的
/// `filename.includes('.') ? filename.split('.').pop() : 'mp3'`。
///
/// 与 [`get_file_extension`] 的差别是**不**做小写化，且没有「空串 → mp3」兜底
/// （空串里没有点，同样落到 `"mp3"`，但 `"a."` 会得到空串）。
pub fn cloud_token_ext(filename: &str) -> String {
    match filename.rfind('.') {
        Some(idx) => filename[idx + 1..].to_string(),
        None => "mp3".to_string(),
    }
}

/// 对应上游 `/\.[^.]+$/`：去掉最后一个点及其后的全部非点字符。
///
/// 要求点后**至少有一个**非点字符，所以 `"a."`、`"a.b."` 这类以点结尾的名字
/// 不会被裁剪（`replace` 找不到匹配就原样返回）。
pub fn strip_last_extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(idx) if idx + 1 < name.len() && !name[idx + 1..].contains('.') => &name[..idx],
        _ => name,
    }
}

/// 对应上游 `util/fileHelper.js` 的 `sanitizeFilename`：
/// 去掉扩展名 → 剥离空白 → 点换成下划线。空输入返回 `"unknown"`。
pub fn sanitize_filename(filename: &str) -> String {
    if filename.is_empty() {
        return "unknown".to_string();
    }
    sanitize_filename_inline(filename)
}

/// 对应上游 `module/cloud_upload_token.js` 里内联的
/// `filename.replace(/\.[^.]+$/,'').replace(/\s/g,'').replace(/\./g,'_')`。
///
/// 与 [`sanitize_filename`] 是同一条表达式，唯一区别是没有 `"unknown"` 兜底
/// （上游 `sanitizeFilename` 开头有 `if (!filename) return 'unknown'`）。
pub fn sanitize_filename_inline(filename: &str) -> String {
    strip_js_whitespace(strip_last_extension(filename)).replace('.', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 期望值由上游 `util/fileHelper.js` 的同名函数在 Node 22 上跑出
    #[test]
    fn test_get_file_extension_matches_upstream() {
        let cases = [
            ("song.mp3", "mp3"),
            // 大写扩展名会被小写化
            ("Song.MP3", "mp3"),
            ("a.b.mp3", "mp3"),
            ("a.b.c", "c"),
            // 以点结尾时扩展名是空串
            ("a.", ""),
            ("a.b.", ""),
            (".hidden", "hidden"),
            ("noext", "mp3"),
            ("", "mp3"),
        ];
        for (input, expected) in cases {
            assert_eq!(get_file_extension(input), expected, "输入 {input:?}");
        }
    }

    /// 期望值由上游 `cloud_upload_token.js` 的
    /// `filename.includes('.') ? filename.split('.').pop() : 'mp3'` 跑出
    #[test]
    fn test_cloud_token_ext_keeps_case() {
        let cases = [
            ("song.mp3", "mp3"),
            // 这里不小写化
            ("Song.MP3", "MP3"),
            ("a.b.mp3", "mp3"),
            ("a.b.c", "c"),
            ("a.", ""),
            ("a.b.", ""),
            (".hidden", "hidden"),
            ("noext", "mp3"),
            ("", "mp3"),
        ];
        for (input, expected) in cases {
            assert_eq!(cloud_token_ext(input), expected, "输入 {input:?}");
        }
    }

    /// 期望值由上游 `cloud_upload_token.js` 内联表达式在 Node 22 上跑出
    #[test]
    fn test_sanitize_filename_inline_matches_upstream() {
        let cases = [
            ("song.mp3", "song"),
            ("Song.MP3", "Song"),
            ("a.b.mp3", "a_b"),
            ("a.b.c", "a_b"),
            ("a.", "a_"),
            ("a.b.", "a_b_"),
            (".hidden", ""),
            ("noext", "noext"),
            ("with space.mp3", "withspace"),
            ("with\ttab.mp3", "withtab"),
            // 全角空格与 NBSP 都属于 JS 的 \s，`.replace(' ', "")` 会漏掉
            ("full\u{3000}width.mp3", "fullwidth"),
            ("nbsp\u{00a0}here.mp3", "nbsphere"),
            ("multi  dots .mp3", "multidots"),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(
                sanitize_filename_inline(input),
                expected,
                "输入 {input:?} 的处理结果与上游不一致"
            );
        }
    }

    /// `sanitizeFilename` 比内联版多一个空值兜底
    #[test]
    fn test_sanitize_filename_empty_falls_back_to_unknown() {
        assert_eq!(sanitize_filename(""), "unknown");
        assert_eq!(sanitize_filename("song.mp3"), "song");
        assert_eq!(sanitize_filename("a.b.c"), "a_b");
    }
}
