# Skills Manager

统一管理 AI Agent Skills 的桌面应用。

[English README](./README.md)

## 项目简介

Skills Manager 是一个基于 Tauri 的桌面应用，用来集中管理不同 AI 编码工具的 skills。它会在本地维护一个统一仓库 `~/.skills-manager`，用 SQLite 记录已管理的技能，并通过 scenario 切换不同的技能组合。

当前仓库已经实现的能力：

- 从本地目录、`.zip` / `.skill` 文件、Git 仓库和 `skills.sh` 市场安装技能
- 扫描各工具现有的 skills 目录，并导入到中央仓库
- 通过软链接或复制的方式，把技能同步到目标工具，或取消同步
- 使用 scenario 对技能分组，并切换当前激活场景
- 为 Git 类技能检查更新，并对本地导入类技能执行重新导入
- 预览技能文档，支持 `SKILL.md`、`README.md` 等常见入口文件
- 管理语言、主题、默认场景、同步模式等设置

## 当前支持的工具

目前内置了以下工具的检测与同步适配：

- Cursor
- Claude Code
- Codex
- OpenCode
- Antigravity
- Amp
- Kilo Code
- Roo Code
- Goose
- Gemini CLI
- GitHub Copilot
- Clawdbot
- Droid
- Windsurf
- TRAE IDE

工具是否已安装，当前是通过用户目录下对应配置目录是否存在来判断。

## 工作方式

应用会先把所有技能整理到本地中央仓库：

- 中央仓库：`~/.skills-manager`
- 技能目录：`~/.skills-manager/skills`
- 场景目录：`~/.skills-manager/scenarios`
- 缓存和日志：`~/.skills-manager/cache`、`~/.skills-manager/logs`
- 数据库：`~/.skills-manager/skills-manager.db`

当你把某个技能同步到某个工具时，应用会按照设置把该技能写入目标工具的 skills 目录，方式可能是软链接，也可能是复制。

## 技术栈

- 前端：React 19、TypeScript、Vite、Tailwind CSS
- 桌面容器：Tauri 2
- 后端：Rust
- 存储：SQLite（`rusqlite`）
- 国际化：`react-i18next`

## 目录结构

```text
.
├── src/                # React 前端
├── src-tauri/          # Tauri 与 Rust 后端
├── public/             # 静态资源
├── docs/               # 补充文档/素材
└── README.md           # 英文 README
```

## 开发

### 前置依赖

- Node.js 18+
- npm
- Rust toolchain
- 当前操作系统所需的 Tauri 依赖

### 安装依赖

```bash
npm install
```

### 开发模式运行

```bash
npm run tauri:dev
```

如果只需要前端开发：

```bash
npm run dev
```

### 构建

```bash
npm run tauri:build
```

只构建前端：

```bash
npm run build
```

### 代码检查

```bash
npm run lint
```

## 主要界面

- Dashboard：查看当前场景、已同步技能数量、工具安装状态
- My Skills：管理已纳入中央仓库的技能，支持筛选、同步、更新、删除、切换场景启用状态
- Install Skills：浏览 `skills.sh`、从 Git 安装、从本地安装、扫描并导入已有技能
- Settings：查看工具状态、打开中央仓库、设置同步模式、主题、语言、当前场景和默认场景

## 说明

- 本地安装和扫描导入的技能会标记为 `local_only`，更新方式是重新导入，而不是拉取 Git 更新。
- 通过 Git 和 `skills.sh` 安装的技能会保留来源信息，便于后续检查远端版本。
- 应用启动时会优先恢复默认场景，并自动同步该场景关联的技能。
- 如果历史上存在 `~/.agent-skills`，应用会尝试迁移到 `~/.skills-manager`。

## License

MIT
