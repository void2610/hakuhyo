mod app;
mod discord;
mod events;
mod ui;

use app::{AppState, Command};
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Discord Bot トークンを環境変数から取得
    let token = std::env::var("DISCORD_TOKEN")
        .expect("DISCORD_TOKEN environment variable must be set");

    // ターミナル初期化
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // アプリケーション実行
    let result = run_app(&mut terminal, token).await;

    // ターミナル復元
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // エラーがあれば表示
    if let Err(err) = result {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    token: String,
) -> anyhow::Result<()> {
    // アプリケーション状態初期化
    let mut app = AppState::new();

    // イベントチャンネル
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);

    // Discord REST クライアント
    let rest_client = DiscordRestClient::new(token.clone());

    // Gateway URL取得
    let gateway_url = rest_client.get_gateway_url().await?;

    // Gateway クライアント
    let gateway_client = GatewayClient::connect(token, gateway_url).await?;

    // Gateway イベントハンドラ（別タスク）
    let gateway_event_tx = event_tx.clone();
    tokio::spawn(async move {
        let result = gateway_client
            .run(move |gateway_event| {
                let tx = gateway_event_tx.clone();
                tokio::spawn(async move {
                    let app_event = match gateway_event {
                        GatewayEvent::Ready(data) => AppEvent::GatewayReady(data),
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
            eprintln!("Gateway error: {:?}", e);
        }
    });

    // UI イベントハンドラ（別タスク）
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

    // 描画タイマー（別タスク）
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
        terminal.draw(|f| ui::render(f, &app))?;

        // イベント処理
        if let Some(event) = event_rx.recv().await {
            // Quit イベントでループ終了
            if matches!(event, AppEvent::Quit) {
                break;
            }

            // 状態更新
            let command = app.update(event);

            // コマンド実行
            match command {
                Command::LoadChannels => {
                    // チャンネル一覧を取得
                    let rest = rest_client.clone();
                    let tx = event_tx.clone();
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
                    // メッセージ一覧を取得
                    let rest = rest_client.clone();
                    let tx = event_tx.clone();
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
                    // メッセージを送信
                    let rest = rest_client.clone();
                    let tx = event_tx.clone();
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
