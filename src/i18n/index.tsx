// 轻量 i18n：桌面端没有 next-intl，也不为一个启动器引入 i18next。
// 键名与 web 的风格保持一致（errors.* 按后端稳定错误码映射），日后要接 Crowdin 也是同一套键。
import { createContext, useCallback, useContext, useEffect, useMemo, type ReactNode } from "react";

export type Locale = "zh-CN" | "en";

type Messages = Record<string, string>;

const zhCN: Messages = {
  "app.name": "Mythic Compass",
  "nav.home": "首页",
  "nav.instances": "实例",
  "nav.settings": "设置",

  "account.signIn": "登录",
  "account.signOut": "退出登录",
  "account.playerName": "玩家名",
  "account.launcherPassword": "启动器独立密码",
  "account.signInHint":
    "用玩家名 + 该玩家的独立密码登录，不是网站主密码。独立密码在网站的账号页（Profile → 启动器登录）生成。",
  "account.signedInAs": "已登录：{player}",
  "account.notSignedIn": "未登录",
  "account.since": "登录于 {time}",
  "account.checking": "正在校验令牌…",
  "account.openSite": "在网站上管理独立密码",

  "home.skinPreview": "皮肤预览",
  "home.skinPreviewPlayer": "预览对象",
  "home.instances": "我的实例",
  "home.noInstances": "还没有实例。到「实例」页新建一个，装好版本即可启动。",

  "nav.content": "内容",

  "content.title": "实例内容",
  "content.instance": "实例",
  "content.noInstance": "先创建一个实例，再来管理它的模组、资源包与存档。",
  "content.kind.mods": "模组",
  "content.kind.resourcepacks": "资源包",
  "content.kind.shaderpacks": "光影",
  "content.kind.saves": "存档",
  "content.stats": "{total} 项 · 启用 {enabled} · {mb} MB",
  "content.empty": "这个目录还是空的。点「导入」选择文件，或把文件拖进窗口。",
  "content.import": "导入",
  "content.imported": "已导入：{names}",
  "content.openDir": "打开目录",
  "content.enable": "启用",
  "content.disable": "禁用",
  "content.remove": "删除",
  "content.removeConfirm": "删除「{name}」？这个操作无法撤销。",
  "content.removed": "已删除 {name}",
  "content.col.name": "名称",
  "content.col.id": "ID",
  "content.col.version": "版本",
  "content.col.loader": "加载器",
  "content.col.depends": "依赖",
  "content.dependsCount": "必需 {n} 项",
  "content.dependsHint":
    "依赖按类型分开显示（把鼠标停在数量上可看完整清单）：必需 / 可选 / 不兼容 / 内嵌。「不兼容」与「依赖」意思相反——是装了才起不来。",
  "content.dep.required": "必需",
  "content.dep.optional": "可选",
  "content.dep.incompatible": "不兼容",
  "content.dep.embedded": "内嵌",
  "content.check": "检查依赖与冲突",
  "content.checkTitle": "依赖体检",
  "content.checkClean": "没有发现缺失的必需依赖，也没有命中不兼容声明。",
  "content.checkBlocked": "{a} 与 {b} 声明了无条件不兼容：其中一个应当被移除",
  "content.checkMissing": "{by} 需要 {id}（{constraint}），当前没有装",
  "content.checkUncertain": "{a} 声明与 {b} 不兼容（{constraint}）——版本区间未判定，请自己确认",
  "content.checkNote":
    "版本区间不求值：四家语法各不相同（>=1.2 <2 / [1.21.4,1.21.5) / 裸版本号），自己写求值器判错的后果比明说「不判定」更糟。所以这里只回答两个不需要求值就能确定的问题：这个 id 在不在，以及这条不兼容声明是不是无条件的。",
  "content.col.size": "大小",
  "content.col.state": "状态",
  "content.enabled": "已启用",
  "content.disabled": "已禁用",
  "content.enableHint":
    "只有模组能启用/禁用：资源包、光影与存档的开关由游戏自己的配置文件决定，启动器再造一个开关只会显示成已启用而实际没生效。",
  "content.dropHint": "把文件拖到这里即可导入到当前目录。",
  "content.metaUnknown": "读不出元数据，显示文件名",

  "mods.searchPlaceholder": "搜索模组…",
  "mods.search": "搜索",
  "mods.empty": "没有匹配的模组",
  "mods.downloads": "{n} 次下载",
  "mods.online": "从 Hoard 安装",
  "mods.install": "安装",
  "mods.installing": "正在下载：{mb} / {total} MB",
  "mods.installed": "已安装 {name}",
  "mods.installInto": "装到当前实例的「{kind}」目录。",
  "mods.pickInstanceFirst": "先在上面选一个实例，才能把内容装进去。",
  "mods.noSavesInstall": "存档不通过 Hoard 分发；要换世界请用「导入」选一个世界文件夹。",
  "mods.maybeIncompatible": "可能不兼容",
  "mods.incompatible": "这个版本没有标明支持 {game}",
  "mods.incompatibleConfirm":
    "{version} 没有标明支持 {game}，仍要安装吗？版本不对的 mod 通常会让游戏启动即崩。",

  "common.cancel": "取消",
  "nav.logs": "日志",

  "instances.new": "新建实例",
  "instances.name": "名称",
  "instances.version": "游戏版本",
  "instances.filterVersions": "筛选版本…",
  "instances.includeSnapshots": "包含快照版本",
  "instances.create": "创建",
  "instances.empty": "还没有实例。点「新建实例」选一个游戏版本开始。",
  "instances.install": "安装",
  "instances.installFirst": "需要先安装这个版本",
  "instances.launch": "启动",
  "instances.stop": "停止",
  "instances.delete": "删除",
  "instances.launches": "启动过 {n} 次",
  "instances.lastPlayed": "上次 {time}",
  "instances.javaFound": "Java {version}：{path}",
  "instances.javaMissing": "没找到这个版本需要的 Java（装一个 JDK，或在实例里指定路径）",
  "instances.created": "已创建实例「{name}」，接下来点「安装」下载版本文件。",
  "instances.installed": "{version} 安装完成：新下载 {downloaded} 个文件（{mb} MB），跳过已就位 {skipped} 个。",
  "instances.launched": "已启动（Java {java}）。日志见「日志」页。",
  "instances.downloading": "正在下载：{files} 个文件已完成，共 {mb} MB · 当前 {current}",
  "instances.working": "处理中…",
  "instances.content": "内容",
  "instances.settings": "设置",
  "instances.settingsTitle": "实例设置",
  "instances.save": "保存设置",
  "instances.saved": "设置已保存（下次启动生效）",
  "instances.memoryMb": "最大内存（MiB）",
  "instances.javaPath": "Java 可执行文件（留空 = 按版本要求自动探测）",
  "instances.pickJava": "选择…",
  "instances.javaPicked": "已选择：{path}（保存设置后生效）",
  "instances.extraJvm": "附加 JVM 参数（每行一个）",
  "instances.extraGame": "附加游戏参数（每行一个）",
  "instances.argsHint": "参数原样传入：JVM 参数在主类之前，游戏参数排在最后（因此能覆盖版本自带的同名项）。",
  "instances.resolution": "启动分辨率",
  "instances.resolutionAuto": "跟随游戏",
  "instances.width": "宽",
  "instances.height": "高",
  "instances.server": "启动后直连服务器",
  "instances.serverHost": "地址",
  "instances.serverPort": "端口（留空 = 25565）",
  "instances.demo": "演示模式",
  "instances.duplicate": "复制",
  "instances.duplicateName": "副本名称",
  "instances.duplicating": "正在复制：{mb} MB / {total} MB",
  "instances.duplicated": "已复制为「{name}」",
  "instances.deleteConfirm": "删除实例「{name}」？它的世界、模组与设置都会一起消失，且无法撤销。",
  "instances.export": "导出",
  "instances.exportWithSaves": "包含存档",
  "instances.exportHint":
    "不含存档的包适合分享实例（配置 + 模组）；含存档的是完整备份。logs、crash-reports、natives 一律不打包。",
  "instances.exported": "已导出 {files} 个文件（{mb} MB）到 {path}",
  "instances.import": "导入实例",
  "instances.importTitle": "确认导入",
  "instances.importWarning":
    "实例配置会决定启动时执行什么（Java 路径、JVM 与游戏参数）。只在信任来源时继续。",
  "instances.importArchive": "{name} · {version} · {files} 个文件 · {mb} MB",
  "instances.importJvm": "自定义 JVM 参数：{args}",
  "instances.importGame": "自定义游戏参数：{args}",
  "instances.importJava": "指定了 Java 路径（导入后会清空，改为本机自动探测）",
  "instances.importServer": "启动后直连：{host}",
  "instances.importDemo": "演示模式",
  "instances.importClean": "没有自定义启动参数。",
  "instances.importConfirm": "导入",
  "instances.imported": "已导入实例「{name}」",
  "instances.modpack": "安装整合包",
  "instances.exportModpack": "导出为 .mrpack",
  "instances.exportModpackVersion": "整合包版本号",
  "instances.exportModpackHint":
    "留空则用实例的版本 id。有来源记录的 mod 只写下载地址与 sha1（包很小），其余文件按字节放进 overrides/；存档、日志、崩溃报告与截图一律不进包。",
  "instances.exportedModpack": "已导出：{files} 个文件走下载、{overrides} 个进 overrides · {mb} MB · {path}",
  "instances.exportStale": "{count} 个文件的来源记录已过期（被改过），按字节放进了包里",
  "instances.exportAllOverrides":
    "这个实例里没有带来源记录的文件，所以全部按字节打包——包会很大，但内容完整",
  "instances.modpackTitle": "确认安装整合包",
  "instances.modpackWarning":
    "整合包决定下载哪些文件、写到游戏目录的哪里，以及装哪个 loader 版本。只在信任来源时继续。",
  "instances.modpackSummary": "{name} · 整合包 {version} · 游戏 {game}",
  "instances.modpackLoader": "会安装 {loader} {version}",
  "instances.modpackNoLoader": "不含 mod loader（装成原版）",
  "instances.modpackFiles": "下载 {required} 个必需文件 + {optional} 个可选文件 · {mb} MB",
  "instances.modpackSkipped": "跳过 {count} 个标记为「客户端不支持」的文件",
  "instances.modpackOverrides": "覆盖目录里有 {count} 个文件会写进游戏目录",
  "instances.modpackUninstallable": "这个包装不了：{reason}",
  "instances.modpackConfirm": "安装",
  "instances.modpackInstalled":
    "已安装整合包「{name}」：{downloaded} 个文件 · {mb} MB · 覆盖 {overrides} 个",
  "instances.modpackOptionalNote": "其中 {count} 个是作者标为可选的，已一并装上",
  "instances.modpackOverwritten": "覆盖了 {count} 个已存在的文件：{files}",
  "instances.modpackFailed": "{count} 个文件下载失败：{files}",
  "instances.modpackDeps":
    "依赖体检：{blocked} 处不兼容、{missing} 个缺失（到「内容」页点“检查依赖与冲突”看明细）",
  "instances.loader": "Mod loader",
  "instances.loaderTitle": "Mod loader",
  "instances.loaderCurrent": "当前：{kind} {version}（游戏 {game}）",
  "instances.loaderNone": "原版（没有 loader）",
  "instances.loaderVersion": "loader 版本",
  "instances.loaderLoading": "加载中…",
  "instances.loaderNoVersions": "{game} 没有可用的 loader 版本",
  "instances.loaderStable": "稳定",
  "instances.loaderDev": "开发版",
  "instances.loaderHint":
    "游戏版本固定为 {game}（取自实例，不让你填：填错要到启动时才炸）。安装后实例指向 loader 的版本，原版仍留在共享缓存里。",
  "instances.loaderInstall": "安装 loader",
  "instances.loaderChange": "换 loader 版本",
  "instances.loaderRemove": "卸载 loader（回到原版）",
  "instances.loaderInstalled": "已装上 {kind} {version}",
  "instances.loaderRemoved": "已回到原版",

  "logs.title": "游戏日志",
  "logs.hint":
    "实时显示游戏进程的 stdout/stderr（只保留最近 2000 行，应用启动即开始记录），也可以打开下面列出的归档。控制台输出是纯文本（不是 Mojang 那份 XML 布局），所以实时流、归档与崩溃报告用同一套解析与同一套诊断规则。",
  "logs.clear": "清空",
  "logs.empty": "还没有日志。启动一个实例后这里会实时显示游戏输出。",
  "logs.exited": "{instance} 已退出，退出码 {code}。",
  "logs.stopped": "{instance} 已被停止。",
  "logs.instance": "实例",
  "logs.refresh": "重新诊断",
  "logs.copy": "复制全文",
  "logs.openDir": "打开目录",
  "logs.search": "搜索…",
  "logs.follow": "跟随末尾",
  "logs.truncated": "文件太大，这里只显示末尾的一段。要看全文请打开日志目录。",
  "logs.diagnosis": "崩溃诊断",
  "logs.diagnosisLoading": "读取中…",
  "logs.diagnosisNoLogs": "{instance} 还没有日志——它可能从没启动过。",
  "logs.diagnosisClean": "在 {source} 里没查出已知问题。",
  "logs.diagnosisSource": "依据：{source}",
  "logs.diagnosisPartial": "文件超过诊断上限，只看了末尾：早期的告警可能漏掉。",
  "logs.severity.fatal": "起不来",
  "logs.severity.warning": "游戏还在跑，但有件事不对",
  "logs.level.fatal": "FATAL",
  "logs.level.error": "ERROR",
  "logs.level.warn": "WARN",
  "logs.level.info": "INFO",
  "logs.level.debug": "DEBUG",
  "logs.level.trace": "TRACE",
  "logs.level.other": "其它",
  "logs.archives": "日志归档",
  "logs.archivesHint":
    "游戏自己滚动的历史日志与崩溃报告。实测常见失败并不写崩溃报告（JVM 启动阶段、loader 的 mod 解析阶段都在报告器之外），所以列表里没有 crash-reports 并不代表没出过错。",
  "logs.archivesEmpty": "还没有归档。",
  "logs.archiveEmpty": "这份日志是空的。",
  "logs.archiveColName": "文件",
  "logs.archiveColKind": "类型",
  "logs.archiveColSize": "大小",
  "logs.archiveOpen": "查看",
  "logs.archiveClose": "回到实时",
  "logs.kind.gameLog": "游戏日志",
  "logs.kind.crashReport": "崩溃报告",
  "logs.oplog": "启动器操作日志",
  "logs.oplogHint":
    "启动器自己做了什么：装了哪个版本、用什么 Java 启动、退出码是多少。报问题时把这份一起给出来——「还没跑起来」那类失败根本没有游戏日志。里面没有任何机密（不记令牌，也不记独立密码）。",
  "logs.oplogEmpty": "还没有记录。",

  "diagnose.java_version_mismatch":
    "Java 版本不对：这个游戏版本要 Java {major}。同一台机器上可以装多个 JDK，在实例设置里挑对的那个即可。",
  "diagnose.heap_misconfigured":
    "内存参数自相矛盾（初始堆大于最大堆），JVM 拒绝启动。检查实例设置里的内存与自定义 JVM 参数。",
  "diagnose.out_of_memory": "内存不足：Java 堆用尽了。给这个实例分配更多内存。",
  "diagnose.fabric_missing_mod": "缺少 mod：{id}。装上它（或装上提供它的 mod）再启动。",
  "diagnose.fabric_mod_version_conflict":
    "有 mod 与当前游戏版本或彼此不兼容。Fabric 在日志里写明了是哪一条依赖、要求的版本范围是什么。",
  "diagnose.fabric_mod_resolution_failed": "Fabric 的 mod 解析失败，但原因不是常见的两种。看日志里的 Reason 行。",
  "diagnose.loader_mismatch_skipped":
    "有 mod 是给别的 loader 的，被跳过了——游戏照常启动，但那个 mod 没有生效。Fabric 的 mod 不能放进 Forge/NeoForge 实例，反之亦然。",
  "diagnose.mod_jar_skipped":
    "有一个 mod jar 读不了，被跳过了（常见原因是它是为别的游戏版本编译的）——游戏照常启动，但那个 mod 没有生效。",
  "diagnose.mojang_user_properties_401":
    "游戏向 Mojang 取用户属性时得到 401。用我们镜像站的账号时这是预期的：令牌是镜像站发的，Mojang 不认，游戏照常进行，可以忽略。",
  "diagnose.crash_report_unrecognized": "游戏写了崩溃报告，但原因不在已知规则里。摘要见下，完整报告在日志目录。",
  "diagnose.action.installJava": "去装 Java {major}",
  "diagnose.action.installMod": "去找 {id}",
  "diagnose.action.adjustMemory": "调内存",
  "diagnose.action.openContent": "管理内容",
  "diagnose.action.openLogs": "看完整日志",

  "settings.servers": "服务器地址",
  "settings.apiUrl": "Mirror（皮肤站 / 账号）",
  "settings.hoardUrl": "Hoard（模组分发）",
  "settings.appearance": "外观",
  "settings.language": "语言",
  "settings.languageAuto": "跟随系统",
  "settings.theme": "主题",
  "settings.themeSystem": "跟随系统",
  "settings.themeDark": "深色",
  "settings.themeLight": "浅色",
  "settings.downloads": "下载",
  "settings.concurrency": "并发数",
  "settings.mirror": "镜像源前缀（留空 = 官方源）",
  "settings.defaults": "默认值",
  "settings.memoryMb": "新实例最大内存（MiB）",
  "settings.save": "保存",
  "settings.saved": "已保存",
  "settings.accounts": "账号",
  "settings.accountsStoreKeyring": "令牌存在系统钥匙串里。",
  "settings.accountsStorePlain":
    "令牌以「明文」存在 {path}（权限 0600，与账号列表分开存）。这不是加密：同机上能读到这个文件的人就能拿到令牌。接入系统钥匙串之前请先知道这一点。",
  "settings.accountsEmpty": "还没有账号。用上面的登录表单登录镜像站，或在下面加一个离线账号。",
  "settings.accountColName": "玩家",
  "settings.accountColKind": "类型",
  "settings.accountColSite": "站点",
  "settings.accountActive": "当前",
  "settings.accountKind.yggdrasil": "镜像站",
  "settings.accountKind.offline": "离线",
  "settings.accountUse": "切换",
  "settings.accountRemove": "删除",
  "settings.accountRemoveConfirm": "删除账号「{name}」并清除它的令牌？",
  "settings.accountOfflineName": "离线玩家名",
  "settings.accountAddOffline": "添加离线账号",
  "settings.accountOfflineHint":
    "离线账号不联网、没有令牌，UUID 由名字派生（与原版服务端的算法一致）。只能用 ASCII 字母、数字与下划线，最长 16 字符；进正版服务器会被拒。",
  "settings.credentials": "启动器独立密码",
  "settings.credentialsHint":
    "启动器登录用的是「玩家名 + 该玩家的独立密码」，网站主密码在这个端点上无效。当前玩家：{name}。建议每台设备一条，上限由站点决定（默认 3 条）。",
  "settings.credentialsLoading": "读取中…",
  "settings.credentialsEmpty":
    "这个玩家还没有独立密码——在补建一条之前，它无法从任何启动器登录（包括这一个）。",
  "settings.credentialsColLabel": "标签",
  "settings.credentialsColCreated": "创建时间",
  "settings.credentialsColUsed": "最近使用",
  "settings.credentialsNeverUsed": "从未使用",
  "settings.credentialsLabel": "标签",
  "settings.credentialsLabelPlaceholder": "台式机",
  "settings.credentialsPassword": "独立密码",
  "settings.credentialsCreate": "新建",
  "settings.credentialsRevoke": "撤销",
  "settings.credentialsRevokeConfirm":
    "撤销独立密码「{name}」？由它建立的会话会立即失效——如果撤的正是这台设备在用的那一条，你自己也会被登出。",
  "settings.credentialsPasswordHint":
    "明文只在这一次提交里存在：不落盘、不进日志，提交后立刻从输入框清掉。撤销与新建都直接连站点，启动器不代存密码。",
  "settings.credentialsOffline": "离线账号没有站点，也没有独立密码可管。",
  "settings.java": "Java",
  "settings.javaHint":
    "启动器会按游戏版本的要求挑 Java：1.16.5 及更早必须是 Java 8，1.17–1.20.4 是 17，1.20.5 起是 21。本机没有合适的就会自动下载官方运行时（约 140MB，进度见下）。也可以在实例设置里为单个实例指定路径。",
  "settings.javaDownloading": "正在下载 Java：{done} MB / {total} MB",
  "settings.javaDetected": "本机探测到的",
  "settings.javaNoneDetected":
    "没有找到任何可用的 Java（每一条候选都真跑过 `java -version`）。下面下载一个官方运行时，或在实例设置里手动指定路径。",
  "settings.javaColVersion": "版本",
  "settings.javaColPath": "路径",
  "settings.javaOfficial": "官方运行时",
  "settings.javaOfficialHint":
    "来自 Mojang 的 java-runtime 清单，装进 cache/java/。「产品名」是 Mojang 的叫法，与 Java 版本不是一一对应（实测 gamma 是 17、delta 才是 21），所以要看右边那一列的版本号。",
  "settings.javaColComponent": "产品名",
  "settings.javaColOfferVersion": "版本",
  "settings.javaDownload": "下载",
  "settings.javaReinstall": "重装",
  "settings.javaRemove": "删除",
  "settings.javaRemoveConfirm": "删除已下载的 {component}？只删 cache/java/ 下的这一份，不影响你自己装的 JDK。",
  "settings.javaNoOfficialForPlatform":
    "这个平台上 Mojang 没有发布官方运行时（Linux arm64 整个不在清单里；Apple Silicon 上没有 Java 8/16）。请自行安装对应版本的 JDK，然后在实例设置里指定它的路径。",
  "settings.javaIndexUnavailable":
    "现在取不到官方运行时清单（{error}）。已经装在 cache/java/ 里的仍然可用；重试或检查网络与镜像设置。",
  "settings.update": "自动更新",
  "settings.updateCheck": "检查更新",
  "settings.updateUnconfigured":
    "这个构建没有接自动更新（未注入更新签名公钥）。从源码构建出来的包就是这样：更新签名私钥必须由发布方自己生成并保管，不能由构建过程随手造一个。要升级请从发布页下载新版。",
  "settings.updateCurrent": "当前是最新版本（{version}）。",
  "settings.updateChecking": "正在检查…",
  "settings.updateInstalling": "正在下载并安装…",
  "settings.updateInstalled": "{version} 已下载完成，重启启动器即可生效。",
  "settings.updateError": "检查更新失败：{error}",
  "settings.updateAvailable": "有新版本 {version}。",
  "settings.updateInstall": "下载并安装",
  "settings.paths": "数据目录",
  "settings.pathsHint": "资源、库、版本与 JRE 全局共享（多实例不重复下载）；实例各自隔离。",
  "settings.openDataDir": "打开数据目录",
  "settings.startupWarning": "启动提示",

  "errors.network": "网络异常，请检查后端地址与连通性",
  "errors.api": "后端返回错误",
  "errors.invalid": "输入不合法",
  "errors.config": "配置读写失败",
  "errors.io": "本地文件读写失败",
  "errors.not_found": "找不到它（可能已被删除、移动，或这个版本还没安装）",
  "errors.auth": "登录状态已失效，请重新登录",
  "errors.binding_required": "本站要求先绑定第三方账号，请到网站的账号页完成绑定",
  "errors.password_login_disabled": "本站已关闭账号密码登录（启动器不受影响，请用玩家名 + 独立密码）",
  "errors.email_not_verified": "邮箱还没有验证，验证之后才能从启动器登录",
  // 站点故意不区分"密码错"与"这个玩家还没有独立密码"（一区分就能拿玩家名探测账号是否存在），
  // 所以这句话必须覆盖两种可能，而后者是存量用户升级后最常见的首次失败。
  "errors.invalid_credentials":
    "玩家名或独立密码不正确。注意启动器只认该玩家的独立密码，网站主密码在这里无效；如果这个玩家还没有独立密码，需要先到网站为它创建一条。",
  "errors.web_session_required": "这一步要在网站的账号页完成：启动器的登录身份只对当前这一个玩家有效",
  "errors.action.bindings": "去绑定第三方账号",
  "errors.action.account": "打开账号页",
  "errors.action.credentials": "去创建独立密码",
  "errors.cli_unknown": "无法识别的命令行参数：{args}（已忽略，启动器照常启动）",
  "errors.unknown": "操作失败",
};

const en: Messages = {
  "app.name": "Mythic Compass",
  "nav.home": "Home",
  "nav.instances": "Instances",
  "nav.settings": "Settings",

  "account.signIn": "Sign in",
  "account.signOut": "Sign out",
  "account.playerName": "Player name",
  "account.launcherPassword": "Launcher password",
  "account.signInHint":
    "Sign in with a player name and that player's launcher password — not your site password. Create one on the site under Profile → Launcher sign-in.",
  "account.signedInAs": "Signed in as {player}",
  "account.notSignedIn": "Not signed in",
  "account.since": "Signed in at {time}",
  "account.checking": "Verifying token…",
  "account.openSite": "Manage launcher passwords on the site",

  "home.skinPreview": "Skin preview",
  "home.skinPreviewPlayer": "Previewing",
  "home.instances": "My instances",
  "home.noInstances":
    "No instances yet. Create one on the Instances page — install a version and launch it.",

  "nav.content": "Content",

  "content.title": "Instance content",
  "content.instance": "Instance",
  "content.noInstance": "Create an instance first, then manage its mods, packs and worlds.",
  "content.kind.mods": "Mods",
  "content.kind.resourcepacks": "Resource packs",
  "content.kind.shaderpacks": "Shaders",
  "content.kind.saves": "Worlds",
  "content.stats": "{total} items · {enabled} enabled · {mb} MB",
  "content.empty": "Nothing here yet. Click “Import”, or drop files onto the window.",
  "content.import": "Import",
  "content.imported": "Imported: {names}",
  "content.openDir": "Open folder",
  "content.enable": "Enable",
  "content.disable": "Disable",
  "content.remove": "Delete",
  "content.removeConfirm": "Delete “{name}”? This cannot be undone.",
  "content.removed": "Deleted {name}",
  "content.col.name": "Name",
  "content.col.id": "ID",
  "content.col.version": "Version",
  "content.col.loader": "Loader",
  "content.col.depends": "Depends",
  "content.dependsCount": "{n} required",
  "content.dependsHint":
    "Dependencies are grouped by kind (hover the count for the full list): required / optional / incompatible / embedded. “Incompatible” means the opposite of a dependency — installing it is what breaks the game.",
  "content.dep.required": "required",
  "content.dep.optional": "optional",
  "content.dep.incompatible": "incompatible",
  "content.dep.embedded": "embedded",
  "content.check": "Check dependencies & conflicts",
  "content.checkTitle": "Dependency check",
  "content.checkClean": "No missing required dependencies and no incompatible declarations matched.",
  "content.checkBlocked": "{a} and {b} declare an unconditional incompatibility: one of them should go",
  "content.checkMissing": "{by} needs {id} ({constraint}), which is not installed",
  "content.checkUncertain": "{a} declares incompatibility with {b} ({constraint}) — version range not evaluated, please check yourself",
  "content.checkNote":
    "Version ranges are not evaluated: the four metadata formats disagree on syntax (>=1.2 <2 / [1.21.4,1.21.5) / a bare version), and an evaluator that gets it wrong is worse than saying “not evaluated”. So this only answers the two questions that need no evaluation: is this id present, and is this incompatibility unconditional.",
  "content.col.size": "Size",
  "content.col.state": "State",
  "content.enabled": "Enabled",
  "content.disabled": "Disabled",
  "content.enableHint":
    "Only mods can be toggled: resource packs, shaders and worlds are switched by the game's own config files, so a launcher-side toggle would read as enabled without taking effect.",
  "content.dropHint": "Drop files here to import them into the current folder.",
  "content.metaUnknown": "metadata unreadable — showing the file name",

  "mods.searchPlaceholder": "Search mods…",
  "mods.search": "Search",
  "mods.empty": "No matching mods",
  "mods.downloads": "{n} downloads",
  "mods.online": "Install from Hoard",
  "mods.install": "Install",
  "mods.installing": "Downloading: {mb} / {total} MB",
  "mods.installed": "Installed {name}",
  "mods.installInto": "Installs into the current instance's “{kind}” folder.",
  "mods.pickInstanceFirst": "Pick an instance above before installing content into it.",
  "mods.noSavesInstall":
    "Worlds aren't distributed through Hoard; use “Import” with a world folder instead.",
  "mods.maybeIncompatible": "maybe incompatible",
  "mods.incompatible": "This version doesn't list {game} as supported",
  "mods.incompatibleConfirm":
    "{version} doesn't list {game} as supported. Install anyway? A mod for the wrong version usually crashes the game at startup.",

  "common.cancel": "Cancel",
  "nav.logs": "Logs",

  "instances.new": "New instance",
  "instances.name": "Name",
  "instances.version": "Game version",
  "instances.filterVersions": "Filter versions…",
  "instances.includeSnapshots": "Include snapshots",
  "instances.create": "Create",
  "instances.empty": "No instances yet. Click “New instance” and pick a game version.",
  "instances.install": "Install",
  "instances.installFirst": "Install this version first",
  "instances.launch": "Launch",
  "instances.stop": "Stop",
  "instances.delete": "Delete",
  "instances.launches": "launched {n}×",
  "instances.lastPlayed": "last played {time}",
  "instances.javaFound": "Java {version}: {path}",
  "instances.javaMissing": "No suitable Java found for this version (install a JDK, or set a path on the instance)",
  "instances.created": "Instance “{name}” created. Click “Install” to download the version files.",
  "instances.installed": "{version} installed: {downloaded} file(s) downloaded ({mb} MB), {skipped} already in place.",
  "instances.launched": "Launched (Java {java}). See the Logs page for output.",
  "instances.downloading": "Downloading: {files} file(s) done, {mb} MB so far · current {current}",
  "instances.working": "Working…",
  "instances.content": "Content",
  "instances.settings": "Settings",
  "instances.settingsTitle": "Instance settings",
  "instances.save": "Save settings",
  "instances.saved": "Settings saved (they apply at the next launch)",
  "instances.memoryMb": "Max memory (MiB)",
  "instances.javaPath": "Java executable (empty = detect from the version's requirement)",
  "instances.pickJava": "Browse…",
  "instances.javaPicked": "Picked: {path} (takes effect once you save)",
  "instances.extraJvm": "Extra JVM arguments (one per line)",
  "instances.extraGame": "Extra game arguments (one per line)",
  "instances.argsHint":
    "Arguments are passed through verbatim: JVM args before the main class, game args last (so they can override the version's own).",
  "instances.resolution": "Window size on launch",
  "instances.resolutionAuto": "Let the game decide",
  "instances.width": "Width",
  "instances.height": "Height",
  "instances.server": "Join a server on launch",
  "instances.serverHost": "Address",
  "instances.serverPort": "Port (empty = 25565)",
  "instances.demo": "Demo mode",
  "instances.duplicate": "Duplicate",
  "instances.duplicateName": "Name for the copy",
  "instances.duplicating": "Copying: {mb} MB of {total} MB",
  "instances.duplicated": "Duplicated as “{name}”",
  "instances.deleteConfirm":
    "Delete instance “{name}”? Its worlds, mods and settings go with it, and this cannot be undone.",
  "instances.export": "Export",
  "instances.exportWithSaves": "Include worlds",
  "instances.exportHint":
    "Without worlds the archive is for sharing an instance (settings + mods); with worlds it's a full backup. logs, crash-reports and natives are never packed.",
  "instances.exported": "Exported {files} file(s) ({mb} MB) to {path}",
  "instances.import": "Import instance",
  "instances.importTitle": "Confirm import",
  "instances.importWarning":
    "An instance's settings decide what runs at launch (Java path, JVM and game arguments). Only continue if you trust where this archive came from.",
  "instances.importArchive": "{name} · {version} · {files} files · {mb} MB",
  "instances.importJvm": "Custom JVM arguments: {args}",
  "instances.importGame": "Custom game arguments: {args}",
  "instances.importJava": "Sets a Java path (cleared on import; this machine detects its own)",
  "instances.importServer": "Joins a server on launch: {host}",
  "instances.importDemo": "Demo mode",
  "instances.importClean": "No custom launch arguments.",
  "instances.importConfirm": "Import",
  "instances.imported": "Imported instance “{name}”",
  "instances.modpack": "Install modpack",
  "instances.exportModpack": "Export as .mrpack",
  "instances.exportModpackVersion": "Pack version",
  "instances.exportModpackHint":
    "Leave blank to use the instance's version id. Mods with recorded provenance are written as a URL plus sha1 (small pack); everything else goes into overrides/ by value. Worlds, logs, crash reports and screenshots are never included.",
  "instances.exportedModpack": "Exported: {files} file(s) by reference, {overrides} in overrides · {mb} MB · {path}",
  "instances.exportStale": "{count} file(s) had stale provenance (changed on disk) and were packed by value",
  "instances.exportAllOverrides":
    "No file in this instance has recorded provenance, so everything was packed by value - the pack will be large but complete",
  "instances.modpackTitle": "Confirm modpack install",
  "instances.modpackWarning":
    "A modpack decides which files get downloaded, where they land in the game directory, and which loader version is installed. Only continue if you trust the source.",
  "instances.modpackSummary": "{name} · pack {version} · game {game}",
  "instances.modpackLoader": "Will install {loader} {version}",
  "instances.modpackNoLoader": "No mod loader (installs as vanilla)",
  "instances.modpackFiles": "Downloads {required} required + {optional} optional files · {mb} MB",
  "instances.modpackSkipped": "Skips {count} file(s) marked unsupported on the client",
  "instances.modpackOverrides": "{count} file(s) from the override folders will be written into the game directory",
  "instances.modpackUninstallable": "This pack cannot be installed: {reason}",
  "instances.modpackConfirm": "Install",
  "instances.modpackInstalled":
    "Installed modpack “{name}”: {downloaded} files · {mb} MB · {overrides} overridden",
  "instances.modpackOptionalNote": "{count} of them were marked optional by the author and were installed anyway",
  "instances.modpackOverwritten": "Overwrote {count} existing file(s): {files}",
  "instances.modpackFailed": "{count} file(s) failed to download: {files}",
  "instances.modpackDeps":
    "Dependency check: {blocked} incompatibility(ies), {missing} missing (open the Content page and run “Check dependencies & conflicts” for details)",
  "instances.loader": "Mod loader",
  "instances.loaderTitle": "Mod loader",
  "instances.loaderCurrent": "Current: {kind} {version} (game {game})",
  "instances.loaderNone": "Vanilla (no loader)",
  "instances.loaderVersion": "Loader version",
  "instances.loaderLoading": "Loading…",
  "instances.loaderNoVersions": "No loader versions for {game}",
  "instances.loaderStable": "stable",
  "instances.loaderDev": "dev",
  "instances.loaderHint":
    "The game version is fixed to {game} (taken from the instance, not editable — a wrong one only blows up at launch). After installing, the instance points at the loader version; vanilla stays in the shared cache.",
  "instances.loaderInstall": "Install loader",
  "instances.loaderChange": "Change loader version",
  "instances.loaderRemove": "Remove loader (back to vanilla)",
  "instances.loaderInstalled": "Installed {kind} {version}",
  "instances.loaderRemoved": "Back to vanilla",

  "logs.title": "Game log",
  "logs.hint":
    "Live stdout/stderr from the game process (last 2000 lines kept, recorded from app start); archives listed below can be opened too. Console output is plain text rather than Mojang's XML layout, so the live stream, the archives and crash reports all share one parser and one set of diagnosis rules.",
  "logs.clear": "Clear",
  "logs.empty": "No log yet. Launch an instance and its output shows up here live.",
  "logs.exited": "{instance} exited with code {code}.",
  "logs.stopped": "{instance} was stopped.",
  "logs.instance": "Instance",
  "logs.refresh": "Re-diagnose",
  "logs.copy": "Copy all",
  "logs.openDir": "Open directory",
  "logs.search": "Search…",
  "logs.follow": "Follow tail",
  "logs.truncated":
    "The file is large, so only its tail is shown here. Open the log directory for the whole thing.",
  "logs.diagnosis": "Crash diagnosis",
  "logs.diagnosisLoading": "Loading…",
  "logs.diagnosisNoLogs": "{instance} has no logs yet — it may never have been launched.",
  "logs.diagnosisClean": "No known problem found in {source}.",
  "logs.diagnosisSource": "Based on: {source}",
  "logs.diagnosisPartial":
    "The file is above the diagnosis limit, so only its tail was read: early warnings may have been missed.",
  "logs.severity.fatal": "did not start",
  "logs.severity.warning": "still running, but something is off",
  "logs.level.fatal": "FATAL",
  "logs.level.error": "ERROR",
  "logs.level.warn": "WARN",
  "logs.level.info": "INFO",
  "logs.level.debug": "DEBUG",
  "logs.level.trace": "TRACE",
  "logs.level.other": "other",
  "logs.archives": "Log archives",
  "logs.archivesHint":
    "The game's own rolled logs and crash reports. Measured fact: common failures do not write a crash report at all (JVM startup and the loader's mod resolution both happen outside the reporter), so an empty crash-reports list does not mean nothing went wrong.",
  "logs.archivesEmpty": "No archives yet.",
  "logs.archiveEmpty": "This log is empty.",
  "logs.archiveColName": "File",
  "logs.archiveColKind": "Kind",
  "logs.archiveColSize": "Size",
  "logs.archiveOpen": "View",
  "logs.archiveClose": "Back to live",
  "logs.kind.gameLog": "Game log",
  "logs.kind.crashReport": "Crash report",
  "logs.oplog": "Launcher operation log",
  "logs.oplogHint":
    "What the launcher itself did: which version it installed, which Java it launched with, what the exit code was. Include this when reporting a problem — for failures that happen before the game starts, there is no game log at all. It contains no secrets (no tokens, no launcher passwords).",
  "logs.oplogEmpty": "Nothing recorded yet.",

  "diagnose.java_version_mismatch":
    "Wrong Java version: this game version needs Java {major}. Several JDKs can coexist on one machine; pick the right one in the instance settings.",
  "diagnose.heap_misconfigured":
    "Contradictory memory settings (initial heap larger than maximum heap), so the JVM refused to start. Check the instance's memory setting and custom JVM arguments.",
  "diagnose.out_of_memory": "Out of memory: the Java heap was exhausted. Allocate more to this instance.",
  "diagnose.fabric_missing_mod": "Missing mod: {id}. Install it (or the mod that provides it) and try again.",
  "diagnose.fabric_mod_version_conflict":
    "A mod is incompatible with this game version or with another mod. The log says which dependency and which version range it wanted.",
  "diagnose.fabric_mod_resolution_failed":
    "Fabric's mod resolution failed, but not for one of the two common reasons. See the Reason line in the log.",
  "diagnose.loader_mismatch_skipped":
    "A mod was built for a different loader and was skipped — the game still starts, but that mod does nothing. Fabric mods cannot go into a Forge/NeoForge instance, or vice versa.",
  "diagnose.mod_jar_skipped":
    "A mod jar could not be read and was skipped (usually because it was built for a different game version) — the game still starts, but that mod does nothing.",
  "diagnose.mojang_user_properties_401":
    "The game got a 401 fetching user properties from Mojang. With an account from our mirror site this is expected: the token was issued by the mirror, Mojang does not know it, and the game carries on. Safe to ignore.",
  "diagnose.crash_report_unrecognized":
    "The game wrote a crash report, but the cause is not one of the known rules. Summary below; the full report is in the log directory.",
  "diagnose.action.installJava": "Get Java {major}",
  "diagnose.action.installMod": "Find {id}",
  "diagnose.action.adjustMemory": "Adjust memory",
  "diagnose.action.openContent": "Manage content",
  "diagnose.action.openLogs": "See the full log",

  "settings.servers": "Servers",
  "settings.apiUrl": "Mirror (skin site / accounts)",
  "settings.hoardUrl": "Hoard (mod distribution)",
  "settings.appearance": "Appearance",
  "settings.language": "Language",
  "settings.languageAuto": "System default",
  "settings.theme": "Theme",
  "settings.themeSystem": "System",
  "settings.themeDark": "Dark",
  "settings.themeLight": "Light",
  "settings.downloads": "Downloads",
  "settings.concurrency": "Concurrency",
  "settings.mirror": "Mirror prefix (empty = official sources)",
  "settings.defaults": "Defaults",
  "settings.memoryMb": "Max memory for new instances (MiB)",
  "settings.save": "Save",
  "settings.saved": "Saved",
  "settings.accounts": "Accounts",
  "settings.accountsStoreKeyring": "Tokens are kept in the system keyring.",
  "settings.accountsStorePlain":
    "Tokens are stored in plain text at {path} (mode 0600, separate from the account list). This is not encryption: anything on this machine that can read that file can use the token. Know this until the system keyring is wired up.",
  "settings.accountsEmpty":
    "No accounts yet. Sign in to your mirror site above, or add an offline account below.",
  "settings.accountColName": "Player",
  "settings.accountColKind": "Type",
  "settings.accountColSite": "Site",
  "settings.accountActive": "active",
  "settings.accountKind.yggdrasil": "Mirror",
  "settings.accountKind.offline": "Offline",
  "settings.accountUse": "Switch",
  "settings.accountRemove": "Remove",
  "settings.accountRemoveConfirm": "Remove the account “{name}” and delete its token?",
  "settings.accountOfflineName": "Offline player name",
  "settings.accountAddOffline": "Add offline account",
  "settings.accountOfflineHint":
    "An offline account needs no network and has no token; its UUID is derived from the name with the same algorithm vanilla servers use. ASCII letters, digits and underscore only, up to 16 characters. Official servers will reject it.",
  "settings.credentials": "Launcher passwords",
  "settings.credentialsHint":
    "Launcher sign-in takes a player name plus that player's own launcher password; the site's main password does not work on this endpoint. Current player: {name}. One per device is the intent, and the site sets the cap (3 by default).",
  "settings.credentialsLoading": "Loading…",
  "settings.credentialsEmpty":
    "This player has no launcher password yet — until one is created it cannot sign in from any launcher, including this one.",
  "settings.credentialsColLabel": "Label",
  "settings.credentialsColCreated": "Created",
  "settings.credentialsColUsed": "Last used",
  "settings.credentialsNeverUsed": "never",
  "settings.credentialsLabel": "Label",
  "settings.credentialsLabelPlaceholder": "Desktop",
  "settings.credentialsPassword": "Launcher password",
  "settings.credentialsCreate": "Create",
  "settings.credentialsRevoke": "Revoke",
  "settings.credentialsRevokeConfirm":
    "Revoke the launcher password “{name}”? Sessions created with it die immediately — if it is the one this device is using, you will be signed out too.",
  "settings.credentialsPasswordHint":
    "The plaintext exists only for this one submission: it is never written to disk, never logged, and cleared from the field as soon as you submit. Create and revoke both talk to the site directly; the launcher never keeps a copy.",
  "settings.credentialsOffline": "An offline account has no site, so there are no launcher passwords to manage.",
  "settings.java": "Java",
  "settings.javaHint":
    "The launcher picks Java by what the game version asks for: exactly Java 8 up to 1.16.5, 17 for 1.17–1.20.4, 21 from 1.20.5 on. If nothing suitable is on this machine it downloads the official runtime (~140MB, progress shown below). You can also pin a path per instance in its settings.",
  "settings.javaDownloading": "Downloading Java: {done} MB / {total} MB",
  "settings.javaDetected": "Detected on this machine",
  "settings.javaNoneDetected":
    "No usable Java found (every candidate was really run with `java -version`). Download an official runtime below, or point an instance at one in its settings.",
  "settings.javaColVersion": "Version",
  "settings.javaColPath": "Path",
  "settings.javaOfficial": "Official runtimes",
  "settings.javaOfficialHint":
    "From Mojang's java-runtime manifest, installed into cache/java/. The product names are Mojang's and do not map one-to-one onto Java versions (measured: gamma is 17, delta is 21), so read the version column.",
  "settings.javaColComponent": "Product",
  "settings.javaColOfferVersion": "Version",
  "settings.javaDownload": "Download",
  "settings.javaReinstall": "Reinstall",
  "settings.javaRemove": "Delete",
  "settings.javaRemoveConfirm":
    "Delete the downloaded {component}? Only the copy under cache/java/ is removed; your own JDK installs are untouched.",
  "settings.javaNoOfficialForPlatform":
    "Mojang publishes no official runtime for this platform (Linux arm64 is absent from the manifest entirely; Apple Silicon has no Java 8 or 16). Install a suitable JDK yourself and point the instance at it in its settings.",
  "settings.javaIndexUnavailable":
    "The official runtime manifest could not be fetched right now ({error}). Whatever is already in cache/java/ still works; retry, or check the network and mirror settings.",
  "settings.update": "Updates",
  "settings.updateCheck": "Check for updates",
  "settings.updateUnconfigured":
    "This build has no auto-update (the update signing public key was not injected). Builds from source are like this on purpose: the signing private key has to be generated and kept by whoever releases, never minted by the build. Grab new versions from the release page.",
  "settings.updateCurrent": "You are on the latest version ({version}).",
  "settings.updateChecking": "Checking…",
  "settings.updateInstalling": "Downloading and installing…",
  "settings.updateInstalled": "{version} is downloaded. Restart the launcher to apply it.",
  "settings.updateError": "The update check failed: {error}",
  "settings.updateAvailable": "Version {version} is available.",
  "settings.updateInstall": "Download and install",
  "settings.paths": "Data directories",
  "settings.pathsHint":
    "Assets, libraries, versions and JREs are shared globally (instances never re-download them); instances stay isolated.",
  "settings.openDataDir": "Open data directory",
  "settings.startupWarning": "Startup notice",

  "errors.network": "Network error. Check the server address and connectivity",
  "errors.api": "The server returned an error",
  "errors.invalid": "Invalid input",
  "errors.config": "Failed to read or write the configuration",
  "errors.io": "Failed to read or write a local file",
  "errors.not_found": "Not found (it may have been deleted or moved, or that version isn't installed yet)",
  "errors.auth": "Your session expired. Please sign in again",
  "errors.binding_required":
    "This site requires a linked third-party account. Link one on the site's account page",
  "errors.password_login_disabled":
    "Password sign-in is disabled on this site (launchers are unaffected — use a player name and its launcher password)",
  "errors.email_not_verified": "The email address is not verified yet; launcher sign-in needs that first",
  // The site deliberately does not tell "wrong password" apart from "this player has no launcher
  // password yet" (telling them apart would let anyone probe which player names exist), so this
  // copy has to cover both — and the latter is the most common first failure after an upgrade.
  "errors.invalid_credentials":
    "Wrong player name or launcher password. Note that the launcher only accepts that player's launcher password, not the site's main password; if this player has none yet, one has to be created on the website first.",
  "errors.web_session_required":
    "That has to be done on the website's account page: a launcher session is only valid for the one player it was issued for",
  "errors.action.bindings": "Link a third-party account",
  "errors.action.account": "Open the account page",
  "errors.action.credentials": "Create a launcher password",
  "errors.cli_unknown": "Unrecognized command-line arguments: {args} (ignored; the launcher started normally)",
  "errors.unknown": "Something went wrong",
};

const tables: Record<Locale, Messages> = { "zh-CN": zhCN, en };

/** 后端错误码 → errors.* 键；未知码落到 errors.api / errors.unknown。 */
export function errorKey(code: string): string {
  const key = `errors.${code}`;
  return key in zhCN ? key : code === "unknown" ? "errors.unknown" : "errors.api";
}

export function detectLocale(preference: string): Locale {
  if (preference === "zh-CN" || preference === "en") return preference;
  const nav = typeof navigator === "undefined" ? "" : navigator.language;
  return nav.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

interface I18nValue {
  locale: Locale;
  t: (key: string, vars?: Record<string, string | number>) => string;
}

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({
  locale,
  children,
}: {
  locale: Locale;
  children: ReactNode;
}) {
  // <html lang> 跟随应用语言：屏幕阅读器与浏览器翻译据此判断，静态 index.html 写不死
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const t = useCallback(
    (key: string, vars?: Record<string, string | number>) => {
      const table = tables[locale];
      let text = table[key] ?? tables.en[key] ?? key;
      for (const [k, v] of Object.entries(vars ?? {})) {
        text = text.split(`{${k}}`).join(String(v));
      }
      return text;
    },
    [locale],
  );
  const value = useMemo(() => ({ locale, t }), [locale, t]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
