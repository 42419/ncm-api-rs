/// IP 工具 - 随机中国 IP 生成
///
/// 对应上游 `util/index.js` 的 `generateRandomChineseIP()`：
/// 优先从 `data/china_ip_ranges.txt`（CIDR 列表）按段大小加权随机取段，
/// 再在段内均匀取一个地址；文件缺失/解析为空时回退到硬编码前缀的兜底逻辑。
use rand::Rng;
use std::sync::LazyLock;

/// 中国 IP 段列表（CIDR），`data/china_ip_ranges.txt` 与上游字节一致
const CHINA_IP_RANGES_TXT: &str = include_str!("../../data/china_ip_ranges.txt");

/// 一个 CIDR 段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CidrRange {
    /// 段起始地址（含）
    start: u32,
    /// 段结束地址（含）
    end: u32,
    /// 段内地址数量（= end - start + 1）
    count: u64,
}

/// 惰性加载并解析 CIDR 列表；解析为空时返回 None（走兜底）
///
/// 与上游一致：按段大小降序排序（不影响加权分布，只是让命中更早返回）。
static CHINA_RANGES: LazyLock<Option<Vec<CidrRange>>> = LazyLock::new(|| {
    let mut ranges: Vec<CidrRange> = CHINA_IP_RANGES_TXT
        // 文件带 UTF-8 BOM，先剥掉再按行切分
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(parse_cidr)
        .collect();

    if ranges.is_empty() {
        return None;
    }

    ranges.sort_by(|a, b| b.count.cmp(&a.count));
    Some(ranges)
});

/// 解析 `a.b.c.d/prefix` 为 [`CidrRange`]，非法行返回 None
fn parse_cidr(line: &str) -> Option<CidrRange> {
    let (ip, prefix) = line.split_once('/')?;
    let prefix: u32 = prefix.trim().parse().ok()?;
    if prefix > 32 {
        return None;
    }
    let ip_int = ipv4_to_u32(ip.trim())?;

    // prefix == 0 时 mask 为 0（`u32::MAX << 32` 会溢出，单独处理）
    let mask: u32 = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let start = ip_int & mask;
    let end = start | !mask;

    Some(CidrRange {
        start,
        end,
        count: (end - start) as u64 + 1,
    })
}

/// `"1.2.3.4"` → `u32`，非法返回 None
fn ipv4_to_u32(ip: &str) -> Option<u32> {
    let mut octets = [0u8; 4];
    let mut n = 0;
    for part in ip.split('.') {
        if n == 4 {
            return None;
        }
        // 上游走 parseInt，这里要求纯十进制数字，`"01"` 也接受
        octets[n] = part.trim().parse().ok()?;
        n += 1;
    }
    if n != 4 {
        return None;
    }
    Some(u32::from_be_bytes(octets))
}

/// `u32` → `"1.2.3.4"`
fn u32_to_ipv4(value: u32) -> String {
    let [a, b, c, d] = value.to_be_bytes();
    format!("{}.{}.{}.{}", a, b, c, d)
}

/// 生成随机中国 IP 地址
///
/// 对应上游 `generateRandomChineseIP()`：优先用 `china_ip_ranges.txt`，
/// 数据不可用时回退到常见 IP 段前缀。
pub fn generate_random_chinese_ip() -> String {
    let mut rng = rand::thread_rng();

    if let Some(ranges) = CHINA_RANGES.as_ref() {
        let total: u64 = ranges.iter().map(|r| r.count).sum();
        if total > 0 {
            // 全局随机偏移，落在哪个段就取哪个（等价上游的加权选择）
            let mut offset = rng.gen_range(0..total);
            for range in ranges {
                if offset < range.count {
                    let value = u64::from(range.start) + rng.gen_range(0..range.count);
                    return u32_to_ipv4(value as u32);
                }
                offset -= range.count;
            }
            // 理论上不可达；兜底取最后一段
            let last = ranges[ranges.len() - 1];
            let value = u64::from(last.start) + rng.gen_range(0..last.count);
            return u32_to_ipv4(value as u32);
        }
    }

    fallback_random_chinese_ip()
}

/// 兜底逻辑：常见中国 IP 段前缀（对应上游文件加载失败时的旧实现）
fn fallback_random_chinese_ip() -> String {
    let mut rng = rand::thread_rng();

    // 常见中国 IP 段前缀
    let prefixes: &[(u8, u8, u8)] = &[
        (116, 25, 94),
        (112, 0, 255),
        (114, 0, 255),
        (119, 0, 255),
        (120, 0, 255),
        (121, 0, 255),
        (122, 0, 255),
        (123, 0, 255),
        (124, 0, 255),
        (125, 0, 255),
        (182, 0, 255),
        (183, 0, 255),
        (211, 0, 255),
        (218, 0, 255),
        (220, 0, 255),
        (221, 0, 255),
        (222, 0, 255),
        (223, 0, 255),
    ];

    let (first, min_second, max_second) = prefixes[rng.gen_range(0..prefixes.len())];
    let second = rng.gen_range(min_second..=max_second);
    let third: u8 = rng.gen_range(1..=255);
    let fourth: u8 = rng.gen_range(1..=254);

    format!("{}.{}.{}.{}", first, second, third, fourth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cidr_basic() {
        // 1.1.8.0/24 -> 1.1.8.0 ~ 1.1.8.255
        let range = parse_cidr("1.1.8.0/24").unwrap();
        assert_eq!(range.start, ipv4_to_u32("1.1.8.0").unwrap());
        assert_eq!(range.end, ipv4_to_u32("1.1.8.255").unwrap());
        assert_eq!(range.count, 256);
    }

    #[test]
    fn test_parse_cidr_edges() {
        // /32 只含一个地址
        let host = parse_cidr("8.8.8.8/32").unwrap();
        assert_eq!(host.start, host.end);
        assert_eq!(host.count, 1);

        // /0 覆盖整个地址空间（`u32::MAX << 32` 会溢出，必须特判）
        let all = parse_cidr("0.0.0.0/0").unwrap();
        assert_eq!(all.start, 0);
        assert_eq!(all.end, u32::MAX);
        assert_eq!(all.count, 1u64 << 32);

        // /13 这种大段
        let big = parse_cidr("1.24.0.0/13").unwrap();
        assert_eq!(big.count, 1 << (32 - 13));
    }

    #[test]
    fn test_parse_cidr_invalid() {
        assert!(parse_cidr("1.1.8.0").is_none()); // 缺 prefix
        assert!(parse_cidr("1.1.8.0/33").is_none()); // prefix 越界
        assert!(parse_cidr("1.1.8/24").is_none()); // 少一段
        assert!(parse_cidr("1.1.8.0.0/24").is_none()); // 多一段
        assert!(parse_cidr("a.b.c.d/24").is_none()); // 非数字
        assert!(parse_cidr("1.1.8.256/24").is_none()); // 越界的八位组
    }

    #[test]
    fn test_ranges_loaded() {
        let ranges = CHINA_RANGES.as_ref().expect("应能加载 china_ip_ranges.txt");
        assert_eq!(ranges.len(), 4147, "行数与文件一致（无注释/空行）");
        // 已按段大小降序
        assert!(ranges.windows(2).all(|w| w[0].count >= w[1].count));
    }

    #[test]
    fn test_generated_ip_is_valid_and_domestic() {
        let ranges = CHINA_RANGES.as_ref().unwrap();
        for _ in 0..500 {
            let ip = generate_random_chinese_ip();
            let value = ipv4_to_u32(&ip).expect("必须是合法 IPv4");
            assert!(
                ranges.iter().any(|r| value >= r.start && value <= r.end),
                "生成的 {} 不在任何中国 IP 段内",
                ip
            );
        }
    }

    #[test]
    fn test_fallback_shape() {
        for _ in 0..200 {
            let ip = fallback_random_chinese_ip();
            let value = ipv4_to_u32(&ip).expect("必须是合法 IPv4");
            let first = (value >> 24) as u8;
            assert!(
                matches!(
                    first,
                    116 | 112
                        | 114
                        | 119
                        | 120
                        | 121
                        | 122
                        | 123
                        | 124
                        | 125
                        | 182
                        | 183
                        | 211
                        | 218
                        | 220
                        | 221
                        | 222
                        | 223
                ),
                "兜底前缀不符合预期: {}",
                ip
            );
        }
    }
}
