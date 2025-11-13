use crate::app::{AppState, InputMode};
use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

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

    // 検索モードでない場合のみ、お気に入りリストを描画
    if !app.ui.search_mode {
        render_channel_list(frame, app, main_chunks[0]);
    } else {
        // 検索モード時は空のお気に入りパネルを表示
        let empty_list = List::new(Vec::<ListItem>::new()).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Favorites")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(empty_list, main_chunks[0]);
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

            // お気に入りマークを追加
            let favorite_mark = "⭐ ";

            let content = format!("{}{}{}{}", favorite_mark, guild_name, prefix, name);

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

/// メッセージリストを描画
fn render_message_list(frame: &mut Frame, app: &mut AppState, area: ratatui::layout::Rect) {
    let mut messages = app.get_current_messages();

    if messages.is_empty() {
        let placeholder = Paragraph::new("No messages")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Messages")
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(placeholder, area);
        return;
    }

    // メッセージを逆順にして、古い順にする
    messages.reverse();

    let items: Vec<ListItem> = messages
        .iter()
        .map(|msg| {
            // タイムスタンプを整形
            let time = format_timestamp(&msg.timestamp);

            // メッセージを1行で構築
            let mut spans = vec![
                Span::styled(
                    format!("[{}] ", time),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}: ", msg.author.username),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
            ];

            // テキストコンテンツを追加
            if !msg.content.is_empty() {
                spans.push(Span::raw(&msg.content));
            }

            // 添付ファイル情報を同じ行に追加
            for (i, attachment) in msg.attachments.iter().enumerate() {
                if i > 0 || !msg.content.is_empty() {
                    spans.push(Span::raw(" "));
                }
                spans.push(Span::styled(
                    attachment.display_text(),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::ITALIC),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let title = if let Some(channel_id) = &app.ui.selected_channel {
        if let Some(channel) = app.discord.channels.get(channel_id) {
            format!("Messages - {}", channel.display_name())
        } else {
            "Messages".to_string()
        }
    } else {
        "Messages".to_string()
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    // メッセージリストの状態を使って、最後のメッセージを表示
    let last_index = messages.len().saturating_sub(1);
    let mut state = app.ui.message_list_state.clone();
    state.select(Some(last_index));

    frame.render_stateful_widget(list, area, &mut state);
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
        let cursor_x = area.x + app.ui.input_buffer.len() as u16 + 1;
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
                Span::raw(" q: Quit | i: Edit | /: Search | f: Favorite | ↑/k: Up | ↓/j: Down ")
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

    // カーソル表示
    let cursor_x = overlay_chunks[0].x + app.ui.search_buffer.len() as u16 + 1;
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

            // お気に入りマークを追加
            let favorite_mark = if app.ui.favorites.contains(&channel.id) {
                "⭐ "
            } else {
                ""
            };

            let content = format!("{}{}{}{}", favorite_mark, guild_name, prefix, name);

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
