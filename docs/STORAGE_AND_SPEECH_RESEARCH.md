# RambleDesk 大体积存储与语音资源管理调研

> 状态：已评审接受，进入实施
> 日期：2026-08-02  
> 范围：数据库、草稿附件、已发布反馈包、语音模型、配置与凭证的路径策略；为后续麦克风与模型管理提供前置决策。

## 1. 结论摘要

建议采用 **“固定状态目录 + 可移动资料库目录”双根模型**，不要让用户直接移动整个 App Data，也不要分别暴露数据库、反馈包、模型三个互不相关的路径。

### 固定状态目录（不提供移动）

继续使用操作系统标准的应用本地数据目录，存放：

- `settings.json`：包括资料库位置这个“引导配置”；
- SQLite 数据库及 WAL/SHM；
- loopback token 等凭证；
- 小体积日志与恢复元数据。

### 可移动资料库目录（设置中可选）

存放真正可能占用大量空间的内容：

- 活跃反馈的草稿附件；
- 已发布、不可变的 Feedback Packages；
- 下载后的语音模型；
- 模型下载临时文件与解压暂存目录。

推荐默认目录仍由系统决定，但所有平台都提供 **设置 → 通用 → 存储 → 资料库位置**。Windows 用户可选择 D/E 盘；macOS/Linux 用户也保留同一高级能力，避免平台分叉。

## 2. 当前实现盘点

### 2.1 当前默认路径

Rust `dirs::data_local_dir()` 的平台标准位置为：

| 平台 | 根目录 |
| --- | --- |
| Windows | `%LOCALAPPDATA%`，例如 `C:\Users\Alice\AppData\Local` |
| macOS | `~/Library/Application Support` |
| Linux | `$XDG_DATA_HOME`，缺省为 `~/.local/share` |

RambleDesk 当前在该根目录下使用 `RambleDesk/`。

### 2.2 当前各类数据的实际归属

| 数据 | 当前来源 | 当前行为 |
| --- | --- | --- |
| SQLite | `crates/rambledesk-storage::default_database_path()` | `<local-data>/RambleDesk/feedback.sqlite3` |
| loopback token | `rambledesk-local-server::default_token_path()` | `<local-data>/RambleDesk/auth/local-server.token` |
| 草稿附件 | `SqliteFeedbackStore.app_data_root` | `<数据库父目录>/drafts/<request-id>/attachments/` |
| 已发布反馈包 | `SqliteFeedbackStore.app_data_root` | `<数据库父目录>/feedback/<timestamp>-<request-id>/` |
| Sherpa 模型 | Tauri `app_local_data_dir()` | `<app-local-data>/models/sherpa-x-asr/` |
| 路径覆盖 | 环境变量 | 数据库、token、模型分别可被环境变量覆盖，但 UI 不可配置 |

关键耦合：`SqliteFeedbackStore::connect(database_path)` 会把 **数据库父目录同时当作草稿和反馈包根目录**。因此当前设置 `RAMBLEDESK_DATABASE_FILE=D:\...\feedback.sqlite3` 会隐式移动数据库、草稿附件和反馈包，却不会移动语音模型与 token。这不是一个清晰的用户级存储合同。

### 2.3 当前体积来源

- Markdown 和 SQLite 通常不是主要空间来源；截图、导入文件和模型才是。
- 单请求最多 20 个附件，每个最多 20 MiB，理论上仅草稿附件可达 400 MiB。
- 发布反馈包时会把附件复制进不可变 package；若草稿附件未清理，同一请求理论上可再占一份，接近 800 MiB。
- 当前 X-ASR 模型清单：下载包约 133.9 MB，主要模型文件解压后约 169.3 MB。若下载包、解压临时目录和最终模型同时存在，安装过程至少应预留约 350–500 MB。
- 当前开发机的 RambleDesk 目录很小，但已经同时存在历史 `rambledesk.sqlite3` 与当前 `feedback.sqlite3`，说明早期开发阶段直接清理旧数据比引入兼容迁移更合理。

## 3. 外部产品做法

### Jan

Jan 把数据组织成一个可理解的 Data Folder，并在 **Settings → General → Jan Data Folder** 中允许自定义。优点是用户心智简单，适合本地 AI 大文件；缺点是把数据库、凭证和可再下载模型放进同一个可移动目录后，磁盘离线与权限问题会同时影响整个应用。

### LM Studio

LM Studio 内置模型下载器，并允许在 My Models 中修改 models directory。它把最占空间、可重新下载的模型单独视为资源。这证明“模型目录可选”符合用户预期，但 RambleDesk 还有不可替代的反馈包，不能只解决模型。

### Ollama

Ollama 通过 `OLLAMA_MODELS` 环境变量更换模型目录。它技术上有效，但 Windows 普通用户需要编辑环境变量并重启应用，正是 RambleDesk 当前模型路径方式应避免的体验。

### Docker Desktop

Docker Desktop 在设置中暴露 Disk image location，把大体积容器/镜像存储作为高级资源位置，同时保留应用自身配置。这与“双根模型”最接近。

### 综合判断

- Jan 证明“一个资料库目录”容易理解；
- LM Studio/Ollama 证明模型需要可移动；
- Docker Desktop 证明大体积数据与应用状态分离更稳健；
- RambleDesk 应组合这些做法：**用户只选择一个大体积资料库目录，但数据库、配置与凭证仍留在标准状态目录。**

## 4. 方案比较

### 方案 A：整个 RambleDesk 数据目录可移动

包括数据库、token、附件、反馈包、模型。

优点：概念最简单；备份整个目录即可。  
缺点：外置盘掉线会导致数据库无法启动；token 权限更难保证；SQLite/WAL 在网络盘或不可靠文件系统上风险高；“资料库位置”配置本身不能只存在于被移动的目录里。

结论：不推荐。

### 方案 B：固定状态目录 + 单一可移动资料库目录

优点：数据库与凭证稳定；用户只理解一个大文件位置；反馈包和模型都可离开 C 盘；适合后续显示容量与清理。  
缺点：备份时有两个根；需要把数据库中的绝对文件路径改为相对资料库路径或稳定 URI。

结论：**推荐。**

### 方案 C：反馈、模型、数据库分别可配置

优点：最大灵活性。  
缺点：设置复杂，测试组合爆炸；普通用户难以判断该改哪一个；容易形成跨盘临时复制与空间估算问题。

结论：不作为首版；未来可在开发者高级设置中保留环境变量覆盖。

## 5. 推荐目录合同

```text
<system-local-app-data>/RambleDesk/       # 固定状态目录
├── settings.json                         # 包含 library_root
├── state/
│   └── feedback.sqlite3                  # WAL/SHM 同目录
├── auth/
│   ├── local-server.token
│   └── mcp.token
├── logs/
└── recovery/                             # 小体积迁移/恢复元数据

<library-root>/RambleDesk Library/        # 用户可选的大体积资料库
├── drafts/<request-id>/attachments/
├── feedback/<timestamp>-<request-id>/
├── models/speech/<model-id>/
├── downloads/                            # .part 与待校验压缩包
└── staging/                              # 同根原子发布/解压暂存
```

### 路径记录原则

数据库不再保存依赖当前盘符的绝对路径作为事实主键。建议保存：

- `drafts/<request-id>/attachments/<file>` 这类资料库相对路径；或
- `rambledesk-library://drafts/...`、`rambledesk-library://feedback/...` 稳定 URI。

API 返回给同机宿主时，再由当前 `library_root` 解析为绝对路径。这样 D 盘改成 E 盘后无需逐行改数据库，也避免 package manifest 被旧盘符污染。

反馈包自身仍应使用相对附件路径；这是当前 manifest 已经采用的正确方向。

## 6. 设置页建议

### 通用 → 存储

显示：

- 当前资料库绝对路径；
- 当前占用空间（草稿、反馈包、模型分别统计）；
- “更改位置…”；
- “在文件管理器中打开”；
- “清理下载缓存”；
- 路径不可用、空间不足、只读或位于网络盘时的明确状态。

首版更改位置对话框可提供：

1. **以后使用新位置（推荐用于当前开发阶段）**：旧数据不移动；若决定不兼容，可要求确认清空旧开发数据。
2. 后续再增加 **移动现有资料库**：复制、hash 校验、原子切换配置，成功后询问是否删除旧目录。

不要在选完目录后静默回退。如果资料库盘符离线，应进入可恢复的降级页，要求重新连接或重新定位；静默创建默认 C 盘目录会造成数据分叉。

### 语音

在存储合同完成后增加：

- 麦克风输入设备列表、默认设备、测试电平；
- 当前模型状态：未安装/下载中/可用/损坏；
- 下载、暂停/重试、hash 校验、删除；
- 模型大小与资料库剩余空间；
- 模型实际路径只读展示，路径由资料库设置统一控制，不再要求用户配置环境变量。

麦克风设备选择本身不依赖存储，可与模型管理并行开发；但“下载模型”应等待资料库根合同落定。

## 7. 可靠性与安全要求

- SQLite 与 token 不放网络共享或可随时拔出的盘。
- 自定义资料库必须是绝对、本机、可写目录；保存 canonical path。
- 下载使用 `.part`，完成后校验 manifest 中的 byte size 与 SHA-256，再解压到 staging，最后同根 rename 发布。
- 更改资料库时禁止同时进行录音、截图导入、反馈提交或模型下载。
- 发布反馈包继续保持不可变；移动过程不得修改 package 内容与 hash。
- 切换前检查目标空间，至少覆盖“现有资料库大小 + 正在安装模型所需 staging 空间”。
- 权限失败、盘符丢失和恢复不得记录正文、token 或附件内容到日志。

## 8. 建议实施顺序

### Phase 1：路径合同与早期数据重置

1. 引入 `RambleDeskPaths { state_root, library_root }`。
2. 在固定状态目录写 `settings.json`，其中保存 `library_root`。
3. Storage 显式接收 `database_path` 与 `library_root`，取消“数据库父目录就是所有数据根”的隐式规则。
4. 数据库中的附件/反馈路径改为资料库相对路径或稳定 URI。
5. 当前处于早期开发阶段：更新 baseline schema 后清理本地旧数据库与旧 feedback/drafts，不写兼容迁移。

### Phase 2：存储设置 UI

1. 展示默认路径与分类占用。
2. 选择新目录并做可写性/空间验证。
3. 首版只支持新数据使用新位置，并明确旧数据处理。
4. 增加打开目录与清理缓存。

### Phase 3：语音资源管理

1. 枚举麦克风、选择并测试。
2. 内置 X-ASR manifest 下载器。
3. 下载进 `library_root/downloads`，校验后发布到 `models/speech/<model-id>`。
4. 去掉普通用户对 `RAMBLEDESK_SHERPA_MODEL_DIR` 的依赖；环境变量仅保留开发覆盖。

### Phase 4：正式迁移能力

产品稳定后再加入跨盘复制、校验、断点恢复和旧目录清理；不应为当前开发数据提前承担这部分复杂度。

## 9. 已确认的产品决策

1. 接受“固定状态目录 + 可移动资料库目录”双根模型。
2. UI 名称使用“数据存储位置”。
3. 当前开发阶段切换目录允许清空旧数据；设置中后续提供显式清空按钮。
4. 已发布 Feedback Package 默认永久保留，后续补充删除与归档管理。
5. package 成功发布后自动回收草稿附件副本，避免长期双份占用。
6. 先实现路径合同，再实现麦克风选择与语音模型管理。
7. 资料库不可用时仍建议只读浏览数据库元数据，并禁止产生新写入。

## 10. 参考资料

- Rust `dirs::data_local_dir` 平台目录表：<https://docs.rs/dirs/latest/dirs/fn.data_local_dir.html>
- Tauri `appLocalDataDir`：<https://v2.tauri.app/reference/javascript/api/namespacepath/#applocaldatadir>
- Jan Data Folder（可在 Settings → General 自定义）：<https://jan.ai/docs/desktop/data-folder>
- LM Studio 内置下载器与 models directory：<https://lmstudio.ai/docs/app/basics/download-model#changing-the-models-directory>
- Ollama `OLLAMA_MODELS`：<https://docs.ollama.com/faq>
- Docker Desktop Disk image location：<https://docs.docker.com/desktop/settings-and-maintenance/settings/#advanced>
- RambleDesk 当前路径实现：`apps/desktop/src-tauri/src/config.rs`、`crates/rambledesk-storage/src/sqlite.rs`、`crates/rambledesk-storage/src/sqlite/workspace_ops.rs`
- 当前语音模型清单：`crates/rambledesk-speech/models/sherpa-x-asr.json`
