use crate::discord::{Channel, Guild, Message, User};
use crate::events::AppEvent;
use crossterm::event::KeyCode;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};

/// アプリケーション全体の状態
pub struct AppState {
    pub discord: DiscordState,
    pub ui: UiState,
}

/// Discord関連の状態
pub struct DiscordState {
    pub guilds: HashMap<String, Guild>,          // guild_id -> guild
    pub channels: HashMap<String, Channel>,
    pub messages: HashMap<String, Vec<Message>>, // channel_id -> messages
    pub users: HashMap<String, User>,            // user_id -> user (DM表示用)
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
    // 検索・お気に入り関連
    pub favorites: HashSet<String>,     // お気に入りチャンネルID
    pub search_mode: bool,               // 検索モードフラグ
    pub search_buffer: String,           // 検索クエリ
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
    LoadMessages(String),
    SendMessage { channel_id: String, content: String },
    None,
}

impl AppState {
    /// 新しいアプリケーション状態を作成
    pub fn new() -> Self {
        Self {
            discord: DiscordState {
                guilds: HashMap::new(),
                channels: HashMap::new(),
                messages: HashMap::new(),
                users: HashMap::new(),
                current_user: None,
                connected: false,
            },
            ui: UiState {
                selected_channel: None,
                channel_list_state: ListState::default(),
                message_list_state: ListState::default(),
                input_mode: InputMode::Normal,
                input_buffer: String::new(),
                favorites: HashSet::new(),
                search_mode: false,
                search_buffer: String::new(),
            },
        }
    }

    /// お気に入り設定を読み込み
    pub fn load_favorites(&mut self, favorites: HashSet<String>) {
        self.ui.favorites = favorites;
        log::debug!("Loaded {} favorites", self.ui.favorites.len());
    }

    /// お気に入り設定を取得
    pub fn get_favorites(&self) -> &HashSet<String> {
        &self.ui.favorites
    }

    /// イベントを処理して状態を更新
    pub fn update(&mut self, event: AppEvent) -> Command {
        match event {
            // Gateway イベント
            AppEvent::GatewayReady(ready_data) => {
                // ユーザー情報を抽出
                if let Some(user_data) = ready_data.get("user") {
                    if let Ok(user) = serde_json::from_value(user_data.clone()) {
                        self.discord.current_user = Some(user);
                    }
                }
                self.discord.connected = true;

                // users フィールドからユーザー情報をキャッシュ（DM表示用）
                if let Some(users_array) = ready_data.get("users").and_then(|v| v.as_array()) {
                    log::info!("Found {} users in READY event", users_array.len());
                    for user_data in users_array {
                        if let Ok(user) = serde_json::from_value::<User>(user_data.clone()) {
                            self.discord.users.insert(user.id.clone(), user);
                        }
                    }
                    log::info!("Cached {} users", self.discord.users.len());
                } else {
                    log::warn!("READY event does NOT contain users field");
                }

                // ギルド情報を抽出して登録
                if let Some(guilds_array) = ready_data.get("guilds").and_then(|v| v.as_array()) {
                    for guild_data in guilds_array {
                        // ギルド情報を抽出
                        if let (Some(guild_id), Some(guild_name), Some(owner_id)) = (
                            guild_data.get("id").and_then(|v| v.as_str()),
                            guild_data.get("properties").and_then(|p| p.get("name")).and_then(|v| v.as_str()),
                            guild_data.get("properties").and_then(|p| p.get("owner_id")).and_then(|v| v.as_str()),
                        ) {
                            let guild = crate::discord::Guild {
                                id: guild_id.to_string(),
                                name: guild_name.to_string(),
                                icon: guild_data.get("properties").and_then(|p| p.get("icon")).and_then(|v| v.as_str()).map(|s| s.to_string()),
                                owner_id: owner_id.to_string(),
                            };

                            self.discord.guilds.insert(guild.id.clone(), guild.clone());

                            // チャンネル情報を抽出
                            if let Some(channels_array) = guild_data.get("channels").and_then(|v| v.as_array()) {
                                for channel_data in channels_array {
                                    if let Ok(mut channel) = serde_json::from_value::<crate::discord::Channel>(channel_data.clone()) {
                                        // チャンネルにguild_idを設定
                                        channel.guild_id = Some(guild.id.clone());

                                        // テキストチャンネル（type 0）のみ追加
                                        if channel.channel_type == 0 {
                                            self.discord.channels.insert(channel.id.clone(), channel);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // DM チャンネルを抽出
                if let Some(private_channels) = ready_data.get("private_channels").and_then(|v| v.as_array()) {
                    log::info!("Found {} private channels", private_channels.len());
                    for channel_data in private_channels.iter() {
                        if let Ok(mut channel) = serde_json::from_value::<crate::discord::Channel>(channel_data.clone()) {
                            // recipient_ids から recipients を構築
                            if let Some(recipient_ids) = &channel.recipient_ids {
                                let mut recipients = Vec::new();
                                for user_id in recipient_ids {
                                    if let Some(user) = self.discord.users.get(user_id) {
                                        recipients.push(user.clone());
                                    } else {
                                        log::warn!("User not found in cache: {}", user_id);
                                    }
                                }
                                channel.recipients = Some(recipients);
                            }

                            log::debug!(
                                "Adding DM channel: id={}, type={}, display_name={}",
                                channel.id,
                                channel.channel_type,
                                channel.display_name()
                            );
                            self.discord.channels.insert(channel.id.clone(), channel);
                        } else {
                            log::warn!("Failed to parse channel data: {}", channel_data);
                        }
                    }
                }
                log::info!("Total channels after READY: {}", self.discord.channels.len());

                // 最初のチャンネルを選択（お気に入りを優先）
                if self.ui.selected_channel.is_none() {
                    let first_channel_id = {
                        let favorites = self.get_favorite_channels();
                        if let Some(ch) = favorites.first() {
                            Some(ch.id.clone())
                        } else {
                            self.get_channel_list().first().map(|ch| ch.id.clone())
                        }
                    };

                    if let Some(channel_id) = first_channel_id {
                        self.ui.selected_channel = Some(channel_id.clone());
                        self.ui.channel_list_state.select(Some(0));
                        return Command::LoadMessages(channel_id);
                    }
                }

                Command::None
            }

            AppEvent::GuildCreate { guild, channels } => {
                // ギルド情報を登録
                self.discord.guilds.insert(guild.id.clone(), guild);

                // ギルドのチャンネル情報を追加
                for channel in channels {
                    self.discord.channels.insert(channel.id.clone(), channel);
                }

                // 最初のチャンネルを選択（お気に入りを優先）
                if self.ui.selected_channel.is_none() {
                    let first_channel_id = {
                        let favorites = self.get_favorite_channels();
                        if let Some(ch) = favorites.first() {
                            Some(ch.id.clone())
                        } else {
                            self.get_channel_list().first().map(|ch| ch.id.clone())
                        }
                    };

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
            AppEvent::MessagesLoaded {
                channel_id,
                messages,
            } => {
                self.discord.messages.insert(channel_id, messages);
                Command::None
            }

            AppEvent::MessageSent(message) => {
                // メッセージ送信後にメッセージリストを再読み込みして最新の状態を取得
                Command::LoadMessages(message.channel_id)
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
        // 検索モード時の処理
        if self.ui.search_mode {
            return match key {
                KeyCode::Esc => {
                    self.toggle_search_mode();
                    Command::None
                }
                KeyCode::Backspace => {
                    self.search_backspace();
                    Command::None
                }
                KeyCode::Up => self.select_previous_channel(),
                KeyCode::Down => self.select_next_channel(),
                KeyCode::Enter => {
                    // チャンネル選択確定して検索モードを終了
                    self.toggle_search_mode();
                    if let Some(channel_id) = &self.ui.selected_channel {
                        Command::LoadMessages(channel_id.clone())
                    } else {
                        Command::None
                    }
                }
                KeyCode::Char(c) => {
                    self.search_input(c);
                    Command::None
                }
                _ => Command::None,
            };
        }

        // 通常モード・編集モードの処理
        match self.ui.input_mode {
            InputMode::Normal => match key {
                KeyCode::Char('q') => Command::None, // Quit は main.rs で処理
                KeyCode::Char('i') => {
                    self.ui.input_mode = InputMode::Editing;
                    Command::None
                }
                KeyCode::Char('/') => {
                    // 検索モードに切り替え
                    self.toggle_search_mode();
                    Command::None
                }
                KeyCode::Char('f') => {
                    // お気に入り登録/解除
                    self.toggle_favorite();
                    Command::None
                }
                KeyCode::Up | KeyCode::Char('k') => self.select_previous_channel(),
                KeyCode::Down | KeyCode::Char('j') => self.select_next_channel(),
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

    /// 現在表示中のチャンネルリストを取得（検索モード/お気に入りモード対応）
    fn get_current_display_channels(&self) -> Vec<&Channel> {
        if self.ui.search_mode {
            // 検索モード: 検索結果を返す
            self.search_channels(&self.ui.search_buffer)
        } else {
            // 通常モード: お気に入りを返す
            self.get_favorite_channels()
        }
    }

    /// 前のチャンネルを選択
    fn select_previous_channel(&mut self) -> Command {
        let channel_ids: Vec<String> = self
            .get_current_display_channels()
            .iter()
            .map(|ch| ch.id.clone())
            .collect();

        if channel_ids.is_empty() {
            return Command::None;
        }

        let current_index = self.ui.channel_list_state.selected().unwrap_or(0);
        let new_index = if current_index > 0 {
            current_index - 1
        } else {
            channel_ids.len() - 1
        };

        self.ui.channel_list_state.select(Some(new_index));
        self.ui.selected_channel = Some(channel_ids[new_index].clone());

        // チャンネル切り替え時に自動的にメッセージを読み込む
        Command::LoadMessages(channel_ids[new_index].clone())
    }

    /// 次のチャンネルを選択
    fn select_next_channel(&mut self) -> Command {
        let channel_ids: Vec<String> = self
            .get_current_display_channels()
            .iter()
            .map(|ch| ch.id.clone())
            .collect();

        if channel_ids.is_empty() {
            return Command::None;
        }

        let current_index = self.ui.channel_list_state.selected().unwrap_or(0);
        let new_index = if current_index < channel_ids.len() - 1 {
            current_index + 1
        } else {
            0
        };

        self.ui.channel_list_state.select(Some(new_index));
        self.ui.selected_channel = Some(channel_ids[new_index].clone());

        // チャンネル切り替え時に自動的にメッセージを読み込む
        Command::LoadMessages(channel_ids[new_index].clone())
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

    /// お気に入りチャンネルリストを取得（ソート済み）
    pub fn get_favorite_channels(&self) -> Vec<&Channel> {
        let mut favorites: Vec<&Channel> = self
            .discord
            .channels
            .values()
            .filter(|ch| self.ui.favorites.contains(&ch.id))
            .collect();

        favorites.sort_by(|a, b| {
            match a.channel_type.cmp(&b.channel_type) {
                std::cmp::Ordering::Equal => a.display_name().cmp(&b.display_name()),
                other => other,
            }
        });

        favorites
    }

    /// チャンネルを検索（名前・ギルド名でフィルタリング）
    pub fn search_channels(&self, query: &str) -> Vec<&Channel> {
        if query.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        log::debug!("Searching channels with query: '{}'", query_lower);
        log::debug!("Total channels to search: {}", self.discord.channels.len());

        let mut results: Vec<&Channel> = self
            .discord
            .channels
            .values()
            .filter(|ch| {
                // チャンネル名で検索
                let display_name = ch.display_name();
                let name_match = display_name.to_lowercase().contains(&query_lower);

                // ギルド名で検索
                let guild_match = if let Some(guild_id) = &ch.guild_id {
                    if let Some(guild) = self.discord.guilds.get(guild_id) {
                        guild.name.to_lowercase().contains(&query_lower)
                    } else {
                        false
                    }
                } else {
                    false
                };

                let matched = name_match || guild_match;
                if matched {
                    log::debug!(
                        "Matched channel: {} (type={}, guild_id={:?})",
                        display_name,
                        ch.channel_type,
                        ch.guild_id
                    );
                }

                matched
            })
            .collect();

        log::debug!("Search found {} results", results.len());

        results.sort_by(|a, b| {
            match a.channel_type.cmp(&b.channel_type) {
                std::cmp::Ordering::Equal => a.display_name().cmp(&b.display_name()),
                other => other,
            }
        });

        results
    }

    /// お気に入りを登録/解除
    pub fn toggle_favorite(&mut self) {
        if let Some(channel_id) = &self.ui.selected_channel {
            if self.ui.favorites.contains(channel_id) {
                self.ui.favorites.remove(channel_id);
                log::info!("Removed from favorites: {}", channel_id);
            } else {
                self.ui.favorites.insert(channel_id.clone());
                log::info!("Added to favorites: {}", channel_id);
            }
        }
    }

    /// 検索モードを切り替え
    pub fn toggle_search_mode(&mut self) {
        self.ui.search_mode = !self.ui.search_mode;

        if self.ui.search_mode {
            // 検索モードに入る時はバッファをクリア
            self.ui.search_buffer.clear();
            log::debug!("Entered search mode");
        } else {
            // 検索モードを抜ける時はバッファをクリア
            self.ui.search_buffer.clear();
            log::debug!("Exited search mode");
        }
    }

    /// 検索入力を追加
    pub fn search_input(&mut self, c: char) {
        if self.ui.search_mode {
            self.ui.search_buffer.push(c);
            log::debug!("Search query: {}", self.ui.search_buffer);
        }
    }

    /// 検索入力をバックスペース
    pub fn search_backspace(&mut self) {
        if self.ui.search_mode {
            self.ui.search_buffer.pop();
            log::debug!("Search query: {}", self.ui.search_buffer);
        }
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
