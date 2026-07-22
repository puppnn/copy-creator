<div align="right">

English | [中文](./README.md)

</div>

<div align="center">

<img src="copy-creator/public/logo.png" alt="Copy Creator Logo" width="120">

# Copy Creator

**Desktop Productivity Tool for Windows**

Clipboard Manager · Quick Phrases · Translation

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows%2010+-brightgreen.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.x-ffc131.svg)
![React](https://img.shields.io/badge/React-19-61dafb.svg)
![Version](https://img.shields.io/badge/version-0.2.22-00a6a6.svg)

</div>

---

## Overview

Copy Creator is a lightweight Windows desktop productivity tool that appears as a floating window and minimizes to the system tray when closed. It integrates three core features: clipboard history management, quick phrases, and translation, helping users improve text processing efficiency in their daily work.

This repository is a fork of [hu-qi-jia/copy-creator](https://github.com/hu-qi-jia/copy-creator). It keeps the original features, selectively incorporates useful OneClip capabilities, and adds continued Windows-specific improvements for clipboard interaction, paste focus, image previews, and window stability. The current customized release is **v0.2.22**.

## Improvements in This Fork (v0.2.22)

Compared with the upstream baseline, this fork includes substantial extensions to clipboard handling, image processing, window interaction, and data management. See the complete code comparison on [GitHub Compare](https://github.com/puppnn/copy-creator/compare/5b415599e82b61b13e034016ca3c287cdbd1a418...main).

### Clipboard Organization and Actions

- Added favorites and a dedicated Favorites category; automatic retention and capacity cleanup preserve favorite records first
- Favorite entries support adding, editing, and clearing notes; search and data import/export preserve those notes
- Added an Explorer Address category for Windows drive paths, UNC network paths, and selected Shell addresses, including migration of existing address records
- Text, links, Explorer addresses, and file paths can all be selected with the mouse; selecting text does not accidentally paste the entire record
- Hold `Ctrl` and click a supported link to open it in the default browser, with underline and pointer feedback while Ctrl is held
- Category controls wrap across two rows so every category remains visible in narrow windows
- Added a smooth back-to-top button without removing the existing scroll-position memory

### Image Experience and Processing

- Increased clipboard thumbnails from `200px` to `264px` for easier inspection
- Large previews appear only after hovering for `300ms`, preventing previews from flashing while moving through the list
- Full-image previews are limited to a `1600px` longest edge and use a smaller cache to reduce memory and UI pressure
- Added configurable maximum image dimensions and compression quality
- Images larger than 10 MB can be compressed automatically, kept unchanged, or skipped
- Image reading, resizing, and thumbnail generation run asynchronously to reduce main-window stalls

### Paste and Window Interaction

- Saves both the target application's top-level window and the actual focused input control before pasting
- Restores Windows focus and waits for the target control, improving paste accuracy in controls such as the File Explorer address bar
- Waits for `Ctrl`, `Alt`, and `Win` modifier keys to be released before sending the paste command
- Unpinned windows hide when focus moves outside; pinned windows remain visible
- Uses a stable native drag path so dragging neither closes the window nor generates repeated WebView IPC

### Storage, Backup, and Notifications

- Added configurable history-count and storage-space limits, plus record, favorite, and disk-usage statistics
- Added import and export for settings and favorites, including image data for favorite images
- Removes unreferenced image and thumbnail files while preserving files that are still in use
- Added optional clipboard notifications and unread counters in the sidebar and tray
- The tray menu exposes the eight most recent clipboard records with direct Copy and Paste actions

### Stability and Shortcut Notes

- Tray refreshes use a dedicated worker and coalesce repeated requests
- Fixed a potential database deadlock while cleaning up image files
- Removed the low-level `Win+V` keyboard hook that could freeze the app or open the Windows Start menu, returning to Tauri global shortcuts
- Because `Win+V` is reserved by Windows, this project does not forcibly replace the native clipboard at the low level; configure a non-conflicting global shortcut for the most reliable behavior

## Features

### 📋 Clipboard Manager
- Automatically records text, links, Explorer addresses, files, and image history
- Keyword search for quick access to historical content
- Category filtering, favorites, text selection, and `Ctrl+click` link opening
- One-click paste back to the original input control with Windows focus restoration
- Configurable retention, history-count, and storage-space limits with automatic cleanup
- Configurable image dimensions, compression quality, and large-image handling

### ⚡ Quick Phrases
- Organize common phrases and code snippets by scenario groups
- Customizable groups for flexible content organization
- Click to paste directly without manual copying

### 🌐 Translation
- **AI Translation**: Compatible with OpenAI API format, customizable endpoint and model
- **Built-in Translation**: Free translation service, ready to use out of the box
- Local caching of translation results to avoid redundant requests

### ⚙️ System Features
- Global hotkey to show/hide window
- Auto-hide when an unpinned window loses focus; pinned windows remain visible
- Light/Dark theme switching
- Launch at system startup
- Recent tray records, unread badges, and optional clipboard notifications
- Settings and favorites import/export

## Tech Stack

| Layer | Technology |
|:---:|:---|
| Desktop Framework | [Tauri 2.x](https://tauri.app/) (Rust) |
| Frontend Framework | React 19 + TypeScript |
| Build Tool | [Vite](https://vitejs.dev/) |
| UI Styling | Pure CSS (iOS-style frosted glass effect) |
| State Management | [Zustand](https://zustand-demo.pmnd.rs/) |
| Local Storage | SQLite (rusqlite, bundled) |
| Internationalization | react-i18next (Simplified Chinese / English) |

## Download

Check this fork's [Releases](https://github.com/puppnn/copy-creator/releases) page first for a customized installer. If a matching installer has not been published, build the customized branch by following the development guide below.

Upstream installers remain available from [hu-qi-jia/copy-creator Releases](https://github.com/hu-qi-jia/copy-creator/releases).

| Package | Description |
|:---|:---|
| `Copy Creator_x64-setup.exe` | NSIS Installer |
| `Copy Creator_x64_zh-CN.msi` | MSI Installer (Chinese) |

**System Requirements**: Windows 11

## Usage Guide

### Getting Started

1. **Launch the App**: Double-click the desktop icon after installation, the app will appear as a floating window
2. **System Tray**: When you close the window, the app automatically minimizes to the system tray and continues running in the background
3. **Show Window**: Use the global hotkey (configurable in settings) to quickly show/hide the window

### Clipboard Feature

1. Copy any text or image, and the system will automatically record it to clipboard history
2. Click the tray icon or use the hotkey to open the main window
3. Switch to the "Clipboard" tab to browse or search history
4. Click any record to paste it directly to the current cursor position

### Quick Phrases Feature

1. Switch to the "Phrases" tab
2. Click "New Group" to create scenario groups (e.g., customer service scripts, code snippets)
3. Add commonly used phrases to the group
4. When needed, click a phrase to paste it to the current input position

### Translation Feature

1. Switch to the "Translation" tab
2. Enter or paste the text to translate
3. Select translation direction (e.g., Chinese → English)
4. Click the translate button to get results
5. For AI translation, please configure the API endpoint and key in settings

### Personalization Settings

- **Hotkeys**: Customize global hotkeys
- **Theme**: Switch between light and dark themes
- **Launch at Startup**: Enable or disable auto-start on boot
- **Storage Management**: Configure retention, maximum history count, and storage limits
- **Image Processing**: Configure dimensions, compression quality, and large-image handling
- **Data Migration**: Import or export settings and favorite records

## Development Guide

### Prerequisites

- [Node.js](https://nodejs.org/) (18+ recommended)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/)
- [Tauri CLI](https://tauri.app/)

### Local Development

```bash
# Clone the repository
git clone https://github.com/puppnn/copy-creator.git
cd copy-creator/copy-creator

# Install dependencies
pnpm install

# Start development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Project Structure

```
copy-creator/
├── src/                    # Frontend source code
│   ├── components/         # React components
│   ├── pages/              # Page components
│   ├── stores/             # Zustand state management
│   ├── styles/             # CSS style files
│   ├── i18n/               # Internationalization config
│   └── types/              # TypeScript type definitions
├── src-tauri/              # Tauri backend source code
│   ├── src/                # Rust source code
│   └── Cargo.toml          # Rust dependency config
├── public/                 # Static assets
└── package.json            # Frontend dependency config
```

## License

This project is licensed under the [MIT License](LICENSE).

---

<div align="center">

If you find this project helpful, feel free to give it a Star!

Based on [hu-qi-jia/copy-creator](https://github.com/hu-qi-jia/copy-creator). Thanks to the original author and contributors.

</div>
