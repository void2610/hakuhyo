# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Hakuhyo (薄氷) is a lightweight Discord TUI client written in Rust. It implements Discord REST API and WebSocket Gateway directly without using third-party Discord libraries.

**Important**: This project uses **User Account Authentication** via QR code, NOT Bot authentication. The authentication flow differs significantly from typical bot implementations.

## Build & Run Commands

```bash
# Build release version
cargo build --release

# Run the application
cargo run --release

# Clear saved token from keychain
cargo run --release --example clear_token
```

Logs are written to `hakuhyo.log` in the current directory.

## Project Architecture

### The Elm Architecture (TEA) Pattern

The application follows The Elm Architecture:

- **Model** (`AppState`): Application state containing Discord data and UI state
- **Update** (`app::update()`): Processes events and returns commands (side effects)
- **View** (`ui::render()`): Renders TUI based on current state

### Key Components

```
src/
├── main.rs           # Event loop, async task coordination
├── app.rs            # State management and update logic
├── ui.rs             # TUI rendering
├── events.rs         # Event definitions
├── auth.rs           # QR code authentication
├── token_store.rs    # OS keychain integration
├── config.rs         # Favorites persistence
└── discord/
    ├── models.rs     # Discord data structures
    ├── rest.rs       # REST API client
    └── gateway.rs    # WebSocket Gateway client
```

### Event Flow

```
User Input / Gateway Events
         ↓
   Event Channel (mpsc)
         ↓
   app::update()
         ↓
   Command Execution (async)
         ↓
   State Update
         ↓
   ui::render()
```

## Authentication Architecture

**Critical Difference**: User account authentication vs Bot authentication

### User Account Authentication (Current Implementation)

1. **QR Code Flow** (`auth.rs`):
   - Connects to `wss://remote-auth-gateway.discord.gg/?v=2`
   - Generates RSA key pair
   - Displays QR code for mobile app scanning
   - Receives encrypted token, decrypts with private key

2. **Token Storage** (`token_store.rs`):
   - Saves to plaintext file: `~/.config/hakuhyo/token.txt`
   - File permissions set to 0600 (owner read/write only on Unix systems)
   - Token validated on startup
   - Falls back to QR auth if invalid/missing
   - ⚠️ **Security Note**: Token stored in plaintext - ensure proper file system permissions

3. **Gateway Identify** (`discord/gateway.rs`):
   - Uses detailed `properties` mimicking Discord web client
   - Includes `capabilities`, `client_state`, `client_build_number`
   - **No `intents` field** (user accounts don't use intents)

### READY Event Handling

**User accounts receive ALL data in READY event** - no REST API calls needed:

- `ready_data.guilds[]` contains full guild objects with channels
- `ready_data.private_channels[]` contains DM channels
- Data structure: `guilds[].properties.name`, `guilds[].channels[]`

The `app.rs` extracts guilds and channels directly from READY payload, not via REST API.

## Data Flow Patterns

### Channel & Guild Data

- **Source**: Gateway READY event (not REST API)
- **Storage**: `AppState.discord.guilds` and `AppState.discord.channels` HashMaps
- **Access**: Search and favorites filter these HashMaps

### Message Loading

- **Trigger**: Channel selection (Enter key in search/favorites)
- **Command**: `Command::LoadMessages(channel_id)`
- **API**: REST `GET /channels/{id}/messages?limit=50`
- **Storage**: `AppState.discord.messages` HashMap (keyed by channel_id)

### Favorites

- **Storage**: `AppState.ui.favorites` HashSet of channel IDs
- **Persistence**: JSON file at `~/.config/hakuhyo/favorites.json`
- **Operations**: Toggle with `f` key, saved on app exit

## UI Modes

### Search Mode (`/` key)

- Activated: Press `/`
- Input: Characters update search query
- Navigation: `↑`/`↓` select from filtered results
- Confirm: `Enter` exits search mode and loads messages
- Cancel: `Esc` exits search mode

**Bug Fix**: Search mode must exit on Enter to allow normal operations (`i`, `f` keys)

### Normal Mode

- Navigation: `↑`/`↓` or `k`/`j` between channels
- Actions: `i` (edit), `f` (favorite toggle), `/` (search)
- Quit: `q`

### Editing Mode (`i` key)

- Input: Type message
- Send: `Enter`
- Cancel: `Esc` returns to Normal mode

## Important Implementation Details

### Gateway Events

- **GUILD_CREATE**: After READY, when joining new guilds (rare during runtime)
- **MESSAGE_CREATE**: New message in any channel
- **MESSAGE_UPDATE/DELETE**: Message modifications

### REST API Usage

**Minimal REST usage** (user accounts get most data via Gateway):

- `GET /channels/{id}/messages` - Message history
- `POST /channels/{id}/messages` - Send message
- `GET /gateway` - Gateway URL

**Not used** (data comes from READY):
- ~~GET /users/@me/guilds~~
- ~~GET /guilds/{id}/channels~~
- ~~GET /users/@me/channels~~

### Token File Storage

- **Location**: `~/.config/hakuhyo/token.txt`
- **Format**: Plaintext (single line)
- **Permissions**: 0600 on Unix systems (owner read/write only)
- **Security**: Token stored **without** "Bot " prefix
- **Note**: File is excluded in `.gitignore` to prevent accidental commits

## Code Style

- **All comments must be in Japanese** (per global CLAUDE.md)
- Use `log::info!`, `log::debug!`, `log::error!` for logging
- Async operations use `tokio::spawn` for concurrency
- Error handling with `anyhow::Result`

## Common Modifications

### Adding New UI Features

1. Update `UiState` in `app.rs`
2. Add event handling in `app::handle_key_press()`
3. Update rendering in `ui.rs`
4. Update status bar key hints

### Adding Gateway Events

1. Add event case in `gateway.rs::handle_message()`
2. Define in `GatewayEvent` enum
3. Map to `AppEvent` in `main.rs`
4. Handle in `app::update()`

### Modifying Search/Filter Logic

- Search: `app::search_channels()` - filters by name/guild
- Favorites: `app::get_favorite_channels()` - filters by ID set
- Display: `app::get_current_display_channels()` - returns active list

## Testing Authentication

To test fresh authentication:
```bash
cargo run --release --example clear_token  # Deletes ~/.config/hakuhyo/token.txt
cargo run --release  # Will prompt for QR code
```

Or manually:
```bash
rm ~/.config/hakuhyo/token.txt
cargo run --release
```

## Known Limitations

- User account authentication may violate Discord ToS (educational purposes only)
- No image/embed rendering
- No attachment sending
- No thread support
- Single-line message input only
