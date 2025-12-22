# SimpleSFTP

**SimpleSFTP** is a lightweight, cross-platform SFTP client built with Rust and the [Iced](https://github.com/iced-rs/iced) GUI library. Designed for Linux, macOS, and Raspberry Pi, it aims to provide a fast and simple interface for media management and file transfers.

## Features & Status

| Feature | Status | Notes |
| :--- | :--- | :--- |
| **Modern UI** | ✅ Implemented | Split-pane layout (Queue/Remote), resizable panels, and overlay menus. |
| **Connection Manager** | ✅ Implemented | Connect via Host, Port, Username, and Password. Settings are persisted. |
| **Remote Browser** | ✅ Implemented | "FileZilla-like" detailed view (Name, Size, Type, Modified). |
| **Navigation** | ✅ Implemented | Double-click to enter folders or go up (`..`). Includes path canonicalization. |
| **Download Queue** | ✅ Implemented | Queue items added via context menu. Recursive scanning supported. |
| **Context Menu** | ✅ Implemented | Right-click overlay with "Download" option and recursive folder scanning. |
| **Target Selection** | ✅ Implemented | Choose local download destination via native dialog. |
| **System Tray** | ✅ Implemented | Minimize to system tray running in background. |
| **File Transfers** | ✅ Implemented | Asynchronous concurrent downloads with progress tracking. |
| **Pause and Resume** | ✅ Implemented | Pause and resume individual downloads. |
| **Resume Queue when Connecting** | ✅ Implemented | Auto-connects to last host, restores path, and resumes pending downloads. |
| **Throttle Downloads** | ✅ Implemented | Limit max download speed (KB/s). |
| **Scheduling** | ✅ Implemented | Define start/end times and allowed days for downloads. |
| **Download Statistics** | ✅ Implemented | Track daily bytes and calculate weekly/monthly averages. |
| **Refresh & Speed** | ✅ Implemented | Refresh remote/queue and see live download speed in status bar. |
| **Change Download Priority** | 🕒 Planned | Ability to reorder queue items and change download priority. |

## Roadmap

*   **Phase 1 (Completed)**: Core UI, authenticating with SFTP servers, and robust remote directory browsing.
*   **Phase 2 (Completed)**: **Context Menu & Queueing**:
    *   Right-click "Download" support.
    *   Recursive directory scanning.
    *   Target folder selection.
    *   Queue management (Deduplication, Statistics).
    *   System Tray integration (minimize to background).
*   **Phase 3 (Completed)**: **File Transfer Engine**:
    *   Asynchronous chunked downloading.
    *   Concurrent transfer limits.
    *   Pause/Resume/Cancel support.
*   **Phase 5 (Completed)**: **Advanced Features**:
    *   Scheduling downloads.
    *   Auto-Connect & Session Restore.
*   **Phase 6 (Completed)**: **Optimization & Polish**:
    *   Bandwidth throttling.
    *   Download Statistics (Daily/Weekly/Monthly).
    *   Process Refresh (Remote & Queue).
    *   Live Download Speed Display.
*   **Change Download Priority** (Planned).

## How to Run

Ensure you have the [Rust toolchain](https://rustup.rs/) installed.

```bash
# Clone the repository
git clone https://github.com/yourusername/simplesftp.git
cd simplesftp

# Run the application
cargo run
```

## Known Issues

*   **Remote Navigation**: Double-clicking certain remote folders might throw a **"Permission denied"** error or fail to list contents, whereas other clients (e.g., FileZilla) work fine. 
    *   *Note*: Recent updates have introduced path resolution (`realpath`) to mitigate this by ensuring canonical paths are used, but some edge cases with specific server configurations or permissions may persist.
*   **Minimize to Tray**: The "Minimize to Tray" feature is currently experiencing issues where the application may not correctly hide to the tray or restore from it on certain Linux distributions/Desktop Environments.

## Technologies

*   **Language**: Rust 🦀
*   **GUI Framework**: Iced
*   **SFTP Client**: ssh2-rs
