mod app;
mod auth;
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
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // アプリケーションを実行し、終了するまで待機
    let result = run_app(&mut terminal, token).await;

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
) -> anyhow::Result<()> {
    log::info!("Initializing application state");

    let mut app = AppState::new();
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);
    let rest_client = DiscordRestClient::new(token.clone());

    let gateway_url = rest_client.get_gateway_url().await?;
    log::info!("Gateway URL: {}", gateway_url);
    let gateway_client = GatewayClient::connect(token, gateway_url).await?;

    // Gateway イベントハンドラ
    let gateway_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let result = gateway_client
            .run(move |gateway_event| {
                let tx = gateway_event_tx.clone();
                tokio::spawn(async move {
                    let app_event = match gateway_event {
                        GatewayEvent::Ready(data) => AppEvent::GatewayReady(data),
                        GatewayEvent::GuildCreate(channels) => AppEvent::GuildCreate(channels),
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
                Command::LoadChannels => {
                    tokio::spawn(async move {
                        // まずギルドを取得
                        if let Ok(guilds) = rest.get_guilds().await {
                            for guild in guilds {
                                if let Ok(channels) = rest.get_guild_channels(&guild.id).await {
                                    let _ = tx.send(AppEvent::ChannelsLoaded(channels)).await;
                                }
                            }
                        }

                        // DM チャンネルも取得
                        if let Ok(dm_channels) = rest.get_dm_channels().await {
                            let _ = tx.send(AppEvent::ChannelsLoaded(dm_channels)).await;
                        }
                    });
                }
                Command::LoadMessages(channel_id) => {
                    tokio::spawn(async move {
                        if let Ok(messages) = rest.get_messages(&channel_id, 50).await {
                            let _ = tx
                                .send(AppEvent::MessagesLoaded {
                                    channel_id,
                                    messages,
                                })
                                .await;
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
                Command::None => {}
            }
        }
    }

    Ok(())
}
