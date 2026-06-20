mod app;
mod auth;
mod config;
mod discord;
mod events;
mod token_store;
mod ui;

use app::{AppState, Command};
use auth::get_or_authenticate_token;
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use discord::{DiscordRestClient, GatewayClient, GatewayEvent};
use events::AppEvent;
use futures::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use ratatui_image::picker::Picker;
use std::io;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};

/// ログを初期化（ファイルに出力）
fn init_logger() {
    use env_logger::Builder;
    use log::LevelFilter;
    use std::fs::OpenOptions;
    use std::io::Write;

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("hakuhyo.log")
        .expect("Failed to open log file");

    Builder::new()
        .filter_level(LevelFilter::Debug)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();
    log::info!("Hakuhyo starting...");

    // トークン取得（キーチェーン → 環境変数 → QRコード認証）
    let token = get_or_authenticate_token().await?;

    // ターミナル初期化（認証完了後）
    enable_raw_mode()?;
    // Picker は termios でフォントサイズを取得し、環境変数からプロトコルを推測
    let picker = match Picker::from_termios() {
        Ok(mut p) => {
            let proto = p.guess_protocol();
            log::info!("Image picker initialized: protocol={:?}", proto);
            Some(p)
        }
        Err(e) => {
            log::warn!("Failed to initialize image picker: {} — image rendering disabled", e);
            None
        }
    };
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // アプリケーションを実行し、終了するまで待機
    let result = run_app(&mut terminal, token, picker).await;

    // ターミナル復元
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        log::error!("Application error: {:?}", err);
        eprintln!("Error: {:?}", err);
    }

    log::info!("Hakuhyo shutting down");
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    token: String,
    picker: Option<Picker>,
) -> anyhow::Result<()> {
    log::info!("Initializing application state");

    let mut app = AppState::new();
    app.set_picker(picker);

    // 設定ファイルを読み込み
    if let Ok(config) = config::load_config() {
        app.load_favorites(config.favorites);
    } else {
        log::warn!("Failed to load config, using default");
    }

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
    let rest_client = DiscordRestClient::new(token.clone());

    let gateway_url = rest_client.get_gateway_url().await?;
    log::info!("Gateway URL: {}", gateway_url);
    let gateway_client = GatewayClient::new(token, gateway_url);

    // Gateway イベントハンドラ
    let gateway_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let result = gateway_client
            .run(move |gateway_event| {
                let tx = gateway_event_tx.clone();
                tokio::spawn(async move {
                    let app_event = match gateway_event {
                        GatewayEvent::Ready(data) => AppEvent::GatewayReady(data),
                        GatewayEvent::GuildCreate { guild, channels } => {
                            // ギルド情報を登録（READY後の新規ギルド参加用）
                            // 通常は READY イベントで既に全ギルドが登録されているため、
                            // これは後から参加したギルドの処理となる
                            log::info!("New guild joined: {} ({})", guild.name, guild.id);
                            AppEvent::GuildCreate { guild, channels }
                        }
                        GatewayEvent::ThreadUpsert(channel) => AppEvent::ThreadUpsert(channel),
                        GatewayEvent::ThreadDelete { id } => AppEvent::ThreadDelete { id },
                        GatewayEvent::MessageCreate(msg) => AppEvent::MessageCreate(msg),
                        GatewayEvent::MessageUpdate(msg) => AppEvent::MessageUpdate(msg),
                        GatewayEvent::MessageDelete { id, channel_id } => {
                            AppEvent::MessageDelete { id, channel_id }
                        }
                    };
                    let _ = tx.send(app_event).await;
                });
            })
            .await;

        if let Err(e) = result {
            log::error!("Gateway error: {:?}", e);
        }
    });

    // UI イベントハンドラ
    let ui_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(Ok(event)) = reader.next().await {
            match event {
                Event::Key(key_event) => {
                    // Ctrl+C で終了
                    if key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        let _ = ui_event_tx.send(AppEvent::Quit).await;
                        break;
                    }
                    // Ctrl+U / Ctrl+D でメッセージを大きめにスクロール (行単位)
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                        match key_event.code {
                            KeyCode::Char('u') => {
                                let _ = ui_event_tx
                                    .send(AppEvent::ScrollMessages(10))
                                    .await;
                                continue;
                            }
                            KeyCode::Char('d') => {
                                let _ = ui_event_tx
                                    .send(AppEvent::ScrollMessages(-10))
                                    .await;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    // 'q' で終了（Normal モード時のみ）
                    if key_event.code == KeyCode::Char('q') {
                        let _ = ui_event_tx.send(AppEvent::Quit).await;
                        break;
                    }

                    let _ = ui_event_tx.send(AppEvent::KeyPress(key_event.code)).await;
                }
                _ => {}
            }
        }
    });

    // 描画タイマー
    let tick_tx = event_tx.clone();
    tokio::spawn(async move {
        let mut tick_interval = interval(Duration::from_millis(100));
        loop {
            tick_interval.tick().await;
            if tick_tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // メインループ
    loop {
        // UI描画
        terminal.draw(|f| ui::render(f, &mut app))?;

        // イベント処理
        if let Some(event) = event_rx.recv().await {
            // Quit イベントでループ終了
            if matches!(event, AppEvent::Quit) {
                break;
            }

            // 状態更新
            let command = app.update(event);

            // コマンド実行
            let rest = rest_client.clone();
            let tx = event_tx.clone();
            match command {
                Command::LoadMessages(channel_id) => {
                    tokio::spawn(async move {
                        if let Ok(messages) = rest.get_messages(&channel_id, 50, None).await {
                            let _ = tx
                                .send(AppEvent::MessagesLoaded {
                                    channel_id,
                                    messages,
                                })
                                .await;
                        }
                    });
                }
                Command::LoadOlderMessages { channel_id, before } => {
                    tokio::spawn(async move {
                        match rest.get_messages(&channel_id, 50, Some(&before)).await {
                            Ok(messages) => {
                                let _ = tx
                                    .send(AppEvent::OlderMessagesLoaded {
                                        channel_id,
                                        messages,
                                    })
                                    .await;
                            }
                            Err(e) => {
                                log::warn!("Failed to load older messages: {}", e);
                                // 失敗時もロード中フラグを解除する (空の結果を送る)
                                let _ = tx
                                    .send(AppEvent::OlderMessagesLoaded {
                                        channel_id,
                                        messages: Vec::new(),
                                    })
                                    .await;
                            }
                        }
                    });
                }
                Command::SendMessage {
                    channel_id,
                    content,
                } => {
                    tokio::spawn(async move {
                        if let Ok(message) = rest.send_message(&channel_id, &content).await {
                            let _ = tx.send(AppEvent::MessageSent(message)).await;
                        }
                    });
                }
                Command::DownloadImages(items) => {
                    for (att_id, url) in items {
                        let tx2 = tx.clone();
                        tokio::spawn(async move {
                            log::debug!("Downloading image: id={}, url={}", att_id, url);
                            match reqwest::get(&url).await {
                                Ok(resp) => match resp.bytes().await {
                                    Ok(bytes) => {
                                        match tokio::task::spawn_blocking(move || {
                                            image::load_from_memory(&bytes)
                                        })
                                        .await
                                        {
                                            Ok(Ok(img)) => {
                                                let _ = tx2
                                                    .send(AppEvent::AttachmentImageLoaded {
                                                        attachment_id: att_id,
                                                        image: Box::new(img),
                                                    })
                                                    .await;
                                            }
                                            Ok(Err(e)) => {
                                                log::warn!("Failed to decode image: {}", e)
                                            }
                                            Err(e) => log::warn!("Decode task panicked: {}", e),
                                        }
                                    }
                                    Err(e) => log::warn!("Failed to read image bytes: {}", e),
                                },
                                Err(e) => log::warn!("Failed to download image: {}", e),
                            }
                        });
                    }
                }
                Command::OpenInDiscord { guild_id, channel_id } => {
                    // discord://-/channels/<guild_or_@me>/<channel_id>
                    let guild_segment = guild_id.unwrap_or_else(|| "@me".to_string());
                    let url = format!("discord://-/channels/{}/{}", guild_segment, channel_id);
                    log::info!("Opening in Discord app: {}", url);
                    tokio::spawn(async move {
                        let opener = if cfg!(target_os = "macos") {
                            "open"
                        } else if cfg!(target_os = "windows") {
                            "start"
                        } else {
                            "xdg-open"
                        };
                        let result = tokio::process::Command::new(opener)
                            .arg(&url)
                            .status()
                            .await;
                        if let Err(e) = result {
                            log::error!("Failed to launch Discord ({}): {}", opener, e);
                        }
                    });
                }
                Command::None => {}
            }
        }
    }

    // 終了時に設定を保存
    log::info!("Saving configuration...");
    let config_to_save = config::Config {
        favorites: app.get_favorites().clone(),
    };
    if let Err(e) = config::save_config(&config_to_save) {
        log::error!("Failed to save config: {}", e);
    }

    Ok(())
}
