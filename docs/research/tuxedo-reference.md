# Tuxedo 参考项目技术调研

> 调研对象：本地 `tuxedo/`，提交 `8c990c0e1f57462115c0d2dffdfffb3f0b63b7db`，上游为 `webstonehq/tuxedo`。  
> 调研日期：2026-07-26。  
> 目标：为新的 Rust/Ratatui 番茄钟 TUI、共享 CLI 和 Waybar 输出提取可复用的架构经验。  
> 本文把可从源码直接确认的内容标为「事实」，把针对新项目的取舍标为「建议」。

## 结论摘要

Tuxedo 最值得借鉴的不是 todo.txt 功能，而是它的边界：一个无界面的 `Store` 承担持久状态与业务变更，`App` 叠加瞬时交互状态，`ui` 只渲染，TUI 和一次性 CLI 共用核心。入口先识别子命令，否则进入 TUI；这种单二进制体验非常适合本项目。[`src/core/mod.rs`](../../tuxedo/src/core/mod.rs) [`src/cmd/mod.rs`](../../tuxedo/src/cmd/mod.rs) [`src/main.rs`](../../tuxedo/src/main.rs)

但番茄钟与 todo 列表有一个根本差异：时间必须在 TUI 退出后仍然正确，而且 TUI、CLI 与 Waybar 可能并发读写同一状态。因此不应原样复制 Tuxedo 的“无 daemon、每个进程独立打开文件”模型。首版也不一定需要 daemon：保存绝对截止时间并在每次读取时推导剩余时间，已经能让关闭 TUI 后计时继续；并发写入则需要锁或带版本号的事务。只有要保证无人查询时也能准点发通知、自动推进周期，才需要常驻服务。

## 1. 技术栈与模块边界

### 事实

- 项目使用 Rust 2024；直接依赖保持很小：`anyhow`、`chrono`、`crossterm`、`ratatui`、文件监听 `notify`，以及分享功能需要的 `qrcode`、`tiny_http` 等。发布配置启用 strip、LTO、单 codegen unit、`panic = abort` 和体积优化。[`Cargo.toml`](../../tuxedo/Cargo.toml)
- `lib.rs` 暴露业务、应用、配置、命令和 UI 模块，二进制入口在 `main.rs`；这让集成测试和示例能直接构造 `App` 并渲染真实 UI。[`src/lib.rs`](../../tuxedo/src/lib.rs) [`tests/snapshots.rs`](../../tuxedo/tests/snapshots.rs)
- `core::Store` 明确声明为 headless core：拥有任务、归档、历史、路径及磁盘快照；不持有视图、输入或呈现状态，变更返回结构化 outcome。TUI 通过 `App` 包装它，CLI 直接驱动它。[`src/core/mod.rs`](../../tuxedo/src/core/mod.rs)
- `App` 聚合 `Store` 与 TUI 特有状态：mode、view、偏好、cursor、draft、selection、flash、chord、可见项缓存、异步接收器等；子模块把草稿、选择、偏好、弹层等拆开。[`src/app/mod.rs`](../../tuxedo/src/app/mod.rs)
- `ui` 目录按屏幕区域和弹层拆分，如 list、detail、status、help、settings、command palette；顶层 `ui::draw` 只按 `App` 当前状态布局并调用各 renderer。[`src/ui/mod.rs`](../../tuxedo/src/ui/mod.rs)
- 键位的语义用 `Action` 枚举表达，实际事件解释集中在入口的 mode-specific handlers；输入编辑行为还被归一为 `EditAction`，供搜索、提示框和命令面板共用。[`src/action.rs`](../../tuxedo/src/action.rs) [`src/main.rs`](../../tuxedo/src/main.rs)

### 建议

为番茄钟采用相同的三层边界，但进一步缩小接口：

```text
timer-core
  TimerState + Command -> Transition
  reconcile(now), remaining(now), next_deadline()
  不读取环境变量、不打印、不依赖 Ratatui

storage
  load/save/lock/migrate
  XDG 路径与原子写入

frontends
  TUI: App + update + render
  CLI: 解析命令、调用同一 Transition、格式化 text/json/waybar
  notifier/daemon（可选）: 等待 next_deadline 并推进状态
```

核心命令可从 `StartFocus { task }`、`Pause`、`Resume`、`Skip`、`Stop` 开始；核心返回结构化 `Transition`（新状态、是否完成一个 session、建议通知），前端负责措辞和退出码。不要让 `TimerState` 持有 Ratatui widgets、终端 mode 或 `Instant`。

## 2. 事件循环、状态更新与渲染

### 事实

- Tuxedo 以 250 ms 为默认轮询周期，用 `dirty` 控制是否重绘；每轮先处理日期变化、后台归档、更新检查与配置热重载，再在 dirty 时调用 `ui::draw`。[`src/main.rs`](../../tuxedo/src/main.rs)
- `event::poll(timeout)` 等待输入；按键只处理 `Press`，resize 强制重绘。空闲超时用于检查外部文件变化，并清理到期的 flash/chord。[`src/main.rs`](../../tuxedo/src/main.rs)
- `next_timeout` 会取 flash 和 chord 的最近 deadline，但上限仍是 250 ms，避免计时器无谓忙循环。[`src/main.rs`](../../tuxedo/src/main.rs)
- 事件解释根据 `Mode` 分派到 insert/search/help/settings/normal 等 handler；handler 更新 `App`，renderer 只读取 `&App`。[`src/main.rs`](../../tuxedo/src/main.rs) [`src/ui/mod.rs`](../../tuxedo/src/ui/mod.rs)
- Ratatui 初始化和恢复由入口包住运行循环；打开外部编辑器时先恢复终端，返回后重新启用 raw mode 和 alternate screen。[`src/main.rs`](../../tuxedo/src/main.rs)

### 建议

- 采用显式的 `update(app, Event, now) -> Effects` 与 `render(frame, &app, now)`。Tuxedo 已有 update/render 分离的方向，但按键 handler 仍集中在很大的 `main.rs`；新项目应及早把事件映射移入 `tui/update.rs`。
- 用 wall-clock 绝对时间（如 Unix 毫秒或带时区无关的 UTC timestamp）持久化 `started_at` / `ends_at`。`Instant` 仅适合当前进程内调度，不能序列化，也不能跨重启。
- 运行中的 UI 不必固定 250 ms 重绘。根据 `next_second_boundary`、最近状态 deadline 和输入事件计算 timeout；显示到秒时通常每秒重绘即可，动画另行以较高帧率启用。
- 把时间作为参数注入核心和测试。任何 `remaining()`、自动推进、统计归档都不应直接调用全局“现在”，这样可确定性测试暂停、跨午夜、休眠唤醒和系统时间变化。
- TUI 的 tick 必须先从共享存储 reconcile，再派生显示。CLI 写入后，TUI 下一 tick 应能看到；Waybar 则每次执行读取同一事实源。

## 3. CLI 与 Waybar

### 事实

- 单一二进制先尝试识别 CLI 子命令；识别成功则执行并退出，否则进入 TUI。全局参数可出现在子命令前。[`src/main.rs`](../../tuxedo/src/main.rs) [`src/cmd/mod.rs`](../../tuxedo/src/cmd/mod.rs)
- CLI 直接同步打开 headless `Store`，映射命令到核心 mutation，并把用户错误、用法错误区分为退出码 1 和 2。[`src/cmd/mod.rs`](../../tuxedo/src/cmd/mod.rs)
- `--json` 是同一命令面的机器输出模式；README 明确承诺 JSON 模式无交互提示和 footer。[`README.md`](../../tuxedo/README.md) [`src/cmd/json.rs`](../../tuxedo/src/cmd/json.rs)
- Waybar 官方 custom module 支持周期执行命令、`return-type: "json"` 和 CSS `class`；也可配置 signal 触发立即刷新。[Waybar 官方仓库](https://github.com/Alexays/Waybar)（其 custom module 文档入口）及[官方 issue 中的配置示例](https://github.com/Alexays/Waybar/issues/1595)。

### 建议

- 保留“无子命令打开 TUI”的体验，CLI 首版提供：
  `start [TASK]`、`pause`、`resume`、`skip`、`stop`、`status`、`toggle`。
- `status` 默认输出适合人读的一行；`status --json` 输出稳定的领域 JSON；另设 `status --waybar` 输出 Waybar 协议，避免把通用 JSON schema 和 Waybar schema 绑死。
- 建议的 Waybar 输出：

```json
{
  "text": "󰔟 18:42",
  "tooltip": "Focus · 写计时核心\n第 2/4 轮",
  "class": ["running", "focus"],
  "percentage": 74
}
```

- Waybar 可每秒执行 `app status --waybar`。点击命令可调用 `toggle` 或 `skip`，随后向 Waybar 发配置好的 real-time signal 立即刷新。CLI 必须把 JSON 独占 stdout，诊断写 stderr。
- `class` 至少稳定区分 `idle`、`running`、`paused`、`focus`、`short-break`、`long-break`、`overdue/error`，方便 CSS。
- 不要让 Waybar 进程成为计时器拥有者。它只是读取/控制前端；Waybar 重启不应重置时间。

## 4. 持久化、并发与 XDG

### 事实

- Tuxedo 的任务写入采用同目录临时文件后 rename；每次 mutation 前读取磁盘并与 `last_disk` 比较，若外部变化则 reload、清空 undo 并中止当前 mutation，避免覆盖未知内容。[`src/todo.rs`](../../tuxedo/src/todo.rs) [`src/core/external.rs`](../../tuxedo/src/core/external.rs)
- 配置位于 `${XDG_CONFIG_HOME:-$HOME/.config}/tuxedo/config.toml`；相对的 `XDG_CONFIG_HOME` 被忽略。配置保存通常使用带 PID/计数器的唯一临时文件再 rename，避免并发 writer 共享一个 tmp 名。[`src/config.rs`](../../tuxedo/src/config.rs) [`src/xdg.rs`](../../tuxedo/src/xdg.rs)
- 配置 parser 忽略未知 key，便于前向兼容；启动加载失败回退默认，热重载则 strict parse，失败时保留旧配置。[`src/config.rs`](../../tuxedo/src/config.rs) [`src/main.rs`](../../tuxedo/src/main.rs)
- 配置 watcher 监听父目录而非单一文件，因此能观察 atomic rename；事件经过 200 ms debounce。[`src/config_watcher.rs`](../../tuxedo/src/config_watcher.rs)

### 建议

按 XDG 数据性质拆分：

- `$XDG_CONFIG_HOME/<app>/config.toml`：周期长度、长休间隔、主题、键位、通知偏好。
- `$XDG_STATE_HOME/<app>/state.json`：当前阶段、绝对开始/结束时间、暂停剩余量、当前任务、revision。
- `$XDG_DATA_HOME/<app>/history.jsonl` 或 SQLite：已完成 session 历史。
- `$XDG_RUNTIME_DIR/<app>.sock` / lock：仅 daemon 或进程间协调需要。

状态写入使用唯一 tmp + flush/sync（按耐久性目标决定）+ rename；不能照搬 `path.with_extension("tmp")`，因为 TUI、CLI 和 Waybar 点击可能并发。写事务应持有跨进程排他锁，并在锁内重新 load、reconcile、apply、save。`fs2::FileExt` 提供阻塞与非阻塞文件锁 API，可作为实现候选。[fs2 官方 rustdoc](https://docs.rs/fs2/latest/fs2/trait.FileExt.html)

首版状态 schema 应带 `schema_version` 和单调递增 `revision`。损坏状态文件不应被静默当 idle 后覆盖；应返回可见错误并保留原文件。配置损坏可以回退默认，但运行状态损坏的风险更高。

## 5. 主题、组件与漂亮 TUI

### 事实

- Tuxedo 把完整 palette 定义为 `Theme`，renderer 从 `app.theme()` 取得语义色，而非散落 RGB；内置主题含 RGB 主题与继承终端背景的 Terminal 主题。[`src/theme.rs`](../../tuxedo/src/theme.rs)
- 用户主题从 XDG config 下的 `themes/*.toml` 加载，按文件名排序；字段缺失、颜色非法或重名时跳过并给出警告。[`src/theme.rs`](../../tuxedo/src/theme.rs) [`README.md`](../../tuxedo/README.md)
- `ui::draw` 先画全屏背景，再划分 body/status、左右侧栏/中心区，最后按 mode 叠加 modal；组件拥有自己的 render 函数。[`src/ui/mod.rs`](../../tuxedo/src/ui/mod.rs)
- 布局密度、侧栏和状态栏是 `Prefs`，不是写死在 renderer 中；窄屏尺寸通过 `min`/`clamp` 收缩。[`src/app/prefs.rs`](../../tuxedo/src/app/prefs.rs) [`src/ui/mod.rs`](../../tuxedo/src/ui/mod.rs)

### 建议

- 复用“语义色 token + 小 renderer + overlay 最后绘制”的方法，不复制 Tuxedo 的具体 palette、logo、布局比例或视觉识别。
- 番茄钟主屏应围绕三个核心组件：大号剩余时间、阶段/轮次、当前任务。任务队列、今日统计和快捷键提示是次级区域；窄屏时按优先级隐藏，而不是压缩到不可读。
- 将 `TimerFaceProps`、`SessionProgressProps`、`TaskQueueProps` 作为纯渲染输入，避免组件直接读取整个 `App`。这比 Tuxedo 的多数 renderer 接收 `&App` 更容易快照测试和复用。
- Unicode/Nerd Font 图标必须有普通 Unicode 或 ASCII fallback；Waybar 与终端字体配置可能不同。
- 动画只是表现层：进度脉冲、番茄图案或完成庆祝不能驱动业务时间，也不能要求高频刷新才能保证状态正确。

## 6. 测试与视觉回归

### 事实

- Tuxedo 有大量靠近实现的单元测试，核心通过 test-support 构造临时 Store，路径解析的纯决策也被从 I/O 中拆出测试。[`src/core/test_support.rs`](../../tuxedo/src/core/test_support.rs) [`src/cli.rs`](../../tuxedo/src/cli.rs)
- UI 集成测试使用 Ratatui `TestBackend` 以固定 100×32 渲染真实 `ui::draw`，每个场景同时保存字符网格 snapshot 和含前景/背景/修饰符的 styled snapshot；使用 `insta` 审核变化。[`tests/snapshots.rs`](../../tuxedo/tests/snapshots.rs)
- README 截图由同样的 `TestBackend` 产生，再把 buffer 转成 SVG，因此文档视觉与实际 renderer 同源。[`examples/screenshots.rs`](../../tuxedo/examples/screenshots.rs)

### 建议

测试金字塔：

1. 核心属性/表格测试：每个 `Command × Phase` 的合法性；剩余时间；暂停/恢复；skip；第 4 轮长休；跨重启 reconcile。
2. 虚拟时钟场景：机器 suspend 后醒来、跨午夜、时钟向前/向后跳、deadline 恰好相等。
3. 存储测试：原子写、schema migration、损坏文件、并发 CLI 进程、锁超时、revision 冲突。
4. CLI golden tests：stdout/stderr/exit code；`--json` 与 `--waybar` schema。
5. TUI snapshots：至少覆盖 idle/focus/break/paused/completed、窄/宽终端、每个内置主题；文字和样式分别 snapshot。

固定 snapshot 中的时间、路径、尺寸和 locale，避免 CI 抖动。把系统通知封装为 port，在测试中记录 effect，不实际发 D-Bus 通知。

## 7. 通知与后台状态

### 事实

- Tuxedo 自己明确选择“no daemon”；其后台线程只服务于当前 TUI 进程内的归档读取、升级检查、配置监听和临时分享服务器，TUI 退出后这些线程不存在。[`README.md`](../../tuxedo/README.md) [`src/main.rs`](../../tuxedo/src/main.rs)
- `notify-rust` 是面向桌面通知的 Rust 库，在 XDG 平台可通过 D-Bus 显示 notification，并支持 summary、body、icon、timeout、action 等；具体能力受 notification server 影响。[notify-rust 官方 rustdoc](https://docs.rs/notify-rust/latest/notify_rust/)

### 建议

先明确两种产品承诺：

- **推导式、无 daemon MVP**：start 时持久化 `ends_at`；TUI、CLI、Waybar 每次读取时 reconcile 到正确阶段。优点是部署简单；缺点是所有前端关闭时无法在 deadline 主动通知，也无法准点落历史/自动连续推进。
- **常驻 timer service**：由 user systemd service 或显式 `daemon` 子命令持有 deadline、发通知、推进状态；TUI/CLI 通过 Unix socket 控制。适合“关掉 TUI 仍准点提醒”的承诺，但带来服务生命周期、协议、升级和故障恢复成本。

建议 MVP 先做无 daemon，但只承诺“关闭 TUI 后状态不丢，重新读取会正确结算”；把 notifier 定义为核心 transition 产生的 effect。若“无人查询也必须准点通知”是 MVP 必需，则应从第一天引入 daemon，而不是让每个 CLI 调用各自发通知，后者会重复提醒。

通知去重需要持久化 `last_notified_transition_id` 或 event id；否则 Waybar 每秒读取可能重复发送。通知失败不应回滚已经完成的 timer transition。

## 8. 可借鉴与应避免

### 概念上可借鉴

- Headless core + `App` 交互状态 + 纯 renderer。
- 同一二进制中的 TUI/CLI 双入口。
- 结构化 outcome/effect，前端负责文案和格式。
- XDG 配置、原子保存、热重载失败保留旧配置。
- dirty redraw、动态 timeout、resize redraw、终端可靠 restore。
- 语义主题 token、按组件拆分 renderer、TestBackend 双 snapshot。
- 外部变化先 reconcile、再 mutation 的防覆盖思路。

### 不应照搬

- todo.txt 的 `Store` 数据结构、任务 mutation、归档/undo/自然语言解析/分享服务器：与番茄钟核心无关。
- 无 daemon 的产品承诺：除非接受关闭所有前端时没有主动通知。
- 单一固定 `.tmp` 文件名：多进程场景会冲突。
- 把大部分键盘更新逻辑继续堆在 `main.rs`。
- renderer 普遍读取整个 `App`；新组件更适合窄 props。
- 自制 JSON 字符串拼接。新项目应使用 `serde` / `serde_json`，尤其状态文件、通用 JSON 和 Waybar JSON 都需要可靠 escaping 与 schema 演进。
- 具体主题数值、logo、截图、文案和独特布局。它们属于参考项目的表达，不是架构模式。

## 9. 许可证影响

### 事实

Tuxedo 使用 MIT License，版权人为 Webstone Technologies Inc（2026）。许可证允许使用、复制、修改、合并、发布、分发、再许可和销售，但要求在软件的所有副本或“实质部分”中保留版权声明与许可声明；软件按现状提供且无担保。[`LICENSE`](../../tuxedo/LICENSE)

### 建议

- 只学习架构思想、模块边界和通用模式，不复制具体实现时，仍在本调研记录来源，便于项目溯源。
- 若复制或改编源码、主题文件、SVG、文案、测试 helper 等实质内容，应在发行物中保留 Tuxedo 的 MIT 版权和许可证文本，建议放入 `THIRD_PARTY_LICENSES` 或相应文件头。
- 删除 `tuxedo/` 前，确认本报告已提交，并决定是否有任何实际复制内容；若有，先补齐第三方声明。删除参考目录不会消除已复制内容的许可证义务。
- 项目自己的许可证可以另选，但被复制的 MIT 部分仍需附带原声明。此处是工程合规建议，不是法律意见。

## 10. 对下一阶段 grilling/spec 的关键问题

1. 关闭 TUI 且 Waybar 暂时不运行时，是否仍必须准点发通知？这决定 MVP 是否需要 daemon。
2. session 到点后是停在“待确认完成”，还是自动进入休息并继续倒计时？
3. 一个 timer 是否必须绑定任务？首版任务是简单文本队列，还是需要项目、估时、完成状态？
4. 暂停是否无限制；系统 suspend 时间计入 session 吗？
5. 历史统计最小需求是什么：每日 focus 分钟、完成轮数、按任务汇总，还是都不要？
6. Waybar 点击语义：左键 toggle、右键 skip、中键打开 TUI 是否合适？
7. 是否只支持 Linux/Wayland？若是，可用 systemd user service 与 D-Bus 通知简化 daemon；若跨平台，服务管理需要抽象。

## 一手来源索引

- 本地参考项目：[`tuxedo/README.md`](../../tuxedo/README.md)、[`Cargo.toml`](../../tuxedo/Cargo.toml)、[`LICENSE`](../../tuxedo/LICENSE) 及上述逐项源码链接。
- Tuxedo 上游：<https://github.com/webstonehq/tuxedo>（本地 remote 指向该仓库）。
- Waybar 官方仓库：<https://github.com/Alexays/Waybar>。
- notify-rust 官方 API 文档：<https://docs.rs/notify-rust/latest/notify_rust/>。
- fs2 官方 API 文档：<https://docs.rs/fs2/latest/fs2/trait.FileExt.html>。
