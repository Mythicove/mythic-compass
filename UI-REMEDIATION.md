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

## 最终验收（2026-11-23）

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

矩阵主要覆盖空数据页面；真实实例创建与设置保存另有交互验收。浏览器业务场景使用外部 Tauri transport fixture，Windows 原生场景使用真实后端。本轮未全量重测在线游戏进服、Hoard 安装、签名升级，也未进行 macOS/Linux 原生运行或 Apple 公证验收。前一轮在线游戏证据不计作本轮重新验证。当前构建未创建新发行版本。
