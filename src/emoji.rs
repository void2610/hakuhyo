/// メッセージ content 内のセグメント (テキスト or カスタム絵文字)
#[derive(Debug, Clone)]
pub enum MessageSegment {
    Text(String),
    Emoji {
        #[allow(dead_code)]
        name: String,
        id: String,
        #[allow(dead_code)]
        animated: bool,
    },
}

/// content から `<:name:id>` / `<a:name:id>` をパースしてセグメント列に分解する。
/// `<` で始まらない範囲はテキストとしてまとめる。
pub fn parse_message_segments(content: &str) -> Vec<MessageSegment> {
    let mut segments: Vec<MessageSegment> = Vec::new();
    let mut text_buf = String::new();
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // 直近の `>` を探して `<...>` 候補を取り出す
            if let Some(end) = content[i..].find('>') {
                let candidate = &content[i..=i + end];
                if let Some(emoji) = try_parse_emoji(candidate) {
                    if !text_buf.is_empty() {
                        segments.push(MessageSegment::Text(std::mem::take(&mut text_buf)));
                    }
                    segments.push(emoji);
                    i += end + 1;
                    continue;
                }
            }
            // 候補にならない場合はそのまま 1 char 取り込み
            text_buf.push('<');
            i += 1;
        } else {
            // UTF-8 セーフに 1 char 取り込む
            let c = content[i..].chars().next().unwrap();
            text_buf.push(c);
            i += c.len_utf8();
        }
    }
    if !text_buf.is_empty() {
        segments.push(MessageSegment::Text(text_buf));
    }
    segments
}

/// `<:name:id>` / `<a:name:id>` 形式の文字列を解析
fn try_parse_emoji(s: &str) -> Option<MessageSegment> {
    let inner = s.strip_prefix('<')?.strip_suffix('>')?;
    let parts: Vec<&str> = inner.splitn(3, ':').collect();
    if parts.len() != 3 {
        return None;
    }
    let (animated, name, id) = match parts[0] {
        "" => (false, parts[1], parts[2]),
        "a" => (true, parts[1], parts[2]),
        _ => return None,
    };
    if name.is_empty() || id.is_empty() {
        return None;
    }
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(MessageSegment::Emoji {
        name: name.to_string(),
        id: id.to_string(),
        animated,
    })
}

/// 絵文字画像の取得 URL を構築
pub fn emoji_cdn_url(id: &str) -> String {
    format!("https://cdn.discordapp.com/emojis/{}.png?size=64", id)
}

/// ダウンロードしたカスタム絵文字を `2 cell x 1 cell` 描画用の StatefulProtocol に変換する。
/// - アルファ付き画像はターミナル背景色とブレンドして RGB へフラット化
///   (ratatui-image v2 の Kitty 実装が `to_rgb8` でアルファを捨てる対策)
/// - area サイズと一致するピクセルに `resize_exact` し、内部 padding (黒背景) を回避
pub fn prepare_emoji_protocol(
    picker: &mut ratatui_image::picker::Picker,
    image: image::DynamicImage,
    bg_color: [u8; 3],
) -> Box<dyn ratatui_image::protocol::StatefulProtocol> {
    let (cw, ch) = picker.font_size;
    let target_w = (2u32 * cw as u32).max(1);
    let target_h = (ch as u32).max(1);
    let flattened = flatten_alpha(&image, bg_color);
    let resized = flattened.resize_exact(
        target_w,
        target_h,
        image::imageops::FilterType::Triangle,
    );
    picker.new_resize_protocol(resized)
}

/// アルファチャネル付きの画像を「指定背景色との合成済み」不透明画像に変換する。
fn flatten_alpha(img: &image::DynamicImage, bg: [u8; 3]) -> image::DynamicImage {
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let composited = image::ImageBuffer::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y);
        let a = p[3] as u32;
        let inv = 255 - a;
        let blend = |src: u8, dst: u8| ((src as u32 * a + dst as u32 * inv) / 255) as u8;
        image::Rgba([
            blend(p[0], bg[0]),
            blend(p[1], bg[1]),
            blend(p[2], bg[2]),
            255,
        ])
    });
    image::DynamicImage::ImageRgba8(composited)
}
