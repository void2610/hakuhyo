use std::io::{stdin, stdout, Write};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant};

/// ターミナルの背景色を取得する。OSC 11 (`\x1b]11;?`) を投げて
/// `\x1b]11;rgb:rrrr/gggg/bbbb` のレスポンスを解析する。
/// 失敗時は環境変数 `HAKUHYO_BG_COLOR=RRGGBB` → デフォルト暗色の順で
/// フォールバックする。
///
/// 注意: 呼び出し前に raw mode に入っていることが望ましい (stdin が echo されないように)。
pub fn detect_background_color() -> [u8; 3] {
    if let Some(c) = query_osc11(Duration::from_millis(150)) {
        return c;
    }
    if let Some(c) = read_env_color() {
        return c;
    }
    // デフォルト: 一般的な暗色ターミナル
    [28, 28, 32]
}

fn read_env_color() -> Option<[u8; 3]> {
    let v = std::env::var("HAKUHYO_BG_COLOR").ok()?;
    parse_hex_color(v.trim_start_matches('#'))
}

fn parse_hex_color(s: &str) -> Option<[u8; 3]> {
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some([r, g, b])
}

fn query_osc11(timeout: Duration) -> Option<[u8; 3]> {
    let mut out = stdout();
    out.write_all(b"\x1b]11;?\x07").ok()?;
    out.flush().ok()?;

    // stdin を一時的に非ブロッキング設定にして read
    let fd = stdin().as_raw_fd();
    let saved = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if saved < 0 {
        return None;
    }
    let restore = saved;
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, saved | libc::O_NONBLOCK);
    }

    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 128];
    let mut filled = 0usize;
    let mut result = None;
    while Instant::now() < deadline {
        let n = unsafe {
            libc::read(
                fd,
                buf.as_mut_ptr().add(filled) as *mut _,
                (buf.len() - filled) as libc::size_t,
            )
        };
        if n > 0 {
            filled += n as usize;
            if let Some(c) = parse_osc11_response(&buf[..filled]) {
                result = Some(c);
                break;
            }
        } else {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    unsafe {
        libc::fcntl(fd, libc::F_SETFL, restore);
    }
    result
}

fn parse_osc11_response(buf: &[u8]) -> Option<[u8; 3]> {
    let s = std::str::from_utf8(buf).ok()?;
    let body = s.split_once("\x1b]11;rgb:")?.1;
    let body = body
        .split(['\x07', '\x1b'])
        .next()?
        .trim_end_matches('\\');
    let mut parts = body.split('/');
    let r = u16::from_str_radix(parts.next()?.trim(), 16).ok()?;
    let g = u16::from_str_radix(parts.next()?.trim(), 16).ok()?;
    let b = u16::from_str_radix(parts.next()?.trim(), 16).ok()?;
    // 16-bit -> 8-bit (上位 8bit)
    Some([(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8])
}
