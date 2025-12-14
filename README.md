# SimpleSFTP

**SimpleSFTP** is a lightweight, cross-platform SFTP client built with Rust and the [Iced](https://github.com/iced-rs/iced) GUI library. Designed for Linux, macOS, and Raspberry Pi, it aims to provide a fast and simple interface for media management and file transfers.

## Features & Status

| Feature | Status | Notes |
| :--- | :--- | :--- |
| **Modern UI** | ✅ Implemented | Split-pane layout (Queue/Remote), resizable panels, and overlay menus. |
| **Connection Manager** | ✅ Implemented | Connect via Host, Port, Username, and Password. Settings are persisted. |
| **Remote Browser** | ✅ Implemented | "FileZilla-like" detailed view (Name, Size, Type, Modified). |
| **Navigation** | ✅ Implemented | Double-click to enter folders or go up (`..`). Includes path canonicalization. |
| **Download Queue** | 🚧 In Progress | UI implemented with mock data. Real download logic coming soon. |
| **Local Browser** | 📅 Planned | Currently using placeholder UI. |
| **File Transfers** | 📅 Planned | Upload and Download functionality to be integrated. |

## Roadmap

*   **Phase 1 (Current)**: Core UI, authenticating with SFTP servers, and robust remote directory browsing.
*   **Phase 2**: Implementing the **Download Queue**, allowing files to be added and transferred asynchronously.
*   **Phase 3**: **Local File Browser** integration for full drag-and-drop support.
*   **Phase 4**: Advanced transfer features (Pause, Resume, Recursive directory downloads).

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
