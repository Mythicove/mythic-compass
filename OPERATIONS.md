# 使用、恢复与发布

## 首次使用

1. 安装对应系统的包，在设置中填写社区提供的 Mirror 与 Hoard 地址。
2. 使用玩家名与该玩家的**启动器独立密码**登录。网站主密码不能用于启动器。
3. 在实例页创建实例并安装版本，或打开 `.mrpack`，检查确认页后安装。
4. 选择实例启动。没有匹配运行时时，启动器会下载官方 Java；也可在实例设置中指定 Java。
5. 内容页管理模组、资源包和存档；日志页查看实际游戏输出、归档和诊断。

离线账号用于本地离线游戏，不能替代在线服务器认证。设置页可切换语言和主题，
并展示实际数据目录。`portable.txt` 放在可执行文件旁时，数据位于同目录 `data/`。

## 下载与故障恢复

- 网络失败会进行有限重试，未完成文件保留为 `.part`；重新安装时下载器尝试续传。
- 服务端忽略 Range 时重新写完整响应。HTTP 416 本身不证明本地文件完整：
  有已知大小或 SHA-1 时校验，否则删除旧 partial 后重新下载。
- 校验失败的 partial 会被丢弃；只有通过可用校验的文件才提升为正式文件。
- 当前没有安装下载的暂停/取消 UI。任务 Future 取消的锁释放已测试，不能将其描述为玩家可点击取消。
- 磁盘不足时先关闭游戏和安装任务，在设置显示的目录检查可用空间。
  优先转移不需要的实例导出包或备份；不要在任务运行中手工删除共享缓存。
- 登录失败先核对服务器地址、玩家名和独立密码。服务暂时不可用不等于密码错误；
  恢复网络后重试。退出登录会移除对应本地令牌。

求助时提供版本、系统、失败操作、错误码及日志页相关内容。分享前去掉私人地址、
玩家信息或路径。不要上传 `secrets.json`、密码或整份个人数据目录。
目前令牌是分离存储的文件，尚未接入系统钥匙串；`accounts.json` 只保存账号身份信息。

## 备份与升级

退出游戏与启动器后备份实例目录及 `compass.json`。实例 ZIP 导出支持是否包含世界；
恢复时先审阅导入确认页中的 JVM 参数、游戏参数和服务器设置，再确认导入。
独立保存账号密码；不要依赖备份令牌长期有效。

当前草稿包用于验收。正式发布前检查三平台包、SHA256SUMS、安装和卸载、
旧版本覆盖安装后实例与配置保留、自动更新签名验证。自动更新需要同时配置：

- `TAURI_SIGNING_PUBLIC_KEY`
- `TAURI_SIGNING_PRIVATE_KEY`
- 私钥有密码时的 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

Apple 签名与公证还需要 `APPLE_CERTIFICATE`、`APPLE_CERTIFICATE_PASSWORD`、
`APPLE_SIGNING_IDENTITY`、`APPLE_ID`、`APPLE_PASSWORD` 和 `APPLE_TEAM_ID`。
密钥由发布负责人管理并放入 GitHub secrets；未配置时生成的草稿不能算签名升级验收通过。
Windows 当前包未代码签名，系统可能显示未知发布者提示。

## 可重复验收

Rust 核心恢复场景：

```sh
cd src-tauri
cargo test --locked --test metadata_transport
```

覆盖取消后锁释放、共享写入、截断响应恢复、不可用源重试与恢复、416 拒绝和未知大小进度。
HTTP 服务位于测试边界，文件下载与校验走生产 Downloader。

Linux 原生矩阵需要 python3-pyatspi、X11/XTest、GTK/WebKit、dbus、Xvfb，
以及中文正文所需的系统 CJK 字体（例如 fonts-noto-cjk）：

```sh
dbus-run-session -- xvfb-run -a -s '-screen 0 1280x900x24' \
  python3 src-tauri/tests/linux-desktop.py /absolute/path/to/compass /absent/output
```

脚本通过所属 PID 的 AT-SPI 树操作五页，覆盖中英、深浅色及 1200/760 宽度，
创建离线账号并核对持久化。结果包含截图、exe SHA-256 和四次进程 UI 就绪耗时。
软件渲染、已有系统缓存与 locale 不同，四次耗时仅作测量记录，不是严格冷启动基准。
它不覆盖在线进服，也不自动证明所有控件无横向溢出；截图需要复查。

Windows 完整在线业务和文件对话框场景见相邻 Web 仓库 `e2e/README.md`；
macOS 包启动与窗口场景为 `src-tauri/tests/macos-desktop.swift`。
