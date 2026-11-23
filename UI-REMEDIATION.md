# Compass 全页面界面整改

## 静态审计（2026-11-23）

入口是 `src/main.tsx → App.tsx → Shell`。`Shell` 实际挂载五个页面：

| 导航 | 页面 | 关键调用与交互 | 整改前控件标记数量* |
| --- | --- | --- | ---: |
| 启动 | `views/start.tsx` | 实例选择、兼容性查询、启动/停止、下载事件、登录、真实皮肤预览 | 9 |
| 实例 | `views/instances.tsx` | 创建、文件导入、mrpack 审计与安装、加载器、复制/导出、实例设置、转到内容页 | 51 |
| 内容 | `views/content.tsx` | 实例选择、本地文件管理、完整性检查、Hoard 搜索与版本安装 | 10 |
| 日志 | `views/logs.tsx` | 游戏日志缓冲、诊断动作、过滤/复制、操作日志 | 11 |
| 设置 | `views/settings.tsx` | 全局配置、账号管理、下载凭据、Java 管理、更新 | 22 |

\* `rg --count-matches '<(button|input|select|textarea)\b|<(Button|Input|Progress)\b' src/App.tsx src/views --glob '*.tsx'` 的静态匹配数，包含条件分支，不等于同时显示的控件数。外壳另外有 3 个按钮标记；`home.tsx` 有 5 个，但 `App.tsx` 的首页路由使用 `StartView`，不能把旧首页计入实际覆盖。

共享状态由 `App.tsx` 提供：配置与语言/主题上下文、账号刷新、全局游戏日志订阅。实例页及日志诊断通过回调携带 slug 跳到内容页；命令行导入意图打开实例页。业务调用统一通过 `lib/ipc.ts`，视觉迁移需要保留这些调用及忙碌/禁用状态。

## 问题与依据

1. **组件迁移只覆盖启动/登录。** `components/ui/README.md` 明确注明实例、内容、设置、日志仍用 legacy 控件；页面源码也直接使用原生按钮和输入。首页仍有原生 select、声音 checkbox、网站按钮。
2. **存在两套控件规则。** `theme.css` 注册 `legacy` 层，`shell.css` 将整个 `App.css` 放入该层；GrassCN 自带 `components` 层。`App.css` 中全局 `button/input/select` 及 `.tab/.chip/.link` 同时拥有自己的圆角、背景、padding。只替换首页无法统一其他页面。
3. **装饰与真实内容脱节。** `start.tsx` 的三个 `start-block` span 与 `start.css` 的旋转/斜切几何块不来自 Minecraft 内容；它们占据 136px 的首屏空间。外壳使用渐变圆点，而项目已有 `public/icon.png`。
4. **品牌 token 漂移。** `App.css` 的深色 primary/hover、surface-raised，以及浅色背景/边框/文字与 `docs/src/content/docs/design/tokens.md` 不一致。组件风格与品牌色应分别由控件资产及语义 token 负责。
5. **密集业务页缺少统一尺寸约束。** 上游 GrassCN 默认按钮/输入宽 400px；直接迁移到工具栏、表格、侧栏会产生溢出。布局需要规定可用宽度、工具栏换行、表格滚动及紧凑导航，同时保留原生表单事件。

## 改造方案

- 所有实际页面使用同一套 GrassCN Button、Input、NativeSelect、Checkbox；存在 textarea 时使用上游 Textarea。原生 option 由 NativeSelect 保留，确保键盘、多选、表单行为沿用浏览器语义。
- 通过官方 registry 安装缺失组件，复用已存在的 Unicode、字体、音效和贴图模块；保留 MIT 与资产来源记录。
- 删除旧的全局控件皮肤规则，在统一样式层只定义布局尺寸及页面层级；不再新增另一套主题覆盖文件。
- 使用真实项目图标，移除几何 hero；启动操作、实例信息和真实皮肤成为首屏重点。各业务页采用方正、紧凑、可扫描的布局。
- 品牌语义色按已有规范对齐；技术日志保留等宽字体及 ANSI 颜色。

最小设计检查：修复的是全页面组件分裂；不增加配置项或业务接口；控件尺寸由布局决定；复用既有依赖与官方实现；同一次改动迁移所有实际调用点。

## 验收台账

- [x] 入口与页面调用关系审计。
- [x] 官方 registry dry-run：新增 15 个文件，15 个现有共享文件一致并跳过。
- [x] 全页面控件迁移及 legacy 规则清理。
- [x] TypeScript 和生产构建。
- [x] 五页深/浅色、中文/英文、宽/窄窗口视觉检查。
- [x] 表单输入、选择、checkbox、导航、禁用状态与 Windows 原生启动器回归（具体范围见下）。

## 首轮验收（2026-11-23）

| 检查 | 结果与范围 |
| --- | --- |
| 控件静态扫描 | `src/App.tsx` 与 `src/views` 中无原生 button/input/select/textarea 标记；旧 `home.tsx` 也完成迁移，但不计入实际五页。 |
| Compass 类型检查 | `pnpm typecheck` 通过。 |
| Windows 生产构建 | `pnpm tauri build --no-bundle` 通过；测试文件为 `D:\Caches\.cargo\target\release\compass.exe`。 |
| 浏览器检查 | Web 仓库 `pnpm exec tsc --noEmit` 通过；`pnpm exec playwright test --config e2e/compass.config.ts`：2 passed，32.2 秒。包含 mrpack 确认安装、页面矩阵及视口外中文标签绘制断言。 |
| Windows 原生矩阵 | 隔离便携目录运行发布 exe，WebView2 159；五页 × 中英 × 深浅 × 1200/760 宽，共 40 项，全部 `overflow: false`。逐页复查截图及长设置页。 |
| 原生表单交互 | 创建真实实例并选择 1.21.1，切换快照 checkbox；中文实例名、Unicode JVM textarea、854×480 分辨率、127.0.0.1 直连及 demo 切换经 UI 保存，由只读 IPC 核对持久化结果，全部通过。中文输入及未登录禁用启动通过。 |
| 差异格式检查 | Compass 与 Web 的 `git diff --check` 通过。 |

原生截图和完整结果：`D:\Temp\compass-native-kdpFL7\`，其中 `scenario-result.json` 记录 40 项矩阵和实例设置结果，`instance-settings-edited.png` 展示保存前的编辑状态。浏览器产物位于 `web/test-results/`。临时产物路径为本机验收证据，不属于仓库分发资产。

### 验收中修复的问题

- 按钮绝对定位标签导致自适应宽度塌缩：标签参与布局，按钮采用内容宽度和统一 padding。
- NativeSelect 默认尺寸导致工具栏遮挡：控件与贴图宽度由容器约束，inline select 保留可读的最小宽度。
- 长页视口外中文透明：共享 GPU 绘制改用目标 canvas 的本地坐标与实际尺寸，同时删除旧 viewport-origin API 和全部调用点。
- 官方 checkbox 隐藏 input 使测试标签定位歧义：测试按 checkbox role 和名称定位；选择控件按 combobox role 定位。
- 不支持的符号合法回退到文本时，旧 bitmap-ready 等待超时：截图等待绘制帧，中文绘制另以非透明像素验证。

### 构建成本与验收范围

本次 Vite 输出约为 CSS 1645.77 kB（gzip 528.21 kB）、主 JavaScript 1127.01 kB（gzip 296.61 kB）、延迟加载 Unicode 数据 2079.76 kB（gzip 1509.02 kB）。大包告警仍存在；离线贴图、声音和 Unicode 数据均计入该成本。

首轮矩阵主要覆盖空数据页面；真实实例创建与设置保存另有交互验收。浏览器业务场景使用外部 Tauri transport fixture，Windows 原生场景使用真实后端。首轮未全量重测在线游戏进服、Hoard 安装、签名升级，也未进行 macOS/Linux 原生运行或 Apple 公证验收；后续验收见下。

## 正式草稿包与业务验收（2026-11-23）

`dev-69` 草稿对应 `cc8b970`，由 CI `45153753346` 打包。下载的 NSIS、MSI、deb、
AppImage 均按草稿附带 SHA256SUMS 核对：分别为 5,718,420、7,327,744、
7,846,268、91,097,592 字节。Windows 原生业务使用 MSI 行政解包后的正式 exe，
隔离便携目录运行。CI 的 Windows 安装验收另覆盖实际安装、窗口、快捷方式、
mrpack 关联、卸载及关联恢复。

| 检查 | 结果与证据范围 |
| --- | --- |
| Chromium / WebKit | 两个浏览器各 2 passed；页面矩阵及启动、失败重试、停止场景使用外部 transport fixture。Web 类型检查通过。 |
| Windows 原生业务 | 最终重跑 `Q:\FullStack\Mythicove\.acceptance-temp\compass-native-Owo0ix\scenario-result.json`：40 项矩阵、中文实例设置、本地 JAR 原生导入、metadata、启停、依赖体检、删除、实例复制、原生 ZIP 导出及确认后导入通过；确认页提交前实例数量不变，导入后的 JAR 字节也与来源一致。 |
| 生产 Hoard 服务 | 独立内存 store 的生产服务实际上传、搜索、版本安装；下载字节与上传的 fixture JAR 一致。 |
| 真实发布与核心进服 | 最终核心验收通过，耗时 321.32s；真实 Isles mrpack、官方 Java 21、MC 1.21.1 / Fabric 0.16.14、online-mode 进服、Mirror UUID 与签名皮肤核对、令牌恢复和失效均通过。 |
| Windows 桌面玩家业务 | 最终 `isles-compass-pack-07HaZ0/desktop-result.json` 全部通过：空缓存确认安装、主密码拒绝与独立密码登录、账号重启恢复、两次真实进服、内容启停、真实日志过滤与复制、复制实例后 loader 改版及卸载、登出；退出清理完成，入口输出最终汇总，错误日志为空。 |
| Linux 原生包 | dev-69 deb 在 WSL Ubuntu 26.04 / WebKitGTK 2.52.6、Xvfb 软件渲染下显示真实窗口；1200×800，窗口就绪约 1770ms。截图 `D:\Temp\opencode\compass-linux-8h7vxy4a\native-window.png` 已复查。 |
| macOS 原生包 | 新增所属进程窗口验收，提交 `b03bb8a`。CI `45185707646` 三平台全部通过并生成 dev-71 草稿；macOS 实际 `.app` 窗口与截图通过，标题 Mythic Compass，1024×674，窗口就绪 3146ms。`macos-native-acceptance` artifact 已下载并复查 GrassCN 首页。 |

### 运行与资源成本

dev-69 的 Windows 原生最终成功场景测得：冷启动至驱动确认就绪 4742ms，暖页面重载
699ms，首个 navigation 完成约 556ms。此前成功场景分别为 4840ms、613ms、529ms。
冷启动包含进程启动、CDP 发现及 UI 等待；
暖重载保留同一进程，不能解释成暖进程启动，也不是多样本性能基准。

实际 WebView2 加载的主 JS、CSS、Unicode 数据 decodedBodySize 分别为
1,127,018、1,645,778、2,079,768 字节；最终场景资源加载耗时约 70、94、69ms。
业务与矩阵结束时 renderer JS heap used 为 16,876,596 字节（约 16.09 MiB），
total 为 20,463,616 字节（约 19.52 MiB），不是完整进程 RSS。
本轮记录成本基线；Vite 大包告警与离线 Unicode / 贴图 / 声音成本仍然存在。

最终验收 exe SHA-256：`b45eac4a1b862f100b922ad2554abc80c42bfde39935a91601e3a991a6d14304`。
D: 临时盘容量耗尽后，后续运行的 TEMP、TMP、GOTMPDIR 和 Go cache 改到
工作区隔离的 `.acceptance-temp`；旧截图和结果保留，删除已失败的局部复制目录
及三个旧场景的下载缓存后，D: 恢复约 60 GiB 可用空间。

最终完整链路证据：`Q:\FullStack\Mythicove\.acceptance-temp\isles-compass-pack-07HaZ0\`，
其中 `desktop-dPZCtp` 保留两次游戏窗口、签入/登出、真实内容与日志、loader
卸载后的截图，`desktop-result.json` 记录业务断言与正式 exe SHA-256。
入口日志为 `D:\Temp\opencode\compass-full-live-12.{out,err}.log`。
这轮运行期间用 SetThreadExecutionState 保持测试显示器与系统唤醒，结束后恢复；
没有修改系统电源方案。测试辅助 PowerShell 隐藏控制台窗口，避免干扰游戏焦点。

### 平台与签名边界

退出清理已改为异步执行 taskkill 并等待所属子进程的实际退出，避免同步命令阻塞
Node exit 回调导致状态滞后；服务端清理放入 finally。过程中曾出现磁盘耗尽、
Windows 系统资源不足、一次 WebView 页面与截图超时，以及游戏前台焦点获取失败。
失败证据保留，不能计作成功；最终在空间充足、显示器唤醒且辅助控制台隐藏的条件下
重新完成整条链路。单次 WebView 超时的独立根因未确定，最终通过不构成其因果证明。

Linux 和 macOS 原生包证据覆盖启动与窗口，尚未完成完整玩家链路及五页交互矩阵。
仓库未配置 updater 签名密钥及 Apple 签名 / 公证 secrets，签名升级和公证待提供
对应凭据后验证。dev-69、dev-71 均保持草稿，没有公开发行。
