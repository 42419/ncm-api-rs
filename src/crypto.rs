/// 加密模块 - 对应 Node.js 版本的 util/crypto.js
///
/// 实现三种加密方式：
/// - weapi: 双层 AES-128-CBC + RSA
/// - eapi: MD5 签名 + AES-128-ECB
/// - linuxapi: AES-128-ECB
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyInit, KeyIvInit};
use aes_gcm::aead::Aead;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use rand::{Rng, RngCore};
use rsa::traits::PublicKeyParts;
use rsa::{BigUint, RsaPublicKey};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::LazyLock;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128EcbEnc = ecb::Encryptor<aes::Aes128>;
type Aes128EcbDec = ecb::Decryptor<aes::Aes128>;

const IV: &[u8] = b"0102030405060708";
const PRESET_KEY: &[u8] = b"0CoJUm6Qyw8W8jud";
pub const LINUXAPI_KEY: &[u8] = b"rFgB&h#%2?^eDg:Q";
pub const EAPI_KEY: &[u8] = b"e82ckenh8dichen8";
const BASE62: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

// RSA 公钥 Base64 编码的 DER 数据（PKCS#8 / SubjectPublicKeyInfo 格式）
const PUBLIC_KEY_DER_B64: &str = "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDgtQn2JZ34ZC28NWYpAUd98iZ37BUrX/aKzmFbt7clFSs6sXqHauqKWqdtLkF2KexO40H1YTX8z2lSgBBOAxLsvaklV8k4cBFK9snQXE9/DDaFt6Rr7iVZMldczhC0JNgTz+SHXT6CBHuX3e9SdB1Ua44oncaTWz7OBGLbCiK45wIDAQAB";

/// AES-128-CBC 加密，输出 Base64
fn aes_cbc_encrypt_base64(plaintext: &[u8], key: &[u8], iv: &[u8]) -> String {
    let cipher = Aes128CbcEnc::new(key.into(), iv.into());
    let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &ciphertext)
}

/// AES-128-ECB 加密，输出大写 Hex
fn aes_ecb_encrypt_hex(plaintext: &[u8], key: &[u8]) -> String {
    let cipher = Aes128EcbEnc::new(key.into());
    let ciphertext = cipher.encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    hex::encode_upper(&ciphertext)
}

/// AES-128-ECB 解密（输入大写 Hex）
fn aes_ecb_decrypt_hex(ciphertext_hex: &str, key: &[u8]) -> Result<Vec<u8>, String> {
    let ciphertext = hex::decode(ciphertext_hex).map_err(|e| e.to_string())?;
    let cipher = Aes128EcbDec::new(key.into());
    cipher
        .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
        .map_err(|e| e.to_string())
}

/// RSA 加密（NONE / raw / textbook RSA，无 padding）
/// 对应 Node.js: forge.publicKey.encrypt(str, 'NONE')
fn rsa_encrypt_no_padding(plaintext: &[u8]) -> String {
    use base64::Engine;
    use rsa::pkcs8::DecodePublicKey;

    let der_bytes = base64::engine::general_purpose::STANDARD
        .decode(PUBLIC_KEY_DER_B64)
        .expect("Failed to decode RSA public key base64");

    let public_key =
        RsaPublicKey::from_public_key_der(&der_bytes).expect("Failed to parse RSA public key DER");

    let n = public_key.n().clone();
    let e = public_key.e().clone();

    // Textbook RSA: c = m^e mod n
    let m = BigUint::from_bytes_be(plaintext);
    let c = m.modpow(&e, &n);

    // 输出固定长度 hex（与模数等长，256 hex chars for 1024-bit key）
    let n_bytes = n.bits() / 8;
    let c_bytes = c.to_bytes_be();

    // 左侧填充 0
    let mut padded = vec![0u8; n_bytes - c_bytes.len()];
    padded.extend_from_slice(&c_bytes);
    hex::encode(&padded)
}

/// weapi 加密
/// 双层 AES-128-CBC + RSA 加密随机密钥
pub fn weapi(object: &serde_json::Value) -> HashMap<String, String> {
    let text = serde_json::to_string(object).unwrap();
    let mut rng = rand::thread_rng();

    // 生成 16 位随机密钥
    let secret_key: String = (0..16)
        .map(|_| BASE62[rng.gen_range(0..62)] as char)
        .collect();

    // 第一层 AES-CBC：preset_key + iv
    let first_encrypt = aes_cbc_encrypt_base64(text.as_bytes(), PRESET_KEY, IV);
    // 第二层 AES-CBC：secret_key + iv
    let params = aes_cbc_encrypt_base64(first_encrypt.as_bytes(), secret_key.as_bytes(), IV);

    // RSA 加密反转后的 secret_key
    let reversed_key: String = secret_key.chars().rev().collect();
    let enc_sec_key = rsa_encrypt_no_padding(reversed_key.as_bytes());

    let mut result = HashMap::new();
    result.insert("params".to_string(), params);
    result.insert("encSecKey".to_string(), enc_sec_key);
    result
}

/// linuxapi 加密
/// 单层 AES-128-ECB
pub fn linuxapi(object: &serde_json::Value) -> HashMap<String, String> {
    let text = serde_json::to_string(object).unwrap();
    let mut result = HashMap::new();
    result.insert(
        "eparams".to_string(),
        aes_ecb_encrypt_hex(text.as_bytes(), LINUXAPI_KEY),
    );
    result
}

/// eapi 加密
/// MD5 签名 + AES-128-ECB
pub fn eapi(url: &str, object: &serde_json::Value) -> HashMap<String, String> {
    let text = serde_json::to_string(object).unwrap();
    let message = format!("nobody{}use{}md5forencrypt", url, text);
    let digest = format!("{:x}", Md5::digest(message.as_bytes()));
    let data = format!("{}-36cd479b6b5-{}-36cd479b6b5-{}", url, text, digest);

    let mut result = HashMap::new();
    result.insert(
        "params".to_string(),
        aes_ecb_encrypt_hex(data.as_bytes(), EAPI_KEY),
    );
    result
}

/// eapi 响应解密
pub fn eapi_res_decrypt(encrypted_hex: &str) -> Option<serde_json::Value> {
    let decrypted = aes_ecb_decrypt_hex(encrypted_hex, EAPI_KEY).ok()?;
    let text = String::from_utf8(decrypted).ok()?;
    serde_json::from_str(&text).ok()
}

/// eapi 请求解密（调试用）
pub fn eapi_req_decrypt(encrypted_hex: &str) -> Option<(String, serde_json::Value)> {
    let decrypted = aes_ecb_decrypt_hex(encrypted_hex, EAPI_KEY).ok()?;
    let text = String::from_utf8(decrypted).ok()?;

    // 按 "-36cd479b6b5-" 分隔符拆分
    let parts: Vec<&str> = text.splitn(3, "-36cd479b6b5-").collect();
    if parts.len() >= 2 {
        let url = parts[0].to_string();
        let data: serde_json::Value = serde_json::from_str(parts[1]).ok()?;
        Some((url, data))
    } else {
        None
    }
}

// ============================================================
//  XEAPI —— 对应 Node.js util/crypto.js 中的 xeapi 相关实现
// ============================================================

/// XEAPI 静态密钥（32 字节 → AES-256-ECB）
static XEAPI_STATIC_KEY: LazyLock<Vec<u8>> = LazyLock::new(|| {
    hex::decode("ab1d5a430f6bb04a3f01e81ddd72bd916d5ce591248ac128714806d7f8fb1b84")
        .expect("XEAPI_STATIC_KEY 应为合法十六进制")
});

/// XEAPI 签名密钥
///
/// 注意：Node 侧 `crypto.createHmac('sha256', <string>)` 会把字符串按 UTF-8 解释，
/// 因此这里同样使用它的 UTF-8 字节，而不是先做 base64 解码。
const XEAPI_SIGN_KEY: &str =
    "mUHCwVNWJbunMqAHf5MImuirT6plvs6VSFW62MGHstFQxhBGdEoIhLItH3djc4+FB/OKty3+lL2rGeoFBpVe5g==";

/// base64 编码（标准字母表）
pub fn b64_encode(data: &[u8]) -> String {
    base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data)
}

/// base64 解码（标准字母表）
pub fn b64_decode(data: &str) -> Result<Vec<u8>, String> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
        .map_err(|e| e.to_string())
}

/// AES-ECB 加密，输出原始字节；密钥长度决定 AES-128/192/256
pub fn aes_ecb_encrypt_raw(key: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    match key.len() {
        16 => Ok(ecb::Encryptor::<aes::Aes128>::new(key.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext)),
        24 => Ok(ecb::Encryptor::<aes::Aes192>::new(key.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext)),
        32 => Ok(ecb::Encryptor::<aes::Aes256>::new(key.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext)),
        n => Err(format!("不支持的 AES 密钥长度: {}", n)),
    }
}

/// AES-ECB 解密，输入原始字节；密钥长度决定 AES-128/192/256
pub fn aes_ecb_decrypt_raw(key: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    match key.len() {
        16 => ecb::Decryptor::<aes::Aes128>::new(key.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|e| e.to_string()),
        24 => ecb::Decryptor::<aes::Aes192>::new(key.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|e| e.to_string()),
        32 => ecb::Decryptor::<aes::Aes256>::new(key.into())
            .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
            .map_err(|e| e.to_string()),
        n => Err(format!("不支持的 AES 密钥长度: {}", n)),
    }
}

/// AES-ECB 解密（输入十六进制字符串），供通用解密接口使用
pub fn aes_ecb_decrypt_hex_raw(ciphertext_hex: &str, key: &[u8]) -> Result<Vec<u8>, String> {
    let ciphertext = hex::decode(ciphertext_hex).map_err(|e| e.to_string())?;
    aes_ecb_decrypt_raw(key, &ciphertext)
}

/// HMAC-SHA256
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC 接受任意长度密钥");
    mac.update(message);
    mac.finalize().into_bytes().to_vec()
}

/// X25519 公钥形态的 XEAPI 公钥状态（对应 Node 的 xeapi_public_key 文件内容）
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct XeapiPublicKeyState {
    /// base64 编码的 32 字节裸 X25519 公钥
    #[serde(rename = "publicKey", default)]
    pub public_key: String,
    /// 密钥版本
    #[serde(default)]
    pub version: String,
    /// 会话密钥（服务端下发的 sk）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sk: Option<String>,
}

/// XEAPI 加密结果（对应请求体里的 B / S / R 三个字段）
#[derive(Debug, Clone)]
pub struct XeapiEncrypted {
    pub b: String,
    pub s: String,
    pub r: String,
}

/// XEAPI 加密入参
#[derive(Debug, Clone)]
pub struct XeapiEncryptOptions<'a> {
    pub public_key_state: &'a XeapiPublicKeyState,
    /// 响应头 `x-encr-ssid` 缓存下来的会话 id
    pub session_id: Option<&'a str>,
    /// 响应头 `x-encr-sskey` 缓存下来的会话密钥
    pub session_key: Option<&'a str>,
    pub os: &'a str,
    pub content_type: Option<&'a str>,
    pub method: Option<&'a str>,
}

/// XEAPI 签名：HMAC-SHA256(timestamp + nonce)，输出 base64
pub fn xeapi_sign(timestamp: &str, nonce: &str) -> String {
    let mac = hmac_sha256(
        XEAPI_SIGN_KEY.as_bytes(),
        format!("{}{}", timestamp, nonce).as_bytes(),
    );
    b64_encode(&mac)
}

/// 把 JS 的 `String(value)` 语义映射为字符串（用于 `new URLSearchParams(object)`）
pub fn js_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Number(_) => value.to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(js_value_to_string)
            .collect::<Vec<_>>()
            .join(","),
        serde_json::Value::Object(_) => "[object Object]".to_string(),
    }
}

/// `application/x-www-form-urlencoded` 序列化，与 JS `URLSearchParams` 保持一致：
/// 仅保留 `A-Za-z0-9*-._`，空格转 `+`，其余按 UTF-8 字节做大写十六进制百分号编码
pub fn form_urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.as_bytes() {
        match *byte {
            b' ' => out.push('+'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{:02X}", other)),
        }
    }
    out
}

/// 构造 XEAPI 请求明文（对应 Node 的 `buildXeapiPlaintext`）
///
/// 字段顺序与 Node 完全一致：`contentType?`、`method?`、`queryString`/`body`、
/// 最后追加或补全 `queryString`（含 `e_r=true`）。
pub fn build_xeapi_plaintext(
    uri: &str,
    data: &serde_json::Value,
    content_type: Option<&str>,
    method: Option<&str>,
) -> String {
    const DEFAULT_CONTENT_TYPE: &str = "application/x-www-form-urlencoded;charset=utf-8";

    let content_type = content_type.unwrap_or(DEFAULT_CONTENT_TYPE);
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let method = method.unwrap_or("POST").to_uppercase();

    let mut fields: Vec<(String, String)> = Vec::new();

    if media_type != "application/x-www-form-urlencoded" {
        fields.push(("contentType".to_string(), content_type.to_string()));
    }
    if method != "POST" {
        fields.push(("method".to_string(), method));
    }
    if let Some((_, query)) = uri.split_once('?') {
        fields.push(("queryString".to_string(), query.to_string()));
    }
    if !data.is_null() {
        if let Some(object) = data.as_object() {
            let encoded = object
                .iter()
                .filter(|(key, _)| key.as_str() != "e_r")
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        form_urlencode(key),
                        form_urlencode(&js_value_to_string(value))
                    )
                })
                .collect::<Vec<_>>()
                .join("&");
            fields.push(("body".to_string(), b64_encode(encoded.as_bytes())));
        }
    }

    let query_index = fields.iter().position(|(key, _)| key == "queryString");
    match query_index {
        Some(index) => fields[index].1.push_str("&e_r=true"),
        None => fields.push(("queryString".to_string(), "e_r=true".to_string())),
    }

    // 保持插入顺序：serde_json 默认的 Map 会按 key 排序，这里手动拼装
    let mut out = String::from("{");
    for (index, (key, value)) in fields.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(key).unwrap_or_default());
        out.push(':');
        out.push_str(&serde_json::to_string(value).unwrap_or_default());
    }
    out.push('}');
    out
}

/// 中间层变换（对应 Node 的 `xeapiMidTransform`）
pub fn xeapi_mid_transform(ciphertext: &[u8], random: &[u8; 16]) -> Vec<u8> {
    let xored: Vec<u8> = ciphertext
        .iter()
        .enumerate()
        .map(|(index, byte)| byte ^ random[index & 0x0f])
        .collect();

    let b64 = b64_encode(&xored).into_bytes();
    let rotation = if b64.is_empty() {
        0
    } else {
        (random[0] & 0x0f) as usize % b64.len()
    };

    let mut out = Vec::with_capacity(16 + b64.len());
    out.extend_from_slice(random);
    out.extend_from_slice(&b64[rotation..]);
    out.extend_from_slice(&b64[..rotation]);
    out
}

/// HKDF 风格的密钥派生（对应 Node 的 `deriveX25519AesKey`）
fn derive_x25519_aes_key(shared_secret: &[u8], ephemeral_public_key: &[u8]) -> Vec<u8> {
    let prk = hmac_sha256(&[0u8; 32], shared_secret);
    let mut message = ephemeral_public_key.to_vec();
    message.push(1);
    let okm = hmac_sha256(&prk, &message);
    okm[..16].to_vec()
}

/// 构造 S 段：X25519 ECDH + AES-128-GCM（对应 Node 的 `xeapiEncryptS`）
pub fn xeapi_encrypt_s(
    dynamic_key: &[u8],
    public_key_state: &XeapiPublicKeyState,
    os: &str,
    ephemeral_secret: &[u8; 32],
    iv: &[u8; 12],
) -> Result<Vec<u8>, String> {
    let peer_raw = b64_decode(&public_key_state.public_key)?;
    if peer_raw.len() != 32 {
        return Err(format!(
            "XEAPI 对端公钥长度应为 32 字节，实际 {}",
            peer_raw.len()
        ));
    }
    let mut peer_bytes = [0u8; 32];
    peer_bytes.copy_from_slice(&peer_raw);
    let peer_public = X25519PublicKey::from(peer_bytes);

    let secret = StaticSecret::from(*ephemeral_secret);
    let ephemeral_public = X25519PublicKey::from(&secret);
    let ephemeral_raw = ephemeral_public.as_bytes();

    let shared_secret = secret.diffie_hellman(&peer_public);
    let aes_key = derive_x25519_aes_key(shared_secret.as_bytes(), ephemeral_raw);

    let plaintext = format!(
        "{}|{}|{}",
        b64_encode(dynamic_key),
        os,
        public_key_state.sk.as_deref().unwrap_or("")
    );

    let cipher = aes_gcm::Aes128Gcm::new_from_slice(&aes_key).map_err(|e| e.to_string())?;
    let nonce = aes_gcm::Nonce::from_slice(iv);
    let encrypted = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(32 + 12 + encrypted.len());
    out.extend_from_slice(ephemeral_raw);
    out.extend_from_slice(iv);
    out.extend_from_slice(&encrypted);
    Ok(out)
}

/// XEAPI 加密，使用系统随机源
pub fn xeapi(
    uri: &str,
    data: &serde_json::Value,
    options: &XeapiEncryptOptions,
) -> Result<XeapiEncrypted, String> {
    let mut rng = rand::thread_rng();

    let dynamic_key = match options.session_key {
        Some(key) => key.as_bytes().to_vec(),
        None => {
            let mut key = [0u8; 16];
            rng.fill_bytes(&mut key);
            key.to_vec()
        }
    };

    let mut mid_random = [0u8; 16];
    rng.fill_bytes(&mut mid_random);
    let mut ephemeral_secret = [0u8; 32];
    rng.fill_bytes(&mut ephemeral_secret);
    let mut iv = [0u8; 12];
    rng.fill_bytes(&mut iv);

    xeapi_with_randomness(
        uri,
        data,
        options,
        &dynamic_key,
        &mid_random,
        &ephemeral_secret,
        &iv,
    )
}

/// XEAPI 加密（显式注入随机量，便于确定性测试）
#[allow(clippy::too_many_arguments)]
pub fn xeapi_with_randomness(
    uri: &str,
    data: &serde_json::Value,
    options: &XeapiEncryptOptions,
    dynamic_key: &[u8],
    mid_random: &[u8; 16],
    ephemeral_secret: &[u8; 32],
    iv: &[u8; 12],
) -> Result<XeapiEncrypted, String> {
    let plaintext = build_xeapi_plaintext(uri, data, options.content_type, options.method);

    let inner = aes_ecb_encrypt_raw(&XEAPI_STATIC_KEY, plaintext.as_bytes())?;
    let middle = xeapi_mid_transform(&inner, mid_random);
    let b = aes_ecb_encrypt_raw(dynamic_key, &middle)?;

    let s = xeapi_encrypt_s(
        dynamic_key,
        options.public_key_state,
        options.os,
        ephemeral_secret,
        iv,
    )?;

    let session_part = match options.session_key {
        Some(_) => options.session_id.unwrap_or(""),
        None => "",
    };
    let r_plaintext = format!("{}|{}", options.public_key_state.version, session_part);
    let r = aes_ecb_encrypt_raw(&XEAPI_STATIC_KEY, r_plaintext.as_bytes())?;

    Ok(XeapiEncrypted {
        b: b64_encode(&b),
        s: b64_encode(&s),
        r: b64_encode(&r),
    })
}

/// XEAPI 响应解密：AES-128-ECB（eapi key）+ 可选 gzip
pub fn xeapi_res_decrypt(body: &[u8]) -> Option<serde_json::Value> {
    let decrypted = aes_ecb_decrypt_raw(EAPI_KEY, body).ok()?;
    let plaintext = if decrypted.len() >= 2 && decrypted[0] == 0x1f && decrypted[1] == 0x8b {
        gunzip(&decrypted).ok()?
    } else {
        decrypted
    };
    serde_json::from_slice(&plaintext).ok()
}

/// 解密服务端下发的 XEAPI 公钥（AES-256-ECB，使用静态密钥）
pub fn xeapi_decrypt_public_key(encrypted_data: &str) -> Option<XeapiPublicKeyState> {
    let raw = b64_decode(encrypted_data).ok()?;
    let decrypted = aes_ecb_decrypt_raw(&XEAPI_STATIC_KEY, &raw).ok()?;
    serde_json::from_slice(&decrypted).ok()
}

/// 剥离 JS 正则 `/\s/g` 所匹配的全部空白字符
///
/// 与 Rust 的 `char::is_whitespace()`（Unicode `White_Space` 属性）**并不等价**：
/// ECMAScript 的 `\s` 不包含 U+0085（NEL），却包含 U+FEFF（ZWNBSP）。
/// 为保证与 Node 参考实现逐字节一致，这里按 ES2016+ 规范逐字符判定。
pub fn strip_js_whitespace(input: &str) -> String {
    input.chars().filter(|c| !is_js_whitespace(*c)).collect()
}

/// 判定单个字符是否属于 ECMAScript `\s` 集合
fn is_js_whitespace(c: char) -> bool {
    let named = matches!(
        c,
        // WhiteSpace
        '\u{0009}' // TAB
            | '\u{000B}' // VT
            | '\u{000C}' // FF
            | '\u{0020}' // SP
            | '\u{00A0}' // NBSP
            | '\u{FEFF}' // ZWNBSP
            | '\u{1680}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            // LineTerminator
            | '\u{000A}' // LF
            | '\u{000D}' // CR
            | '\u{2028}' // LS
            | '\u{2029}' // PS
    );
    named || ('\u{2000}'..='\u{200A}').contains(&c)
}

/// gzip 解压
fn gunzip(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut decoder = flate2::read::GzDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_ecb_roundtrip() {
        let plaintext = b"hello world test";
        let encrypted = aes_ecb_encrypt_hex(plaintext, EAPI_KEY);
        let decrypted = aes_ecb_decrypt_hex(&encrypted, EAPI_KEY).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_weapi_produces_params_and_encseckey() {
        let obj = serde_json::json!({"id": 123});
        let result = weapi(&obj);
        assert!(result.contains_key("params"));
        assert!(result.contains_key("encSecKey"));
        assert!(!result["params"].is_empty());
        // RSA 输出 256 hex chars (1024-bit key)
        assert_eq!(result["encSecKey"].len(), 256);
    }

    #[test]
    fn test_linuxapi_produces_eparams() {
        let obj = serde_json::json!({"method": "POST", "url": "https://music.163.com/api/test"});
        let result = linuxapi(&obj);
        assert!(result.contains_key("eparams"));
        assert!(!result["eparams"].is_empty());
    }

    #[test]
    fn test_eapi_encrypt_decrypt() {
        let url = "/api/song/detail";
        let obj = serde_json::json!({"id": 123});
        let encrypted = eapi(url, &obj);
        let params = &encrypted["params"];

        // 验证能解密回来
        let (dec_url, dec_data) = eapi_req_decrypt(params).unwrap();
        assert_eq!(dec_url, url);
        assert_eq!(dec_data, obj);
    }

    // ========================================================
    //  XEAPI 测试向量
    //
    //  期望值由上游 api-enhanced v4.34.3 的 util/crypto.js 实现生成，
    //  随机量（dynamicKey / midRandom / iv）与临时私钥均为固定值。
    // ========================================================

    /// 固定随机序列：从 start 起连续递增
    fn seq(start: u8, len: usize) -> Vec<u8> {
        (0..len).map(|i| start.wrapping_add(i as u8)).collect()
    }

    fn seq16(start: u8) -> [u8; 16] {
        let mut out = [0u8; 16];
        out.copy_from_slice(&seq(start, 16));
        out
    }

    fn seq12(start: u8) -> [u8; 12] {
        let mut out = [0u8; 12];
        out.copy_from_slice(&seq(start, 12));
        out
    }

    fn test_public_key_state() -> XeapiPublicKeyState {
        XeapiPublicKeyState {
            public_key: b64_encode(&seq(0x20, 32)),
            version: "1.0.0".to_string(),
            sk: Some("sktest".to_string()),
        }
    }

    #[test]
    fn test_xeapi_sign() {
        assert_eq!(
            xeapi_sign("1700000000000", "0123456789012345"),
            "Z9T+aFZBr0nWVV5WMzIyYWvtSQUx7ysC9WcxILLfs2c="
        );
    }

    #[test]
    fn test_xeapi_mid_transform() {
        let input = hex::decode("00112233445566778899aabbccddeeff0102030405").unwrap();
        let out = xeapi_mid_transform(&input, &seq16(0xa0));
        assert_eq!(
            hex::encode(out),
            "a0a1a2a3a4a5a6a7a8a9aaabacadaeaf6f4c43416b4f4477774e41674d41415159484241554b476a6f616568"
        );
    }

    #[test]
    fn test_xeapi_static_ecb() {
        let ciphertext =
            aes_ecb_encrypt_raw(&XEAPI_STATIC_KEY, br#"{"queryString":"e_r=true"}"#).unwrap();
        assert_eq!(
            hex::encode(ciphertext),
            "d6101c9080885cb7b7562e841c2fe2818f8c56d74c51c756bf17add2cfa4304a"
        );
    }

    #[test]
    fn test_xeapi_build_plaintext_without_query() {
        let data = serde_json::json!({ "type": "0" });
        assert_eq!(
            build_xeapi_plaintext(
                "/api/vipnewcenter/app/minidesk/music/sign/pc",
                &data,
                None,
                None
            ),
            r#"{"body":"dHlwZT0w","queryString":"e_r=true"}"#
        );
    }

    #[test]
    fn test_xeapi_build_plaintext_multi_fields() {
        let data = serde_json::json!({
            "taskType": "app_vip_task_center",
            "userId": "123456",
        });
        assert_eq!(
            build_xeapi_plaintext(
                "/api/middle/vip/mission/user/progress/list",
                &data,
                None,
                None
            ),
            r#"{"body":"dGFza1R5cGU9YXBwX3ZpcF90YXNrX2NlbnRlciZ1c2VySWQ9MTIzNDU2","queryString":"e_r=true"}"#
        );
    }

    #[test]
    fn test_xeapi_build_plaintext_strips_e_r() {
        let data = serde_json::json!({ "type": "0", "e_r": true });
        assert_eq!(
            build_xeapi_plaintext("/api/x", &data, None, None),
            r#"{"body":"dHlwZT0w","queryString":"e_r=true"}"#
        );
    }

    #[test]
    fn test_form_urlencode_matches_urlsearchparams() {
        let input = r#"a b~c&d=e+f/g?h#i%j*k-l.m_n"o'p<q>r,s;t:u\v[w]x{y}z|"#;
        assert_eq!(
            form_urlencode(input),
            "a+b%7Ec%26d%3De%2Bf%2Fg%3Fh%23i%25j*k-l.m_n%22o%27p%3Cq%3Er%2Cs%3Bt%3Au%5Cv%5Bw%5Dx%7By%7Dz%7C"
        );
        // 中文与 emoji 按 UTF-8 字节编码
        assert_eq!(form_urlencode("中文 😀"), "%E4%B8%AD%E6%96%87+%F0%9F%98%80");
    }

    #[test]
    fn test_js_value_to_string() {
        assert_eq!(js_value_to_string(&serde_json::json!(1)), "1");
        assert_eq!(js_value_to_string(&serde_json::json!(1.5)), "1.5");
        assert_eq!(js_value_to_string(&serde_json::json!(true)), "true");
        assert_eq!(js_value_to_string(&serde_json::json!(false)), "false");
        assert_eq!(js_value_to_string(&serde_json::json!(null)), "null");
        assert_eq!(js_value_to_string(&serde_json::json!([1, 2])), "1,2");
        assert_eq!(
            js_value_to_string(&serde_json::json!({"x": 1})),
            "[object Object]"
        );
    }

    #[test]
    fn test_xeapi_case1_without_session() {
        let options = XeapiEncryptOptions {
            public_key_state: &test_public_key_state(),
            session_id: None,
            session_key: None,
            os: "android",
            content_type: None,
            method: None,
        };
        let out = xeapi_with_randomness(
            "/api/vipnewcenter/app/minidesk/music/sign/pc",
            &serde_json::json!({ "type": "0" }),
            &options,
            &seq(0xa0, 16),
            &seq16(0xb0),
            &[0x01u8; 32],
            &seq12(0xc0),
        )
        .unwrap();

        assert_eq!(
            out.b,
            "iyaKxIhgv3YVxxA9mNMA1l1szIjN4pm3UPt8tPpkAtCh8vZsSUjHTqXczsN5o7gD3LMLlYCFjti5nOiphs4zBDCzi9aZPpesnsdVwxNbGTfUnW4R8GEPOJGDbSSzVPyg"
        );
        assert_eq!(
            out.s,
            "pOCSkrZRwni5dyxWn1+puxPZBrRqtoyd+dwrRAn4ognAwcLDxMXGx8jJyssujUurEpWYKJ7BCKgocDzFFjo/BbBKW/Eq1Q61kib//+10a184hu9UHXymhS7SSP5Xh7vqWGG3"
        );
        assert_eq!(out.r, "0kt0zSFXZnu9lUkMzQhZsQ==");
    }

    #[test]
    fn test_xeapi_case2_with_session() {
        let options = XeapiEncryptOptions {
            public_key_state: &test_public_key_state(),
            session_id: Some("SSID-1"),
            session_key: Some("0123456789abcdef"),
            os: "android",
            content_type: None,
            method: None,
        };
        let out = xeapi_with_randomness(
            "/api/middle/vip/mission/user/progress/list",
            &serde_json::json!({
                "taskType": "app_vip_task_center",
                "userId": "123456",
            }),
            &options,
            b"0123456789abcdef",
            &seq16(0xa0),
            &[0x01u8; 32],
            &seq12(0xb0),
        )
        .unwrap();

        assert_eq!(
            out.b,
            "QGhx+TqFShUTkcw7OcSLgEpRy6scUh2ua0AAPS0txzYFAXPS7opLMiVpcLR7mnqSOXf1Qo3nfP3COmWXT4S733ztn1HC22pDD6hGuOaUx6kg6IVqcwxb6c8Akrv0B/qDIvqnIchhz028oZ56bQTS+jAzm1m6p/7Xa6nRaLlO+tRigDNwKvvyOcwwHbWjZrV2N3Ii4GGpJMWRzZwn6hY+1A=="
        );
        assert_eq!(
            out.s,
            "pOCSkrZRwni5dyxWn1+puxPZBrRqtoyd+dwrRAn4ogmwsbKztLW2t7i5urvitSUDtBJ3brpot/AVGhzphASdDed+SG90vpvDeBobMhEUs7l7qHlf5DjjiJzGMBYoPG291OxB"
        );
        assert_eq!(out.r, "QYUTRUkNlauUk8zRoXkW6Q==");
    }

    #[test]
    fn test_xeapi_res_decrypt_plain_and_gzip() {
        let expected = serde_json::json!({
            "code": 200,
            "data": { "ok": true },
            "message": "hi",
        });

        let plain =
            b64_decode("3MUrMBPptmwDj44CflgOzlxkdzNtDHiguWXamTIOmSD3J/IJZyMMLS9iO6YX8zYG").unwrap();
        assert_eq!(xeapi_res_decrypt(&plain).unwrap(), expected);

        let gzipped = b64_decode(
            "UtqDYh+i5LhGQT4EOiuvVc5e/PVk0gpBsrZVb8lQPUfwBSeZgOIPTSaFwRfe4cXETxqjr7/OOF/YX2u2yx2yuZBUrTO8UtxYgU8bUDd4W4A=",
        )
        .unwrap();
        assert_eq!(xeapi_res_decrypt(&gzipped).unwrap(), expected);
    }

    #[test]
    fn test_xeapi_decrypt_public_key() {
        let encrypted = "ZVUbwR/Lx5XO11fPO4jGOjM4TaqgK2ERNKiwdnsTFkK3V47cPYdfDPtNZDSUniKsJPFljOuz2SSYMeG+IvpsbAx2j3E5kBOeTcMFIUyfcNAZpZ8TpfHS8CI+Tmgx8nT1";
        let state = xeapi_decrypt_public_key(encrypted).unwrap();
        assert_eq!(state.public_key, b64_encode(&seq(0x20, 32)));
        assert_eq!(state.version, "1.0.0");
        assert_eq!(state.sk.as_deref(), Some("sktest"));
    }

    // ========================================================
    //  strip_js_whitespace
    //
    //  期望集合由 Node 22 对 U+0000..U+10FFFF 全码点扫描
    //  `/\s/.test(String.fromCodePoint(c))` 得到，共 25 个码点。
    // ========================================================

    #[test]
    fn test_strip_js_whitespace_matches_ecmascript() {
        let js_whitespace: Vec<char> = vec![
            '\u{0009}', '\u{000A}', '\u{000B}', '\u{000C}', '\u{000D}', '\u{0020}', '\u{00A0}',
            '\u{1680}', '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}',
            '\u{2006}', '\u{2007}', '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}',
            '\u{202F}', '\u{205F}', '\u{3000}', '\u{FEFF}',
        ];
        assert_eq!(js_whitespace.len(), 25);

        let mut input = String::from("abc");
        for c in &js_whitespace {
            assert!(is_js_whitespace(*c), "U+{:04X} 应属于 JS \\s", *c as u32);
            input.push(*c);
        }
        input.push_str("def");
        assert_eq!(strip_js_whitespace(&input), "abcdef");

        // JS 的 \s 不含 U+0085（NEL）与 U+180E，而 Rust 的
        // `char::is_whitespace()`（Unicode White_Space）包含 U+0085——
        // 这正是不能直接用 `char::is_whitespace` 的原因。
        assert!(char::is_whitespace('\u{0085}'));
        assert!(!is_js_whitespace('\u{0085}'));
        assert_eq!(strip_js_whitespace("a\u{0085}b"), "a\u{0085}b");
        assert!(!is_js_whitespace('\u{180E}'));
    }
}
