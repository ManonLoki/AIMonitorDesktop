// 图片相关的逻辑：格式探测、GIF 无限循环修正、上传文件名白名单校验、文件探测。
// 除 probe_image_file 走 tokio::fs 异步 I/O 外，其余都是不涉及 HTTP/Tauri 的纯函数，
// 只依赖字节切片，便于独立单元测试。
use std::path::Path;
use tokio::io::AsyncReadExt;

// 通过文件头部的魔数判断图片格式，返回 (扩展名, MIME类型)；不依赖文件名/扩展名，
// 因为上传的文件名是随机生成的 UUID，必须靠内容本身判断类型。
pub(crate) fn detect_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(("png", "image/png")) // PNG 文件签名
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("jpg", "image/jpeg")) // JPEG 文件签名（SOI 标记）
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("gif", "image/gif")) // GIF87a/GIF89a 两种历史版本签名
    } else {
        None // 未识别的格式一律拒绝
    }
}

// 浏览器会遵循 GIF 内嵌的 NETSCAPE/ANIMEXTS 循环次数；不少生成器默认写成 1，
// 导致监控宫格里的动图播完一轮后停住。这里不重新编码帧，只把已有循环次数改为 0
//（GIF 约定的无限循环），或在缺少循环扩展时补上一段标准 NETSCAPE2.0 扩展。
pub(crate) fn make_gif_loop_forever(bytes: &mut Vec<u8>) -> bool {
    if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) || bytes.len() < 13 {
        return false; // 不是 GIF，或短到连逻辑屏幕描述符都放不下，直接放弃
    }

    let packed = bytes[10]; // 逻辑屏幕描述符里的 Packed Fields 字节
    let color_table_len = if packed & 0x80 != 0 {
        // 最高位为 1 表示存在全局颜色表，低 3 位编码颜色表大小（2^(n+1) 个 RGB 三元组）
        3usize << (usize::from(packed & 0x07) + 1)
    } else {
        0 // 没有全局颜色表
    };
    let data_start = 13usize.saturating_add(color_table_len); // 头部固定 13 字节之后紧跟全局颜色表
    if data_start > bytes.len() {
        return false; // 声明的颜色表比实际文件还长，数据损坏
    }

    const NETSCAPE: &[u8; 11] = b"NETSCAPE2.0"; // 主流浏览器/编码器使用的循环扩展标识
    const ANIMEXTS: &[u8; 11] = b"ANIMEXTS1.0"; // 少数工具使用的等价历史标识
    let mut cursor = data_start; // 从颜色表结束处开始扫描
    while cursor + 2 <= bytes.len() {
        match bytes[cursor] {
            0x21 if bytes[cursor + 1] == 0xff => {
                // 0x21 0xFF 是"应用扩展"标记
                let block_size_index = cursor + 2; // 紧跟其后的一个字节是应用标识块长度
                if block_size_index >= bytes.len() {
                    return false; // 数据被截断
                }
                let block_size = usize::from(bytes[block_size_index]); // 标准应用扩展该字段固定为 11
                let identifier_start = block_size_index + 1; // 应用标识符起始位置
                let identifier_end = identifier_start.saturating_add(block_size); // 应用标识符结束位置
                if identifier_end > bytes.len() {
                    return false; // 声明长度超出文件范围
                }
                let identifier = &bytes[identifier_start..identifier_end]; // 取出应用标识符本身
                if block_size == 11 && (identifier == NETSCAPE || identifier == ANIMEXTS) {
                    // 循环子块格式固定为 03 01 <count-le-u16> 00。
                    if identifier_end + 5 <= bytes.len()
                        && bytes[identifier_end] == 3 // 子块长度固定为 3
                        && bytes[identifier_end + 1] == 1 // 子块 ID 固定为 1（循环计数）
                        && bytes[identifier_end + 4] == 0 // 子块列表结尾的终止符
                    {
                        bytes[identifier_end + 2] = 0; // 循环次数低字节改为 0（无限循环）
                        bytes[identifier_end + 3] = 0; // 循环次数高字节改为 0
                        return true; // 已就地修改完成
                    }
                    return false; // 标识符匹配但子块格式不符合预期，放弃修改以免破坏文件
                }
                cursor = identifier_end; // 不是循环扩展，跳过应用标识符继续扫描其后的数据子块
                while cursor < bytes.len() {
                    let size = usize::from(bytes[cursor]); // 数据子块长度前缀
                    cursor += 1;
                    if size == 0 {
                        break; // 长度为 0 表示该扩展的子块列表结束
                    }
                    cursor = cursor.saturating_add(size); // 跳过这个数据子块
                    if cursor > bytes.len() {
                        return false; // 数据被截断
                    }
                }
            }
            // 注释、图形控制、纯文本等其他扩展也由长度前缀子块组成；跳过它们，
            // 避免把扩展正文中恰好出现的 0x2c 误判为第一帧图像标记。
            0x21 => {
                cursor += 2; // 跳过 0x21 与扩展标签字节
                while cursor < bytes.len() {
                    let size = usize::from(bytes[cursor]); // 数据子块长度前缀
                    cursor += 1;
                    if size == 0 {
                        break; // 子块列表结束
                    }
                    cursor = cursor.saturating_add(size); // 跳过该数据子块
                    if cursor > bytes.len() {
                        return false; // 数据被截断
                    }
                }
            }
            // 第一帧图像或文件结束标记之前没有循环扩展，补到全局色表之后。
            0x2c | 0x3b => break, // 0x2c=图像描述符开始，0x3b=文件尾，两者都意味着不会再有扩展块
            // 容忍编码器写入的非标准填充字节，继续查找扩展/图像标记。
            _ => cursor += 1,
        }
    }

    const LOOP_FOREVER_EXTENSION: &[u8] = b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"; // 完整的"无限循环"应用扩展字节序列
    bytes.splice(
        data_start..data_start, // 在颜色表结束处原地插入（不替换任何已有字节）
        LOOP_FOREVER_EXTENSION.iter().copied(),
    );
    true // 已插入新的循环扩展
}

// 只读取文件开头的几个字节来判断类型和 MIME，用 metadata 拿文件大小——
// 避免为了列出图片目录而把每个文件的全部内容读进内存（图片可能有数 MB）。
// 这是 GET /api/images 分页列表接口的关键优化点：无论请求哪一页，都需要遍历
// 全部文件以计算 total，如果每个文件都整体读入内存，目录越大、单张图片越大，
// 这个接口的内存占用和耗时就越不可控。走 tokio::fs 异步 I/O，不阻塞 Tokio 工作线程。
pub(crate) async fn probe_image_file(path: &Path) -> Option<(&'static str, u64)> {
    let mut file = tokio::fs::File::open(path).await.ok()?; // 打开文件，失败则整体返回 None
    let mut header = [0u8; 8]; // 8 字节足够覆盖 PNG/JPEG/GIF 的魔数
    let read = file.read(&mut header).await.ok()?; // 读取实际字节数（文件可能不足 8 字节）
    let (_, mime) = detect_image(&header[..read])?; // 只用魔数判断类型，丢弃扩展名
    let size = file.metadata().await.ok()?.len(); // 走文件系统元数据拿总大小，不读正文
    Some((mime, size))
}

// 上传文件名的白名单校验：禁止路径分隔符（防目录穿越）、限制字符集与长度、
// 要求以已知图片扩展名结尾。用于 DELETE/GET /api/images/{filename} 之前的防御性检查。
pub(crate) fn safe_image_filename(filename: &str) -> bool {
    !filename.is_empty() // 不能为空
        && filename.len() <= 180 // 限制最大长度，避免异常长文件名
        && !filename.contains(['/', '\\']) // 禁止任何路径分隔符，防止目录穿越
        && filename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) // 只允许字母数字与 . _ -
        && matches!(
            filename
                .rsplit('.')
                .next() // 取最后一个 '.' 之后的部分作为扩展名
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "gif") // 扩展名必须是已知图片类型之一
        )
}

#[cfg(test)]
mod tests {
    use super::*; // 引入被测的所有函数

    // 构造一个只含最小合法头部（无全局颜色表）的 GIF，payload 部分由调用方指定。
    fn minimal_gif(payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec(); // 签名 + 1x1 逻辑屏幕描述符 + 无颜色表标记
        bytes.extend_from_slice(payload); // 拼接测试用例自定义的后续数据
        bytes
    }

    #[test]
    fn changes_single_play_gif_to_infinite_loop() {
        // 已有 NETSCAPE2.0 扩展，循环次数为 1（只播一次）
        let mut gif = minimal_gif(b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x01\x00\x00\x2c\x3b");

        assert!(make_gif_loop_forever(&mut gif)); // 应成功就地修改
        assert!(gif
            .windows(5)
            .any(|window| window == b"\x03\x01\x00\x00\x00")); // 循环计数应被改写为 0（无限循环）
    }

    #[test]
    fn adds_infinite_loop_extension_when_gif_has_none() {
        // 没有任何应用扩展，直接是图像描述符 + 文件尾
        let mut gif = minimal_gif(b"\x2c\x3b");

        assert!(make_gif_loop_forever(&mut gif)); // 应成功插入新扩展
        assert_eq!(&gif[13..32], b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"); // 新扩展应插入在颜色表结束处（此处即偏移 13）
    }

    #[test]
    fn finds_loop_extension_after_other_extension_data() {
        // 先有一个无关的图形控制扩展（0x21 0xfe...），之后才是循环扩展
        let mut gif =
            minimal_gif(b"\x21\xfe\x01\x2c\x00\x21\xff\x0bNETSCAPE2.0\x03\x01\x01\x00\x00\x2c\x3b");
        let original_len = gif.len();

        assert!(make_gif_loop_forever(&mut gif)); // 应能跳过无关扩展正确定位到循环扩展
        assert_eq!(gif.len(), original_len); // 就地修改不应改变文件总长度
        assert!(gif
            .windows(5)
            .any(|window| window == b"\x03\x01\x00\x00\x00")); // 循环计数已被改写为 0
    }

    #[test]
    fn leaves_non_gif_bytes_unchanged() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec(); // PNG 文件签名
        let original = png.clone();

        assert!(!make_gif_loop_forever(&mut png)); // 非 GIF 应直接返回 false
        assert_eq!(png, original); // 且内容必须原样不变
    }
}
