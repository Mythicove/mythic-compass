// 与 Rust 命令层一一对应的类型化封装。
//
// 前端不再直接 fetch 后端：所有 HTTP 都在 Rust 侧（src-tauri/src/api.rs），
// 于是桌面端不依赖 Mirror 的 CORS 配置，CSP 也能收紧到不放行任何远端 connect/img。
// 令牌同理不进 webview——authLogin 返回的是不含令牌的 AccountView。
import { invoke } from "@tauri-apps/api/core";

export interface DownloadConfig {
  concurrency: number;
  mirror: string;
}

export interface Defaults {
  memoryMb: number;
}

export interface Config {
  apiUrl: string;
  hoardUrl: string;
  /** 空串 = 跟随系统 */
  language: string;
  theme: "system" | "dark" | "light";
  download: DownloadConfig;
  defaults: Defaults;
}

export interface PathsInfo {
  root: string;
  configFile: string;
  accountsFile: string;
  /** 令牌所在的文件。**明文**（0600），所以要在 UI 上显示出来而不是藏着 */
  secretsFile: string;
  cache: string;
  assets: string;
  libraries: string;
  versions: string;
  java: string;
  tools: string;
  instances: string;
}

/** 账号类型。微软正版（device code flow）还没接：它需要一个 Azure 应用的 client ID，
 *  那是个待定决策，所以这里不放假的取值——放了就意味着 UI 上有个点了会失败的按钮。 */
export type AccountKind = "yggdrasil" | "offline";

export interface AccountView {
  playerName: string;
  playerUuid: string;
  apiUrl: string;
  signedInAt: number;
  kind: AccountKind;
}

/** 账号列表里的一条：**不含任何机密**。 */
export interface AccountEntry {
  /** 本机内唯一，也是机密存储的键。由 (kind, apiUrl, playerUuid) 派生，
   *  所以改名不会让令牌失联，重复登录也不会产生第二条。 */
  id: string;
  kind: AccountKind;
  playerName: string;
  playerUuid: string;
  apiUrl: string;
  signedInAt: number;
  lastUsedAt: number;
}

/** 机密存储的现状。UI 必须照着 `encrypted` 说话。 */
export interface SecretStoreInfo {
  /** 目前只有 `"file"`；接了系统钥匙串之后会是 `"keyring"` */
  kind: string;
  /** **false = 令牌是明文落盘的**（权限 0600，与账号列表分开存）。
   *  不能显示成"已加密"：把混淆叫成加密，正是会让人误判风险的那类错。 */
  encrypted: boolean;
  path: string;
}

export interface AccountsView {
  accounts: AccountEntry[];
  activeId: string | null;
  store: SecretStoreInfo;
}

/** `authCheck` 的结果。
 *
 *  两个字段而不是 `account` 一个：`account` 为 null 有两种完全不同的含义——"从来没登录过"
 *  与"本来登录着，但服务端判定令牌失效、已经把你登出了"。后者必须解释给用户，
 *  否则他看到的是启动器莫名其妙回到了登录界面，而自己刚才明明还登录着。 */
export interface AuthCheckView {
  account: AccountView | null;
  /** 本地本来有账号，但令牌已失效并被清除。UI 据此说"登录已失效，请重新登录"。 */
  expired: boolean;
}

/** 一条启动器独立密码的元数据。**没有 password 字段**：mirror 永不返回哈希或明文，
 *  所以这个类型不可能泄露它——不是"约定不去读"，是根本不在契约里。 */
export interface CredentialView {
  id: string;
  /** 用户给的标签（如"台式机"），只为在列表里区分多条，不参与认证 */
  name: string;
  createdAt: string;
  /** null = 从未用过 */
  lastUsedAt: string | null;
}

/** 游戏日志（`logs/latest.log` 与滚动出去的 `.log.gz`）还是崩溃报告（`crash-reports/*.txt`） */
export type LogKind = "gameLog" | "crashReport";

export interface LogArchive {
  /** **相对实例游戏目录**的路径（如 `logs/latest.log`）。读取时原样传回。 */
  name: string;
  kind: LogKind;
  /** Unix 秒 */
  modifiedAt: number;
  /** 磁盘上的字节数。`.gz` 是压缩后的大小，与读出来的文本长度不是一回事。 */
  size: number;
  compressed: boolean;
}

export interface LogText {
  name: string;
  text: string;
  /** 文件比上限大，`text` 只是**末尾**那一段——UI 必须说清楚，
   *  否则用户会以为游戏就打了这么几句 */
  truncated: boolean;
}

/** 严重程度。`warning` 不是"轻微"，而是"游戏还在跑，但有一件事没按你以为的方式发生"
 *  ——实测现代 Forge/NeoForge 对混装的 mod 是跳过并继续启动，不是崩溃。 */
export type FindingSeverity = "fatal" | "warning";

/** 一键动作。按 `kind` 决定点了去哪儿，**不要解析文案**。 */
export type DiagnoseAction =
  | { kind: "installJava"; major: number }
  | { kind: "installMod"; id: string }
  | { kind: "adjustMemory" }
  | { kind: "openContent" }
  | { kind: "openLogs" };

export interface Finding {
  /** 稳定码，对应 i18n 的 `diagnose.<code>` */
  code: string;
  severity: FindingSeverity;
  /** 命中的**原文行**：给用户看"我们是从哪句判断的"。一句能被核对的证据
   *  比一段我们自己的转述更可信，也让我们在规则错的时候容易被发现。 */
  evidence: string;
  action: DiagnoseAction | null;
}

export interface Diagnosis {
  /** 诊断依据的那份文件（相对路径）。null = 这个实例还没有任何日志，
   *  与"日志里没查出问题"是两件事，UI 要分开说。 */
  source: string | null;
  findings: Finding[];
  /** 文件超过诊断上限，只看了末尾：早期的告警可能漏掉 */
  partial: boolean;
}

export interface Player {
  id: string;
  name: string;
  premium?: boolean;
}

export interface HoardProject {
  id: string;
  slug: string;
  title: string;
  description: string;
  project_type: string;
  loaders?: string[];
  game_versions?: string[];
  downloads: number;
  published: string;
}

export interface HoardFile {
  url: string;
  filename: string;
  primary: boolean;
  size: number;
  hashes?: Record<string, string>;
}

export interface HoardVersion {
  id: string;
  project_id: string;
  version_number: string;
  version_type: string;
  loaders?: string[];
  game_versions?: string[];
  files: HoardFile[];
}

export interface SearchResult<T> {
  hits: T[];
  offset: number;
  limit: number;
  total_hits: number;
}

/** 版本清单条目（piston-meta）。字段与 Rust 侧 `meta::ManifestEntry` 一一对应，
 *  两边由 tests/shape_contract.rs 比对——改一边不改另一边会在那里失败。 */
export interface ManifestEntry {
  id: string;
  /** Rust 侧字段名是 `kind`，序列化成 `type`（`type` 是 TS 关键字，但作为属性名合法） */
  type: string;
  url: string;
  time: string;
  releaseTime: string;
  sha1?: string;
  complianceLevel: number;
}

export interface DownloadSummary {
  downloaded: number;
  skipped: number;
  bytes: number;
  failed: string[];
}

export interface InstalledVersion {
  id: string;
  jarVersion: string;
  assetIndexId: string;
  mainClass: string;
  javaMajor: number;
  download: DownloadSummary;
}

export interface Instance {
  slug: string;
  name: string;
  versionId: string;
  createdAt: number;
  lastLaunchedAt: number | null;
  launches: number;
  memoryMb: number | null;
  javaPath: string | null;
  extraJvmArgs: string[];
  extraGameArgs: string[];
  resolution: Resolution | null;
  server: ServerJoin | null;
  demo: boolean;
  /** null = 原版。装载 loader 时写入，不是从 versionId 反推的（见 Rust 侧 InstanceLoader） */
  loader: InstanceLoader | null;
}

export interface Resolution {
  width: number;
  height: number;
}

export interface ServerJoin {
  host: string;
  port: number | null;
}

/** 设置面板能改的字段。启动计数与最近启动时间由 Rust 侧维护，不在这里回传。 */
export interface InstanceSettings {
  name: string;
  versionId: string;
  memoryMb: number | null;
  javaPath: string | null;
  extraJvmArgs: string[];
  extraGameArgs: string[];
  resolution: Resolution | null;
  server: ServerJoin | null;
  demo: boolean;
}

/** 内容类型，取值就是 Minecraft 的目录名（Rust 侧 `content::Kind`）。 */
export type ContentKind = "mods" | "resourcepacks" | "shaderpacks" | "saves";

export const CONTENT_KINDS: ContentKind[] = [
  "mods",
  "resourcepacks",
  "shaderpacks",
  "saves",
];

/** 依赖类型。取值与 Hoard 的 `version_dependencies.dependency_type` 一致，
 *  也是 Modrinth 那一套——不新增第五种。 */
export type DepKind = "required" | "optional" | "incompatible" | "embedded";

export interface ModDep {
  id: string;
  kind: DepKind;
  /** 版本约束**原文**（`>=0.16.0`、`<0.6.6`、`*`、`[1.21.4,1.21.5)`…）。
   *  后端不求值它，UI 也不要装作求值过。 */
  constraint: string;
}

/** jar 内解析出的 mod 元数据；解析不出来时为 null，UI 退回显示文件名。 */
export interface ModMeta {
  name: string;
  id: string;
  version: string;
  loaders: string[];
  /** 这个 jar 还顶替哪些 mod id（`fabric-api` 顶替 `fabric`）。依赖检查要算上它们。 */
  provides: string[];
  /** mod 自己声明的运行侧（`client` / `server` / `*`）。只有 fabric.mod.json 有这个字段，
   *  其它三种格式没有 → null（表示"来源没说"，不是"两侧都行"）。 */
  environment: string | null;
  /** 全部依赖，按类型分开。已排序去重。 */
  deps: ModDep[];
}

/** 缺一个必需依赖。 */
export interface MissingDep {
  id: string;
  constraint: string;
  /** 谁要它。用户要能看出"是 X 需要 Y"，而不只是"缺 Y"。 */
  requiredBy: string;
}

/** 命中一条不兼容声明。 */
export interface Conflict {
  /** 冲突的另一方 */
  id: string;
  constraint: string;
  /** 声明者（可能是已装的那个 mod，不是新装的那个） */
  declaredBy: string;
}

/** 依赖体检结果。**不评估版本区间**：四家语法各不相同，自己写求值器判错的后果
 *  比明说"不判定"更糟。所以只回答两个不需要求值就能确定的问题。 */
export interface DependencyReport {
  /** 缺的必需依赖。不阻止安装，但该让用户看见并给出去 Hoard 搜的动作。 */
  missing: MissingDep[];
  /** 命中的**无条件**不兼容 → 安装时会被拦下 */
  blocked: Conflict[];
  /** 命中了带版本约束的不兼容，而我们不求值区间 → 只提醒 */
  uncertain: Conflict[];
}

export interface ContentItem {
  kind: ContentKind;
  fileName: string;
  displayName: string;
  size: number;
  modifiedAt: number;
  enabled: boolean;
  meta: ModMeta | null;
}

export interface ContentStats {
  kind: ContentKind;
  total: number;
  enabled: number;
  bytes: number;
}

export interface ContentListing {
  kind: ContentKind;
  /** 内容目录绝对路径，交给 opener 插件的 revealItemInDir */
  dir: string;
  items: ContentItem[];
  stats: ContentStats;
}

export interface JavaRuntime {
  path: string;
  version: number;
  is64bit: boolean;
}

/** 这个平台上可下载的官方 Java 运行时 */
export interface JavaOffer {
  /** Mojang 的产品名（`java-runtime-delta` 等），也是 cache/java/ 下的目录名 */
  component: string;
  /** 索引里自称的版本号（如 `21.0.7`）。注意产品名到版本的映射是 Mojang 定的：
   *  `java-runtime-gamma` 实测是 17 而不是 21，所以别从名字推版本。 */
  versionName: string;
}

export interface JavaOverview {
  /** 本机探测到的可用 JRE（`java -version` 真跑过的），含我们自己下载的 */
  runtimes: JavaRuntime[];
  /** `cache/java/` 下已装好的组件名 */
  downloaded: string[];
  available: JavaOffer[];
  /** 取不到官方索引时的原因。UI 必须区分两种「available 为空」：
   *  null = 这个平台根本没有官方运行时（如 Linux arm64），要引导手动指定路径；
   *  有值 = 只是现在查不到，重试即可。混成一句「没有可用的 Java」会让前者看起来像临时故障。 */
  availableError: string | null;
}

export interface InstalledJava {
  /** `bin/java`（Windows 上是 `bin\java.exe`）的绝对路径 */
  path: string;
  component: string;
  versionName: string;
  download: DownloadSummary;
}

/** 启动时的命令行意图。
 *
 *  `importModpack` 也可能来自**双击 `.mrpack`**（文件关联会把路径作为位置参数传进来），
 *  不一定有人真打了 `--import`。
 *
 *  `unknown` 是没能识别的参数，原样保留：打错参数却看到启动器正常起来，
 *  用户会以为命令生效了，然后去查一个不存在的问题。 */
export interface CliIntent {
  instance: string | null;
  launch: boolean;
  importModpack: string | null;
  unknown: string[];
}

/** Mod loader 类型。Forge 的版本号是 `<游戏版本>-<forge版本>`（如 `1.21.4-54.1.22`），
 *  与 NeoForge 的 `21.4.161` 不同构，UI 直接原样显示。 */
export type LoaderKind = "fabric" | "quilt" | "neoforge" | "forge";

export const LOADER_KINDS: LoaderKind[] = ["fabric", "quilt", "neoforge", "forge"];

/** 专有名词，不翻译（所以放在这里而不是 i18n 表里）。 */
export const LOADER_LABELS: Record<LoaderKind, string> = {
  fabric: "Fabric",
  quilt: "Quilt",
  neoforge: "NeoForge",
  forge: "Forge",
};

/** 一个可用的 loader 版本。`stable` 为 null 表示**来源没说**
 *  （Quilt 的 meta 没有这个字段）——不要把它显示成"开发版"。 */
export interface LoaderVersion {
  version: string;
  stable: boolean | null;
}

/** 实例上记录的 loader。装载时写入，不是从版本 id 反推的。 */
export interface InstanceLoader {
  kind: LoaderKind;
  version: string;
  gameVersion: string;
}

/** 导出结果（对应 Rust 的 `archive::ZipSummary`）。 */
export interface ZipSummary {
  files: number;
  bytes: number;
}

/** 导入前对压缩包的"看一眼"（对应 Rust 的 `instance::ArchiveInfo`）。
 *  这些字段会影响启动行为，UI 必须在导入前原样摊开给用户看。 */
export interface ArchiveInfo {
  name: string;
  versionId: string;
  memoryMb: number | null;
  files: number;
  bytes: number;
  extraJvmArgs: string[];
  extraGameArgs: string[];
  server: ServerJoin | null;
  demo: boolean;
  /** 包里带了别人机器上的 Java 路径；导入时会被清空 */
  hadCustomJava: boolean;
}

/** 装之前对 `.mrpack` 的"看一眼"（对应 Rust 的 `modpack::ModpackInfo`）。
 *  整合包决定我们下载什么、往游戏目录哪里写、装哪个 loader 版本，
 *  所以这些数字与警告必须在落盘之前摊开给用户看。 */
export interface ModpackInfo {
  name: string;
  /** 整合包自己的版本号（不是游戏版本），如 `8.2.0` */
  packVersion: string;
  summary: string | null;
  formatVersion: number;
  /** null = 包里没写、或写成了 `*`，这种包装不了 */
  minecraft: string | null;
  loader: LoaderKind | null;
  loaderVersion: string | null;
  filesTotal: number;
  filesRequired: number;
  /** 作者标为可选，我们仍然装上，但单独报出来 */
  filesOptional: number;
  /** `env.client = unsupported`，客户端安装会跳过 */
  filesSkipped: number;
  bytes: number;
  overrideEntries: number;
  warnings: string[];
}

/** 一次整合包安装的结果（对应 Rust 的 `modpack::ModpackInstall`）。 */
export interface ModpackInstall {
  instance: Instance;
  downloaded: number;
  bytes: number;
  optionalInstalled: number;
  skippedUnsupported: number;
  overrideFiles: number;
  /** 被整合包覆盖掉的已存在文件（相对游戏目录） */
  overwritten: string[];
  failed: string[];
  warnings: string[];
  /** 装完之后对整个实例做的依赖体检。整合包一次装几十个，逐个拦既慢又不合适
   *  （两个 mod 互相冲突不该让整包失败），所以这里只报不拦。 */
  deps: DependencyReport;
}

/** 一次整合包导出的结果。分类计数是为了让用户看懂"为什么这个包这么大"。 */
export interface ModpackExport {
  path: string;
  /** 进了 `files[]` 的（只存地址与 sha1，安装方自己下） */
  files: number;
  /** 进了 `overrides/` 的实体文件（字节真的写进包里） */
  overrideFiles: number;
  bytes: number;
  /** 有来源簿记但 sha1 与磁盘对不上（文件被换过）→ 退回 overrides */
  stale: string[];
  /** 被排除的用户数据目录（存档、日志、崩溃报告、截图） */
  skippedDirs: string[];
}

export interface LaunchInfo {
  instance: string;
  versionId: string;
  java: string;
  javaVersion: number;
}

/** 事件载荷（Rust 侧经 Tauri event 推送）。 */
export interface FileProgress {
  url: string;
  doneBytes: number;
  totalBytes: number;
}

export interface LogLine {
  stream: "stdout" | "stderr" | string;
  line: string;
}

export interface GameExit {
  instance: string;
  /** null = 用户主动停止（与"游戏崩了"必须能区分） */
  code: number | null;
}

/** 复制实例的进度（字节）。与下载进度分开：那边按文件计数，这边按字节。 */
export interface CopyProgress {
  slug: string;
  copiedBytes: number;
  totalBytes: number;
}

export const EVENTS = {
  progress: "compass://progress",
  gameLog: "compass://game-log",
  gameExit: "compass://game-exit",
  copyProgress: "compass://copy-progress",
} as const;

/** Rust 侧 `Error` 序列化后的形状：`code` 用于本地化，`message` 只用于日志。 */
export interface IpcErrorBody {
  code: string;
  message: string;
}

export class IpcError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "IpcError";
  }
}

/** invoke 的拒绝值是我们序列化的 { code, message }；其它异常归为 unknown。 */
export function toIpcError(e: unknown): IpcError {
  if (e instanceof IpcError) return e;
  if (e && typeof e === "object" && "code" in e && "message" in e) {
    const body = e as IpcErrorBody;
    return new IpcError(String(body.code), String(body.message));
  }
  return new IpcError("unknown", String(e));
}

export const ipc = {
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (config: Config) => invoke<Config>("set_config", { config }),
  pathsInfo: () => invoke<PathsInfo>("paths_info"),
  /** 读取一次即清空：启动期的问题只提示一次 */
  takeStartupWarning: () => invoke<string | null>("take_startup_warning"),

  /** 玩家名 + 该玩家的启动器独立密码（不是网站主密码，见 ADR-0012） */
  authLogin: (playerName: string, password: string) =>
    invoke<AccountView>("auth_login", { playerName, password }),
  authLogout: () => invoke<void>("auth_logout"),
  /** 校验令牌，快到期时顺带轮换（mirror 的 refresh 只能换未过期的令牌，所以必须提前）。
   *  `expired` 为 true 表示"本来登录着、但令牌已失效并被清除"，与"从来没登录过"不同。 */
  authCheck: () => invoke<AuthCheckView>("auth_check"),
  /** 账号总览。`store.encrypted` 为 false 时 UI 必须明示令牌是明文落盘的。 */
  accountsList: () => invoke<AccountsView>("accounts_list"),
  /** 加一个离线账号并切换过去（不需要网络，也没有令牌） */
  accountAddOffline: (playerName: string) =>
    invoke<AccountView>("account_add_offline", { playerName }),
  accountSetActive: (id: string) => invoke<AccountView>("account_set_active", { id }),
  /** 删账号连同它的令牌；删的是当前账号时自动切到剩下的第一个 */
  accountRemove: (id: string) => invoke<void>("account_remove", { id }),

  /** 当前账号的启动器独立密码（只有元数据）。仅站点账号有；离线账号别调，Rust 侧会拒。 */
  credentialsList: () => invoke<CredentialView[]>("credentials_list"),
  /** 新建一条（每台设备一条，上限由 mirror 定，超了回 409）。
   *  `password` 明文只在这一次调用里存在：调用方提交后要立刻清掉自己那个 state。 */
  credentialsCreate: (label: string, password: string) =>
    invoke<CredentialView>("credentials_create", { label, password }),
  /** 撤销一条，由它建立的会话**立即**失效——可能包括当前这一个，届时下次 authCheck 会报 expired */
  credentialsDelete: (id: string) => invoke<void>("credentials_delete", { id }),

  /** 这个实例的日志与崩溃报告清单，新的在前。没跑过的实例返回空数组，不是错误。 */
  instanceLogs: (slug: string) => invoke<LogArchive[]>("instance_logs", { slug }),
  /** 读一份日志的**末尾**（512KB 上限）。`truncated` 为真时必须告诉用户不是全文。 */
  instanceLogRead: (slug: string, name: string) =>
    invoke<LogText>("instance_log_read", { slug, name }),
  /** 诊断最近一次失败：优先崩溃报告，没有就看 latest.log */
  instanceDiagnose: (slug: string) => invoke<Diagnosis>("instance_diagnose", { slug }),
  /** 实例的 logs 目录（交给 opener 定位） */
  instanceLogsDir: (slug: string) => invoke<string>("instance_logs_dir", { slug }),
  /** 启动器自己的操作日志全文（用户报问题时导出的就是这个，里面没有任何机密） */
  operationLog: () => invoke<string>("operation_log"),
  operationLogPath: () => invoke<string>("operation_log_path"),

  /** 皮肤纹理的 data: URL；无皮肤返回 null（前端用默认模型） */
  skinDataUrl: (name: string) => invoke<string | null>("mirror_skin_data_url", { name }),

  hoardSearch: (q: string, offset: number, limit: number) =>
    invoke<SearchResult<HoardProject>>("hoard_search", { q, offset, limit }),
  hoardVersions: (projectId: string) =>
    invoke<HoardVersion[]>("hoard_versions", { projectId }),
  /** 装进实例的内容目录。校验和由 Rust 从 file.hashes 里挑，不在前端挑 */
  hoardInstall: (slug: string, kind: ContentKind, file: HoardFile) =>
    invoke<ContentItem>("hoard_install", { slug, kind, file }),

  // ---- 版本与安装 ----
  versionsList: (includeSnapshots: boolean) =>
    invoke<ManifestEntry[]>("versions_list", { includeSnapshots }),
  versionsInstalled: () => invoke<string[]>("versions_installed"),
  /** 装到全局缓存（幂等）；进度经 EVENTS.progress 推送 */
  installVersion: (versionId: string) => invoke<InstalledVersion>("install_version", { versionId }),

  // ---- 实例 ----
  instanceList: () => invoke<Instance[]>("instance_list"),
  instanceCreate: (slug: string, name: string, versionId: string) =>
    invoke<Instance>("instance_create", { slug, name, versionId }),
  instanceDelete: (slug: string) => invoke<void>("instance_delete", { slug }),
  /** 保存实例设置；校验规则在 Rust 侧（内存/分辨率范围、空参数、换行） */
  instanceUpdate: (slug: string, settings: InstanceSettings) =>
    invoke<Instance>("instance_update", { slug, settings }),
  /** 复制实例；进度经 EVENTS.copyProgress 推送 */
  instanceDuplicate: (slug: string, newSlug: string, newName: string) =>
    invoke<Instance>("instance_duplicate", { slug, newSlug, newName }),
  /** 导出为 zip；dest 来自原生保存对话框 */
  instanceExport: (slug: string, dest: string, includeSaves: boolean) =>
    invoke<ZipSummary>("instance_export", { slug, dest, includeSaves }),
  /** 只读压缩包里的 instance.json，不落盘；UI 据此在导入前让用户确认 */
  instanceInspectArchive: (path: string) =>
    invoke<ArchiveInfo>("instance_inspect_archive", { path }),
  /** slug 由调用方给，不取自压缩包 */
  instanceImport: (path: string, slug: string, name: string) =>
    invoke<Instance>("instance_import", { path, slug, name }),
  /** 只读 `.mrpack` 里的 modrinth.index.json，不落盘；UI 据此在安装前让用户确认 */
  modpackInspect: (path: string) => invoke<ModpackInfo>("modpack_inspect", { path }),
  /** slug 由调用方给，不取自整合包；失败时后端会把实例目录整个删掉 */
  modpackInstall: (path: string, slug: string, name: string) =>
    invoke<ModpackInstall>("modpack_install", { path, slug, name }),
  /** 把实例导出成 .mrpack；dest 来自原生保存对话框。存档/日志/崩溃报告/截图不进包。 */
  modpackExport: (slug: string, dest: string, packVersion: string) =>
    invoke<ModpackExport>("modpack_export", { slug, dest, packVersion }),
  /** 这个实例会用哪个 Java；null = 没找到（此时启动会失败，UI 要提前说清楚） */
  instanceJava: (slug: string) => invoke<JavaRuntime | null>("instance_java", { slug }),

  /** 这个构建有没有配好自动更新（`plugins.updater.pubkey` 非空）。   *  false 是**正常状态**而不是错误：签名私钥必须由发布方自己生成并放进 CI 密钥，
   *  仓库里的配置不带 updater 段。UI 据此说明"这个构建没有自动更新"，
   *  而不是给一个点了必然失败的按钮。 */
  updateConfigured: () => invoke<boolean>("update_configured"),
  /** 启动时的命令行意图；前端在启动时读一次 */
  cliIntent: () => invoke<CliIntent>("cli_intent"),

  /** Java 管理页的数据。探测要真跑 `java -version`，所以可能慢——调用方要有加载态。 */
  javaOverview: () => invoke<JavaOverview>("java_overview"),
  /** 下载官方运行时。进度走 `compass://progress`（与版本安装同一套）；实测 143MB，
   *  静默下载看起来就像卡死。 */
  javaDownload: (component: string) => invoke<InstalledJava>("java_download", { component }),
  /** 删掉一个**我们下载的**运行时；用户自己装的 JDK 不在 cache/java/ 下，删不到 */
  javaRemove: (component: string) => invoke<void>("java_remove", { component }),

  // ---- 实例内容（mods / 资源包 / 光影 / 存档）----
  contentList: (slug: string, kind: ContentKind) =>
    invoke<ContentListing>("content_list", { slug, kind }),
  /** 只有 mods 支持启停：资源包/光影/存档的启用状态由游戏自己的配置文件决定 */
  contentSetEnabled: (slug: string, fileName: string, enabled: boolean) =>
    invoke<ContentItem>("content_set_enabled", { slug, fileName, enabled }),
  /** sources 是原生文件选择器给的绝对路径 */
  contentImport: (slug: string, kind: ContentKind, sources: string[]) =>
    invoke<string[]>("content_import", { slug, kind, sources }),
  contentRemove: (slug: string, kind: ContentKind, fileName: string) =>
    invoke<void>("content_remove", { slug, kind, fileName }),
  /** 整个实例的依赖体检：缺哪些必需依赖、哪些 mod 互相声明了不兼容。
   *  安装时的闸门只看新装的那一个，整合包一次装几十个就需要这个全局视图。 */
  contentCheck: (slug: string) => invoke<DependencyReport>("content_check", { slug }),

  // ---- Mod loader ----
  /** 某个游戏版本可用的 loader 版本（stable 为 null = 来源没说，别当成 dev） */
  loaderVersions: (kind: LoaderKind, gameVersion: string) =>
    invoke<LoaderVersion[]>("loader_versions", { kind, gameVersion }),
  /** 装 loader 并把实例指到新的版本 id；下载进度走 EVENTS.progress */
  loaderInstall: (slug: string, kind: LoaderKind, loaderVersion: string) =>
    invoke<Instance>("loader_install", { slug, kind, loaderVersion }),
  /** 回到原版；原版没装好时会报错而不是把实例指向一个不存在的版本 */
  loaderRemove: (slug: string) => invoke<Instance>("loader_remove", { slug }),

  // ---- 启动 ----
  instanceLaunch: (slug: string) => invoke<LaunchInfo>("instance_launch", { slug }),
  instanceStop: () => invoke<boolean>("instance_stop"),
  gameRunning: () => invoke<string | null>("game_running"),
};
