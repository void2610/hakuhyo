use crate::discord::{Channel, Message, User};
use crate::events::AppEvent;
use crossterm::event::KeyCode;
use ratatui::widgets::ListState;
use std::collections::HashMap;

/// アプリケーション全体の状態
pub struct AppState {
    pub discord: DiscordState,
    pub ui: UiState,
}

/// Discord関連の状態
pub struct DiscordState {
    pub channels: HashMap<String, Channel>,
    pub messages: HashMap<String, Vec<Message>>, // channel_id -> messages
    pub current_user: Option<User>,
    pub connected: bool,
}

/// UI関連の状態
pub struct UiState {
    pub selected_channel: Option<String>,
    pub channel_list_state: ListState,
    #[allow(dead_code)]
    pub message_list_state: ListState,
    pub input_mode: InputMode,
    pub input_buffer: String,
}

/// 入力モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,  // ナビゲーションモード
    Editing, // 入力モード
}

/// コマンド（副作用を持つ処理）
#[derive(Debug, Clone)]
pub enum Command {
    LoadChannels,
    LoadMessages(String),
    SendMessage { channel_id: String, content: String },
    None,
}

impl AppState {
    /// 新しいアプリケーション状態を作成
    pub fn new() -> Self {
        Self {
            discord: DiscordState {
                channels: HashMap::new(),
                messages: HashMap::new(),
                current_user: None,
                connected: false,
            },
            ui: UiState {
                selected_channel: None,
                channel_list_state: ListState::default(),
                message_list_state: ListState::default(),
                input_mode: InputMode::Normal,
                input_buffer: String::new(),
            },
        }
    }

    /// イベントを処理して状態を更新
    pub fn update(&mut self, event: AppEvent) -> Command {
        match event {
            // Gateway イベント
            AppEvent::GatewayReady(ready_data) => {
                self.discord.current_user = Some(ready_data.user);
                self.discord.connected = true;
                Command::LoadChannels
            }

            AppEvent::GuildCreate(channels) => {
                // ギルドのチャンネル情報を追加
                for channel in channels {
                    self.discord.channels.insert(channel.id.clone(), channel);
                }

                // 最初のチャンネルを選択
                if self.ui.selected_channel.is_none() {
                    let first_channel_id = self
                        .get_channel_list()
                        .first()
                        .map(|ch| ch.id.clone());

                    if let Some(channel_id) = first_channel_id {
                        self.ui.selected_channel = Some(channel_id.clone());
                        self.ui.channel_list_state.select(Some(0));
                        return Command::LoadMessages(channel_id);
                    }
                }

                Command::None
            }

            AppEvent::MessageCreate(message) => {
                // メッセージを追加
                self.discord
                    .messages
                    .entry(message.channel_id.clone())
                    .or_default()
                    .push(message);
                Command::None
            }

            AppEvent::MessageUpdate(message) => {
                // メッセージを更新（簡略化: 既存のメッセージを置き換え）
                if let Some(messages) = self.discord.messages.get_mut(&message.channel_id) {
                    if let Some(pos) = messages.iter().position(|m| m.id == message.id) {
                        messages[pos] = message;
                    }
                }
                Command::None
            }

            AppEvent::MessageDelete { id, channel_id } => {
                // メッセージを削除
                if let Some(messages) = self.discord.messages.get_mut(&channel_id) {
                    messages.retain(|m| m.id != id);
                }
                Command::None
            }

            // コマンド完了イベント
            AppEvent::ChannelsLoaded(channels) => {
                for channel in channels {
                    self.discord.channels.insert(channel.id.clone(), channel);
                }

                // 最初のチャンネルを選択
                if self.ui.selected_channel.is_none() {
                    let first_channel_id = self
                        .get_channel_list()
                        .first()
                        .map(|ch| ch.id.clone());

                    if let Some(channel_id) = first_channel_id {
                        self.ui.selected_channel = Some(channel_id.clone());
                        self.ui.channel_list_state.select(Some(0));
                        return Command::LoadMessages(channel_id);
                    }
                }

                Command::None
            }

            AppEvent::MessagesLoaded {
                channel_id,
                messages,
            } => {
                self.discord.messages.insert(channel_id, messages);
                Command::None
            }

            AppEvent::MessageSent(message) => {
                // 送信したメッセージを追加（Gateway で MESSAGE_CREATE が来るので重複する可能性あり）
                self.discord
                    .messages
                    .entry(message.channel_id.clone())
                    .or_default()
                    .push(message);
                Command::None
            }

            // UI イベント
            AppEvent::KeyPress(key) => self.handle_key_press(key),
            AppEvent::Input(c) => {
                if self.ui.input_mode == InputMode::Editing {
                    self.ui.input_buffer.push(c);
                }
                Command::None
            }

            // システムイベント
            AppEvent::Tick => Command::None,
            AppEvent::Quit => Command::None,
        }
    }

    /// キー入力を処理
    fn handle_key_press(&mut self, key: KeyCode) -> Command {
        match self.ui.input_mode {
            InputMode::Normal => match key {
                KeyCode::Char('q') => Command::None, // Quit は main.rs で処理
                KeyCode::Char('i') => {
                    self.ui.input_mode = InputMode::Editing;
                    Command::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.select_previous_channel();
                    Command::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.select_next_channel();
                    Command::None
                }
                KeyCode::Enter => {
                    // チャンネル選択確定
                    if let Some(channel_id) = &self.ui.selected_channel {
                        Command::LoadMessages(channel_id.clone())
                    } else {
                        Command::None
                    }
                }
                _ => Command::None,
            },
            InputMode::Editing => match key {
                KeyCode::Esc => {
                    self.ui.input_mode = InputMode::Normal;
                    Command::None
                }
                KeyCode::Enter => {
                    if !self.ui.input_buffer.is_empty() {
                        let content = self.ui.input_buffer.clone();
                        self.ui.input_buffer.clear();

                        if let Some(channel_id) = &self.ui.selected_channel {
                            return Command::SendMessage {
                                channel_id: channel_id.clone(),
                                content,
                            };
                        }
                    }
                    Command::None
                }
                KeyCode::Backspace => {
                    self.ui.input_buffer.pop();
                    Command::None
                }
                KeyCode::Char(c) => {
                    self.ui.input_buffer.push(c);
                    Command::None
                }
                _ => Command::None,
            },
        }
    }

    /// 前のチャンネルを選択
    fn select_previous_channel(&mut self) {
        let channel_ids: Vec<String> = self
            .get_channel_list()
            .iter()
            .map(|ch| ch.id.clone())
            .collect();

        if channel_ids.is_empty() {
            return;
        }

        let current_index = self.ui.channel_list_state.selected().unwrap_or(0);
        let new_index = if current_index > 0 {
            current_index - 1
        } else {
            channel_ids.len() - 1
        };

        self.ui.channel_list_state.select(Some(new_index));
        self.ui.selected_channel = Some(channel_ids[new_index].clone());
    }

    /// 次のチャンネルを選択
    fn select_next_channel(&mut self) {
        let channel_ids: Vec<String> = self
            .get_channel_list()
            .iter()
            .map(|ch| ch.id.clone())
            .collect();

        if channel_ids.is_empty() {
            return;
        }

        let current_index = self.ui.channel_list_state.selected().unwrap_or(0);
        let new_index = if current_index < channel_ids.len() - 1 {
            current_index + 1
        } else {
            0
        };

        self.ui.channel_list_state.select(Some(new_index));
        self.ui.selected_channel = Some(channel_ids[new_index].clone());
    }

    /// チャンネルリストを取得（ソート済み）
    pub fn get_channel_list(&self) -> Vec<&Channel> {
        let mut channels: Vec<&Channel> = self.discord.channels.values().collect();
        channels.sort_by(|a, b| {
            // タイプでソート、次に名前でソート
            match a.channel_type.cmp(&b.channel_type) {
                std::cmp::Ordering::Equal => a.display_name().cmp(&b.display_name()),
                other => other,
            }
        });
        channels
    }

    /// 現在選択中のチャンネルのメッセージリストを取得
    pub fn get_current_messages(&self) -> Vec<&Message> {
        if let Some(channel_id) = &self.ui.selected_channel {
            if let Some(messages) = self.discord.messages.get(channel_id) {
                return messages.iter().collect();
            }
        }
        Vec::new()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
