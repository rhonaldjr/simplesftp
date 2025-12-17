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
| **Throttle Downloads** | 📅 Planned | The ability to limit the download speed |
| **Scheduling** | 📅 Planned | The ability to schedule downloads |
| **Detect Media Type** | 📅 Planned | Detect the media type and then pick up the appropriate download folders |
| **File Cleanup** | 📅 Planned | Review the filenames, folder structures and perform cleanup to be consistent with the media type |
| **Local Browser** | 📅 Planned | Full local file browser integration. |

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
*   **Phase 4**: **Local File Browser** integration for drag-and-drop support.
*   **Phase 5**: **Advanced Features**:
    *   Scheduling downloads.
    *   Bandwidth throttling.
    *   Media type detection and auto-organization.
    *   File cleanup tools.

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

## Technologies

*   **Language**: Rust 🦀
*   **GUI Framework**: Iced
*   **SFTP Client**: ssh2-rs
