use crate::app::{AppState, InputMode};
use crate::discord::Message;
use chrono::{DateTime, Utc};
use unicode_width::UnicodeWidthStr;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{CropOptions, Resize, StatefulImage};

/// TUIを描画
pub fn render(frame: &mut Frame, app: &mut AppState) {
    // メインレイアウト: 左サイドバー | 右コンテンツ
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // サイドバー
            Constraint::Percentage(75), // メインコンテンツ
        ])
        .split(frame.area());

    // 右エリア: メッセージエリア | 入力エリア
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),      // メッセージ
            Constraint::Length(3),   // 入力
            Constraint::Length(1),   // ステータスバー
        ])
        .split(main_chunks[1]);

    // サイドバーを上下に分割: 上 = Favorites、下 = Unread
    let sidebar_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[0]);

    if !app.ui.search_mode {
        render_channel_list(frame, app, sidebar_chunks[0]);
        render_unread_list(frame, app, sidebar_chunks[1]);
    } else {
        // 検索モード時はサイドバーを淡く表示
        let placeholder = List::new(Vec::<ListItem>::new()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Favorites")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(placeholder, sidebar_chunks[0]);
        let placeholder2 = List::new(Vec::<ListItem>::new()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Unread")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(placeholder2, sidebar_chunks[1]);
    }

    // メッセージリストを描画
    render_message_list(frame, app, content_chunks[0]);

    // 入力エリアを描画
    render_input_area(frame, app, content_chunks[1]);

    // ステータスバーを描画
    render_status_bar(frame, app, content_chunks[2]);

    // 検索モードの場合、最後にオーバーレイを描画
    if app.ui.search_mode {
        render_search_overlay(frame, app);
    }
}

/// チャンネルリストを描画（お気に入り）
fn render_channel_list(frame: &mut Frame, app: &mut AppState, area: ratatui::layout::Rect) {
    // 通常モード: お気に入りを表示
    let favorites = app.get_favorite_channels();

    let items: Vec<ListItem> = favorites
        .iter()
        .map(|channel| {
            let prefix = channel.type_prefix();
            let name = channel.display_name();

            // ギルド名を取得
            let guild_name = if let Some(guild_id) = &channel.guild_id {
                if let Some(guild) = app.discord.guilds.get(guild_id) {
                    format!("[{}] ", guild.name)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // スレッドの場合は親チャンネル名を併記
            let parent_name = channel
                .parent_id
                .as_ref()
                .and_then(|pid| app.discord.channels.get(pid))
                .map(|parent| format!("{} > ", parent.display_name()))
                .unwrap_or_default();

            // お気に入りマークを追加
            let favorite_mark = "⭐ ";

            let content = format!("{}{}{}{}{}", favorite_mark, guild_name, parent_name, prefix, name);

            let style = if Some(&channel.id) == app.ui.selected_channel.as_ref() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Favorites")
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(list, area, &mut app.ui.channel_list_state);
}

/// 未読チャンネル一覧を描画
fn render_unread_list(frame: &mut Frame, app: &AppState, area: ratatui::layout::Rect) {
    let unread = app.get_unread_channels();
    let title = format!("Unread ({})", unread.len());

    let items: Vec<ListItem> = unread
        .iter()
        .map(|channel| {
            let prefix = channel.type_prefix();
            let name = channel.display_name();

            let guild_name = channel
                .guild_id
                .as_ref()
                .and_then(|gid| app.discord.guilds.get(gid))
                .map(|g| format!("[{}] ", g.name))
                .unwrap_or_default();

            let parent_name = channel
                .parent_id
                .as_ref()
                .and_then(|pid| app.discord.channels.get(pid))
                .map(|parent| format!("{} > ", parent.display_name()))
                .unwrap_or_default();

            let content = format!("• {}{}{}{}", guild_name, parent_name, prefix, name);

            let style = if Some(&channel.id) == app.ui.selected_channel.as_ref() {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Red)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    frame.render_widget(list, area);
}

/// メッセージリストを描画
fn render_message_list(frame: &mut Frame, app: &mut AppState, area: ratatui::layout::Rect) {
    // タイトル算出
    let title = if let Some(channel_id) = &app.ui.selected_channel {
        if let Some(channel) = app.discord.channels.get(channel_id) {
            let guild_name = channel
                .guild_id
                .as_ref()
                .and_then(|gid| app.discord.guilds.get(gid))
                .map(|g| format!("[{}] ", g.name))
                .unwrap_or_default();

            let parent_name = channel
                .parent_id
                .as_ref()
                .and_then(|pid| app.discord.channels.get(pid))
                .map(|p| format!("{} > ", p.display_name()))
                .unwrap_or_default();

            format!(
                " {}{}{}{} ",
                guild_name,
                parent_name,
                channel.type_prefix(),
                channel.display_name()
            )
        } else {
            "Messages".to_string()
        }
    } else {
        "Messages".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // 借用衝突を避けるため、表示対象のメッセージを clone で抽出
    let messages: Vec<Message> = app
        .get_current_messages()
        .iter()
        .map(|m| (*m).clone())
        .collect();

    if messages.is_empty() {
        let placeholder = Paragraph::new("No messages").alignment(Alignment::Center);
        frame.render_widget(placeholder, inner);
        return;
    }

    // 画像高さ計算用のセル寸法 (1セルあたりピクセル数)。Picker 未取得時は妥当なデフォルト
    let (cell_w_px, cell_h_px) = app
        .picker
        .as_ref()
        .map(|p| p.font_size)
        .unwrap_or((10, 20));

    // 画像セル高さの上下限
    const IMAGE_MIN_H: u16 = 3;
    const IMAGE_MAX_H: u16 = 24;
    const IMAGE_FALLBACK_H: u16 = 10;

    let area_w = inner.width;
    let inner_top = inner.y as i32;
    let inner_bottom = inner_top + inner.height as i32;

    // 元画像のサイズから、アスペクトを保ったリサイズ後 (target_w_px, target_h_px) と表示セル高さを算出
    let calc_dims = |orig_w: u32, orig_h: u32| -> Option<(u16, u32, u32)> {
        if orig_w == 0 || orig_h == 0 || area_w == 0 || cell_w_px == 0 || cell_h_px == 0 {
            return None;
        }
        let area_w_px = (area_w as u32).saturating_mul(cell_w_px as u32);
        let max_h_px = (IMAGE_MAX_H as u32).saturating_mul(cell_h_px as u32);
        // 幅基準のアスペクト保持リサイズ
        let h_by_width_px =
            (orig_h as u64 * area_w_px as u64 / orig_w as u64).min(u32::MAX as u64) as u32;
        let (target_w_px, target_h_px) = if h_by_width_px > max_h_px {
            // 縦長で MAX_H 超過 → 高さ基準で抑え、幅は < area_w_px に
            let target_h = max_h_px;
            let target_w =
                (orig_w as u64 * target_h as u64 / orig_h as u64).min(u32::MAX as u64) as u32;
            (target_w, target_h)
        } else {
            (area_w_px, h_by_width_px)
        };
        // ratatui-image の ImageSource.desired は ceil() で算出されるため、こちらも ceil で揃える
        // (round だと target_h_px = 340px などで desired=22 / cells=21 のズレが生じて再リサイズが走る)
        let cells = ((target_h_px as f32 / cell_h_px as f32).ceil() as u16)
            .clamp(IMAGE_MIN_H, IMAGE_MAX_H);
        Some((cells, target_w_px, target_h_px))
    };

    // 全メッセージの (msg, 総高さ, 画像リスト) を最新→古い順で計算
    type MessageImages = Vec<(String, u16)>;
    let entries: Vec<(Message, u16, MessageImages)> = messages
        .iter()
        .map(|msg| {
            let images: MessageImages = msg
                .attachments
                .iter()
                .filter(|a| {
                    a.content_type
                        .as_deref()
                        .is_some_and(|ct| ct.starts_with("image/"))
                        && app.discord.image_sources.contains_key(&a.id)
                })
                .map(|a| {
                    let (ow, oh) = if let Some(src) = app.discord.image_sources.get(&a.id) {
                        (src.width(), src.height())
                    } else {
                        (a.width.unwrap_or(0), a.height.unwrap_or(0))
                    };
                    let cells = calc_dims(ow, oh)
                        .map(|(c, _, _)| c)
                        .unwrap_or(IMAGE_FALLBACK_H);
                    (a.id.clone(), cells)
                })
                .collect();
            // 画像が多数 or 高さが大きい場合に u16 がオーバーフローしないよう u32 で集計
            let img_sum: u32 = images.iter().map(|(_, c)| *c as u32).sum();
            let h: u16 = (1u32 + img_sum).min(u16::MAX as u32) as u16;
            (msg.clone(), h, images)
        })
        .collect();

    // 画像キャッシュを area_w に合わせて準備 (アスペクトは保持してリサイズ)
    {
        let sources = &app.discord.image_sources;
        let protocols = &mut app.discord.image_protocols;
        let resized_cache = &mut app.discord.image_resized;
        if let Some(picker) = app.picker.as_mut() {
            for (_, _, imgs) in entries.iter() {
                for (att_id, _) in imgs {
                    let cached_w = protocols.get(att_id).map(|(w, _, _)| *w);
                    if cached_w == Some(area_w) {
                        continue;
                    }
                    let Some(source) = sources.get(att_id) else {
                        continue;
                    };
                    let Some((_, target_w_px, target_h_px)) =
                        calc_dims(source.width(), source.height())
                    else {
                        continue;
                    };
                    if target_w_px == 0 || target_h_px == 0 {
                        continue;
                    }
                    let resized = source.resize_exact(
                        target_w_px,
                        target_h_px,
                        image::imageops::FilterType::Triangle,
                    );
                    let protocol = picker.new_resize_protocol(resized.clone());
                    protocols.insert(att_id.clone(), (area_w, None, protocol));
                    resized_cache.insert(att_id.clone(), (area_w, resized));
                }
            }
        }
    }

    // 全体高さからスクロール offset の上限を決めてクランプ
    let total_height: u32 = entries.iter().map(|(_, h, _)| *h as u32).sum();
    let max_offset = total_height.saturating_sub(inner.height as u32) as usize;
    let scroll_offset = app.ui.message_scroll_offset.min(max_offset);
    app.ui.message_scroll_offset = scroll_offset; // 過剰な offset をクランプして書き戻す
    app.ui.cached_max_scroll_offset = max_offset; // 最古到達判定に使う

    // 最新メッセージの底辺 y を求める。offset 0 で inner 下端ぴったり、offset>0 で下に押し下げる
    let mut y_bottom: i32 = inner_bottom + scroll_offset as i32;

    for (msg, h, images) in entries.iter() {
        let y_top = y_bottom - *h as i32;

        // 画面下端より下にメッセージ全体がある場合 (offset 大きすぎ等) → skip して次へ
        if y_top >= inner_bottom {
            y_bottom = y_top;
            continue;
        }
        // 画面上端より上にメッセージ全体が抜けたら、これより古いメッセージは描画不要
        if y_bottom <= inner_top {
            break;
        }

        // テキスト行 (画面内なら描画)
        if y_top >= inner_top && y_top < inner_bottom {
            let text_area = Rect {
                x: inner.x,
                y: y_top as u16,
                width: inner.width,
                height: 1,
            };
            let line = build_message_line(msg);
            frame.render_widget(Paragraph::new(line), text_area);
        }

        // 画像領域 (テキストの 1 行下から)
        let mut img_y = y_top + 1;
        for (att_id, img_h) in images {
            let img_top = img_y;
            let img_bottom = img_top + *img_h as i32;

            // 完全に画面外
            if img_bottom <= inner_top || img_top >= inner_bottom {
                img_y = img_bottom;
                continue;
            }

            let visible_top = img_top.max(inner_top);
            let visible_bottom = img_bottom.min(inner_bottom);
            let visible_h = (visible_bottom - visible_top) as u16;
            let img_area = Rect {
                x: inner.x,
                y: visible_top as u16,
                width: inner.width,
                height: visible_h,
            };

            let hidden_top_cells = (visible_top - img_top) as u32;
            let hidden_bottom_cells = (img_bottom - visible_bottom) as u32;
            let partial = hidden_top_cells > 0 || hidden_bottom_cells > 0;

            let two_sided = hidden_top_cells > 0 && hidden_bottom_cells > 0;

            if two_sided {
                // 上下両方が切れる (画像が画面より大きい) ケースは Resize::Crop では片側しか
                // 指定できないため自前クロップ。同一 (area_w, hidden_top, visible) のときは
                // 直近の protocol を再利用してエンコードを省略する。
                let visible_cells = visible_h as u32;
                let cached_match = app
                    .discord
                    .image_partial_protocols
                    .get(att_id)
                    .map(|(cw, ch, cv, _)| {
                        *cw == area_w && *ch == hidden_top_cells && *cv == visible_cells
                    })
                    .unwrap_or(false);
                if !cached_match {
                    if let (Some((_, resized)), Some(picker)) =
                        (app.discord.image_resized.get(att_id), app.picker.as_mut())
                    {
                        let w = resized.width();
                        let h_px = resized.height();
                        let img_h_cells = *img_h as u32;
                        if w > 0 && h_px > 0 && img_h_cells > 0 && visible_cells > 0 {
                            let crop_y = ((hidden_top_cells as u64 * h_px as u64)
                                / img_h_cells as u64)
                                .min(h_px as u64)
                                as u32;
                            let crop_h_raw =
                                (visible_cells as u64 * h_px as u64) / img_h_cells as u64;
                            let crop_h =
                                (crop_h_raw as u32).min(h_px.saturating_sub(crop_y));
                            if crop_h > 0 {
                                let cropped = resized.crop_imm(0, crop_y, w, crop_h);
                                let protocol = picker.new_resize_protocol(cropped);
                                app.discord.image_partial_protocols.insert(
                                    att_id.clone(),
                                    (area_w, hidden_top_cells, visible_cells, protocol),
                                );
                            }
                        }
                    }
                }
                if let Some((_, _, _, protocol)) =
                    app.discord.image_partial_protocols.get_mut(att_id)
                {
                    let widget = StatefulImage::new(None);
                    frame.render_stateful_widget(widget, img_area, protocol);
                }
            } else {
                let desired_clip_top: Option<bool> = if partial {
                    Some(hidden_top_cells > 0)
                } else {
                    None
                };
                // 前回の clip_top と違うなら protocol を作り直して内部の encode をリセット
                // (ratatui-image は CropOptions の変化を needs_resize で検知できないため)
                let needs_rebuild = app
                    .discord
                    .image_protocols
                    .get(att_id)
                    .map(|(_, last, _)| *last != desired_clip_top)
                    .unwrap_or(false);
                if needs_rebuild {
                    if let (Some((_, resized)), Some(picker)) = (
                        app.discord.image_resized.get(att_id),
                        app.picker.as_mut(),
                    ) {
                        let new_protocol = picker.new_resize_protocol(resized.clone());
                        if let Some(entry) = app.discord.image_protocols.get_mut(att_id) {
                            entry.1 = desired_clip_top;
                            entry.2 = new_protocol;
                        }
                    }
                } else if let Some(entry) = app.discord.image_protocols.get_mut(att_id) {
                    entry.1 = desired_clip_top;
                }

                if let Some((_, _, protocol)) = app.discord.image_protocols.get_mut(att_id) {
                    let widget = if partial {
                        StatefulImage::new(None).resize(Resize::Crop(Some(CropOptions {
                            clip_top: hidden_top_cells > 0,
                            clip_left: false,
                        })))
                    } else {
                        StatefulImage::new(None)
                    };
                    frame.render_stateful_widget(widget, img_area, protocol);
                }
            }

            img_y = img_bottom;
        }

        y_bottom = y_top;
    }
}


/// 1メッセージ分のテキスト行を構築
fn build_message_line(msg: &Message) -> Line<'_> {
    let time = format_timestamp(&msg.timestamp);

    let mut spans = vec![
        Span::styled(
            format!("[{}] ", time),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{}: ", msg.author_display_name()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    if !msg.content.is_empty() {
        spans.push(Span::raw(&msg.content));
    }

    for (i, attachment) in msg.attachments.iter().enumerate() {
        if i > 0 || !msg.content.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            attachment.display_text(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    Line::from(spans)
}

/// 入力エリアを描画
fn render_input_area(frame: &mut Frame, app: &mut AppState, area: ratatui::layout::Rect) {
    let style = match app.ui.input_mode {
        InputMode::Editing => Style::default().fg(Color::Yellow),
        InputMode::Normal => Style::default(),
    };

    let title = match app.ui.input_mode {
        InputMode::Editing => "Input (Press Esc to exit, Enter to send)",
        InputMode::Normal => "Input (Press 'i' to edit)",
    };

    let input = Paragraph::new(app.ui.input_buffer.as_str())
        .style(style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(style),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(input, area);

    // カーソル表示（編集モードの場合）
    if app.ui.input_mode == InputMode::Editing {
        // 全角文字を考慮し、バイト長ではなく表示幅でカーソル位置を計算
        let cursor_x = area.x + app.ui.input_buffer.width() as u16 + 1;
        let cursor_y = area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// ステータスバーを描画
fn render_status_bar(frame: &mut Frame, app: &mut AppState, area: ratatui::layout::Rect) {
    let status = if app.discord.connected {
        Span::styled(
            " Connected ",
            Style::default().fg(Color::Black).bg(Color::Green),
        )
    } else {
        Span::styled(
            " Disconnected ",
            Style::default().fg(Color::Black).bg(Color::Red),
        )
    };

    let help = if app.ui.search_mode {
        // 検索モード
        Span::raw(" Esc: Exit search | ↑/↓: Navigate | Enter: Select ")
    } else {
        match app.ui.input_mode {
            InputMode::Normal => {
                Span::raw(" q: Quit | i: Edit | /: Search | f: Fav | o: Open | e/^U: ScrollUp | d/^D: ScrollDown | ↑/k ↓/j ")
            }
            InputMode::Editing => Span::raw(" Esc: Normal mode | Enter: Send message "),
        }
    };

    let status_line = Line::from(vec![status, help]);
    let paragraph = Paragraph::new(status_line).alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

/// 検索オーバーレイを描画（Spotlightスタイル）
fn render_search_overlay(frame: &mut Frame, app: &mut AppState) {
    let area = frame.area();

    // 画面中央に配置するための計算
    let vertical_margin = area.height / 6; // 上部の余白
    let horizontal_margin = area.width / 5; // 左右の余白

    // オーバーレイの領域を計算
    let overlay_area = Rect {
        x: area.x + horizontal_margin,
        y: area.y + vertical_margin,
        width: area.width.saturating_sub(horizontal_margin * 2),
        height: area.height.saturating_sub(vertical_margin * 2),
    };

    // 検索結果を取得
    let results = app.search_channels(&app.ui.search_buffer);
    let result_count = results.len();

    // 表示する結果の最大数を計算（検索ボックスとボーダーの分を除く）
    let max_results = (overlay_area.height as usize).saturating_sub(4).min(result_count);

    // オーバーレイレイアウト: 検索ボックス | 結果リスト
    let overlay_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),           // 検索ボックス
            Constraint::Min(1),              // 結果リスト
        ])
        .split(overlay_area);

    // 背景をクリア（オーバーレイ効果）
    frame.render_widget(Clear, overlay_area);

    // 検索ボックスを描画
    let search_input = Paragraph::new(app.ui.search_buffer.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Search ({} results) ", result_count))
                .border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                .style(Style::default().bg(Color::Black)),
        );

    frame.render_widget(search_input, overlay_chunks[0]);

    // カーソル表示（全角文字を考慮した表示幅で計算）
    let cursor_x = overlay_chunks[0].x + app.ui.search_buffer.width() as u16 + 1;
    let cursor_y = overlay_chunks[0].y + 1;
    frame.set_cursor_position((cursor_x, cursor_y));

    // 結果リストを描画
    let items: Vec<ListItem> = results
        .iter()
        .take(max_results)
        .map(|channel| {
            let prefix = channel.type_prefix();
            let name = channel.display_name();

            // ギルド名を取得
            let guild_name = if let Some(guild_id) = &channel.guild_id {
                if let Some(guild) = app.discord.guilds.get(guild_id) {
                    format!("[{}] ", guild.name)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // スレッドの場合は親チャンネル名を併記
            let parent_name = channel
                .parent_id
                .as_ref()
                .and_then(|pid| app.discord.channels.get(pid))
                .map(|parent| format!("{} > ", parent.display_name()))
                .unwrap_or_default();

            // お気に入りマークを追加
            let favorite_mark = if app.ui.favorites.contains(&channel.id) {
                "⭐ "
            } else {
                ""
            };

            let content = format!("{}{}{}{}{}", favorite_mark, guild_name, parent_name, prefix, name);

            ListItem::new(content)
        })
        .collect();

    let results_list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .style(Style::default().bg(Color::Black)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    frame.render_stateful_widget(results_list, overlay_chunks[1], &mut app.ui.channel_list_state);
}

/// タイムスタンプを "HH:MM" 形式に整形（日本時間）
fn format_timestamp(timestamp: &str) -> String {
    if let Ok(dt) = timestamp.parse::<DateTime<Utc>>() {
        // UTC+9（日本時間）に変換
        use chrono::offset::FixedOffset;
        let jst = FixedOffset::east_opt(9 * 3600).unwrap();
        let dt_jst = dt.with_timezone(&jst);
        dt_jst.format("%H:%M").to_string()
    } else {
        "??:??".to_string()
    }
}
