<div align="right">

[English](./README_EN.md) | 中文

</div>

<div align="center">

<img src="copy-creator/public/logo.png" alt="Copy Creator Logo" width="120">

# Copy Creator

**PC 端效率辅助工具**

剪切板管理 · 快捷短语 · 翻译

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows%2010+-brightgreen.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.x-ffc131.svg)
![React](https://img.shields.io/badge/React-19-61dafb.svg)
![Version](https://img.shields.io/badge/version-0.2.21-00a6a6.svg)

</div>

---

## 项目简介

Copy Creator 是一款轻量级的 Windows 桌面效率工具，以悬浮窗形式呈现，关闭后自动驻留系统托盘。它集成了剪切板历史管理、快捷短语和翻译三大核心功能，帮助用户在日常工作中提升文本处理效率。

本仓库 Fork 自 [hu-qi-jia/copy-creator](https://github.com/hu-qi-jia/copy-creator)，在保留原有功能的基础上，选择性迁移了 OneClip 的部分能力，并针对 Windows 下的剪切板交互、粘贴焦点、图片预览和窗口稳定性进行了持续优化。当前定制版本为 **v0.2.21**。

## 本 Fork 的改进（v0.2.21）

相对上游基线，本 Fork 对剪切板、图片处理、窗口交互和数据管理进行了系统扩展。可在 [GitHub Compare](https://github.com/puppnn/copy-creator/compare/5b415599e82b61b13e034016ca3c287cdbd1a418...main) 查看完整代码差异。

### 剪切板组织与操作

- 新增**收藏**功能和独立收藏分类，可随时收藏、取消收藏；自动清理历史或超出容量时优先保留收藏内容
- 新增**资源管理器地址**分类，可识别 Windows 盘符路径、UNC 网络路径以及部分 Shell 地址，并自动迁移已有的地址记录
- 文本、链接、资源管理器地址和文件路径均可使用鼠标拖选；选中文字时不会误触整条记录的粘贴
- 链接支持按住 `Ctrl` 后点击，使用系统默认浏览器打开；按住 `Ctrl` 悬停时会显示下划线和指针反馈
- 分类栏支持两行排列，避免窗口较窄时后方分类不可见
- 新增平滑回到顶部按钮，同时保留原有的滚动位置记忆

### 图片体验与处理

- 图片缩略图由 `200px` 增大到 `264px`，更容易辨认剪切内容
- 鼠标悬停图片 `300ms` 后才显示大图，避免划过列表时预览瞬间弹出
- 全图预览最长边限制为 `1600px`，并减少全图缓存数量，降低大图导致的内存和界面压力
- 支持设置图片最大尺寸和压缩质量
- 对超过 10 MB 的大图，可选择自动压缩、保留原图或不记录
- 图片读取、缩放和缩略图生成使用异步任务，减少主窗口卡顿

### 粘贴与窗口交互

- 粘贴前同时保存目标应用的顶层窗口和实际输入控件，窗口隐藏后恢复到原输入位置
- 增强 Windows 焦点恢复与等待机制，改善资源管理器地址栏等子控件中的粘贴准确性
- 发送粘贴前等待 `Ctrl`、`Alt` 和 `Win` 修饰键释放，避免快捷键残留影响输入
- 未置顶时，点击窗口外部会自动隐藏；窗口置顶后保持显示
- 拖动区域改用稳定的原生窗口拖动路径，拖动过程中不再误关窗口，也避免频繁 WebView IPC

### 存储、备份与通知

- 可分别设置最大历史数量和最大存储空间，并在设置页查看记录数、收藏数和空间占用
- 支持导出和导入设置及收藏记录，收藏图片会一并写入备份
- 清理数据库记录时同步清理未引用的图片和缩略图，同时保护仍在使用的文件
- 支持可选的新剪切板系统通知，侧边栏和托盘可显示未读数量
- 托盘菜单展示最近 8 条剪切板记录，可直接执行复制或粘贴

### 稳定性与快捷键说明

- 托盘菜单刷新使用独立工作线程，并合并短时间内的重复刷新请求
- 修复图片清理期间持有数据库锁造成的潜在死锁
- 去除了容易造成卡死和 Windows 开始菜单误弹的低级 `Win+V` 键盘钩子，恢复使用 Tauri 全局快捷键
- 因为 `Win+V` 是 Windows 保留组合键，本项目不会在底层强制替换系统剪切板；建议在设置中使用一个不与系统冲突的全局快捷键

## 主要功能

### 📋 剪切板管理
- 自动记录文本、链接、资源管理器地址、文件和图片的复制历史
- 支持关键词搜索，快速定位历史内容
- 支持分类筛选、收藏、文字拖选和 `Ctrl+点击` 打开链接
- 一键粘贴到原输入位置，并针对 Windows 子窗口焦点进行恢复
- 可设置保留时长、最大历史数量和存储空间，自动清理过期记录
- 支持图片尺寸、压缩质量和大图处理策略

### ⚡ 快捷短语
- 按场景分组管理常用话术和代码片段
- 支持自定义分组，灵活组织内容
- 点击即粘贴，无需手动复制

### 🌐 翻译
- **AI 翻译**：兼容 OpenAI API 格式，可自定义端点和模型
- **内置翻译**：免费翻译服务，开箱即用
- 翻译结果本地缓存，避免重复请求

### ⚙️ 系统功能
- 全局快捷键唤起/隐藏窗口
- 未置顶时失焦自动隐藏，置顶后保持显示
- 亮色/暗色主题切换
- 开机自启动
- 托盘最近记录、未读角标和可选剪切板通知
- 设置与收藏数据导入导出

## 技术栈

| 层级 | 技术选型 |
|:---:|:---|
| 桌面框架 | [Tauri 2.x](https://tauri.app/) (Rust) |
| 前端框架 | React 19 + TypeScript |
| 构建工具 | [Vite](https://vitejs.dev/) |
| UI 样式 | 纯 CSS（iOS 风格磨砂玻璃效果） |
| 状态管理 | [Zustand](https://zustand-demo.pmnd.rs/) |
| 本地存储 | SQLite (rusqlite, bundled) |
| 国际化 | react-i18next（简体中文 / English） |

## 下载安装

优先前往本 Fork 的 [Releases](https://github.com/puppnn/copy-creator/releases) 页面查看定制版安装包。若暂未发布对应安装包，可按照下方开发指南从源码构建。

上游原版安装包仍可从 [hu-qi-jia/copy-creator Releases](https://github.com/hu-qi-jia/copy-creator/releases) 下载。

| 安装包 | 说明 |
|:---|:---|
| `Copy Creator_x64-setup.exe` | NSIS 安装包 |
| `Copy Creator_x64_zh-CN.msi` | MSI 安装包（中文） |

**系统要求**：Windows 11

## 操作说明

### 基本使用

1. **启动应用**：安装后双击桌面图标启动，应用将以悬浮窗形式显示
2. **驻留托盘**：关闭窗口后，应用会自动最小化到系统托盘，继续在后台运行
3. **唤起窗口**：使用全局快捷键（默认可在设置中查看）快速唤起/隐藏窗口

### 剪切板功能

1. 复制任意文本或图片，系统会自动记录到剪切板历史
2. 点击托盘图标或使用快捷键打开主窗口
3. 切换到「剪切板」标签页，浏览或搜索历史记录
4. 点击任意记录即可一键粘贴到当前光标位置

### 快捷短语功能

1. 切换到「短语」标签页
2. 点击「新建分组」创建场景分组（如：客服话术、代码片段等）
3. 在分组中添加常用短语
4. 需要使用时，点击短语即可粘贴到当前输入位置

### 翻译功能

1. 切换到「翻译」标签页
2. 输入或粘贴需要翻译的文本
3. 选择翻译方向（如：中文 → 英文）
4. 点击翻译按钮获取结果
5. 如需使用 AI 翻译，请在设置中配置 API 端点和密钥

### 个性化设置

- **快捷键**：自定义全局快捷键
- **主题**：切换亮色/暗色主题
- **开机自启**：设置是否开机自动启动
- **存储管理**：配置保留时长、最大历史数量和存储空间
- **图片处理**：配置最大尺寸、压缩质量和大图处理策略
- **数据迁移**：导入或导出设置与收藏记录

## 开发指南

### 环境准备

- [Node.js](https://nodejs.org/) (推荐 18+)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/)
- [Tauri CLI](https://tauri.app/)

### 本地开发

```bash
# 克隆项目
git clone https://github.com/puppnn/copy-creator.git
cd copy-creator/copy-creator

# 安装依赖
pnpm install

# 启动开发模式
pnpm tauri dev

# 构建生产版本
pnpm tauri build
```

## 项目结构

```
copy-creator/
├── src/                    # 前端源码
│   ├── components/         # React 组件
│   ├── pages/              # 页面组件
│   ├── stores/             # Zustand 状态管理
│   ├── styles/             # CSS 样式文件
│   ├── i18n/               # 国际化配置
│   └── types/              # TypeScript 类型定义
├── src-tauri/              # Tauri 后端源码
│   ├── src/                # Rust 源码
│   └── Cargo.toml          # Rust 依赖配置
├── public/                 # 静态资源
└── package.json            # 前端依赖配置
```

## 许可证

本项目采用 [MIT 许可证](LICENSE) 开源。

---

<div align="center">

如果觉得这个项目对你有帮助，欢迎点个 Star 支持一下！

本项目基于 [hu-qi-jia/copy-creator](https://github.com/hu-qi-jia/copy-creator) 开发，感谢原作者及 baihejiangnan 的贡献！


</div>
