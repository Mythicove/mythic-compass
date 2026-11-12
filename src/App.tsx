import { createContext, useCallback, useContext, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { I18nProvider, detectLocale, errorKey, useI18n, type Locale } from "./i18n";
import {
  EVENTS,
  ipc,
  toIpcError,
  type AccountView,
  type Config,
  type GameExit,
  type LogLine,
} from "./lib/ipc";
import { StartView } from "./views/start";
import { InstancesView } from "./views/instances";
import { ContentView } from "./views/content";
import { SettingsView } from "./views/settings";
import { LogsView } from "./views/logs";
import "./shell.css";

type View = "home" | "instances" | "content" | "logs" | "settings";

/** 一个错误 = 稳定码 + 已本地化的文案。留着码是因为有些错误要配一个能点的动作。 */
interface UiError {
  code: string;
  message: string;
}

/** 光说清楚还不够、得给一个动作的错误。**按稳定码匹配，不匹配文案**——
 *  匹配文案的话改一个字就会让按钮消失，而且看不出是哪儿断的。
 *  路径拼在 `config.apiUrl` 上：部署是单入口网关（web 与 mirror 同源），
 *  且 Rust 侧的 `normalize_base_url` 已经去过尾斜杠。 */
const ERROR_ACTIONS: Record<string, { labelKey: string; path: string }> = {
  // 站点要求先绑定第三方。绑定只能在网站的浏览器授权流里完成（启动器参与不了回跳），
  // 所以这里能做的最好的事就是把人送到那一页，而不是留一句 403。
  binding_required: { labelKey: "errors.action.bindings", path: "/user/bindings" },
  // 邮箱未验证：验证链接在邮件里，账号页可以重发
  email_not_verified: { labelKey: "errors.action.account", path: "/user" },
  // 名字或独立密码不对。mirror 故意不区分"密码错"与"这个玩家还没有独立密码"
  // （一区分就能拿玩家名探测账号是否存在），所以文案要覆盖两种可能——而后者是
  // ADR-0012 之后最常见的首次失败：存量用户在网站上补建独立密码之前登不进启动器。
  invalid_credentials: { labelKey: "errors.action.credentials", path: "/user/security" },
};

// 游戏日志的模块级环形缓冲：订阅在 App 挂载就开始，早于用户打开日志页——
// 否则启动头几秒的输出（往往正是崩溃原因）会丢。
const LOG_CAP = 2000;
const logState: { lines: LogLine[]; exit: GameExit | null } = { lines: [], exit: null };
const logListeners = new Set<() => void>();

function notifyLogListeners() {
  for (const fn of logListeners) fn();
}

export interface GameLogsSnapshot {
  lines: LogLine[];
  exit: GameExit | null;
}

export function snapshotGameLogs(): GameLogsSnapshot {
  return { lines: logState.lines.slice(), exit: logState.exit };
}

export function clearGameLogs() {
  logState.lines = [];
  logState.exit = null;
  notifyLogListeners();
}

/** LogsView 用它渲染：首帧拿到缓冲快照，之后每次追加/清空都会重新渲染。 */
export function useGameLogs(): GameLogsSnapshot {
  const [snapshot, setSnapshot] = useState(snapshotGameLogs);
  useEffect(
    () => {
      const update = () => setSnapshot(snapshotGameLogs());
      logListeners.add(update);
      update(); // 挂载前的行也要拿到
      return () => {
        logListeners.delete(update);
      };
    },
    [],
  );
  return snapshot;
}

const VIEWS: { id: View; labelKey: string }[] = [
  { id: "home", labelKey: "nav.home" },
  { id: "instances", labelKey: "nav.instances" },
  { id: "content", labelKey: "nav.content" },
  { id: "logs", labelKey: "nav.logs" },
  { id: "settings", labelKey: "nav.settings" },
];

// 设置页读写全局配置；语言与主题变化要立刻反映到整个应用，故提到这一层。
const ConfigContext = createContext<{
  config: Config | null;
  onConfigSaved: (c: Config) => void;
}>({ config: null, onConfigSaved: () => {} });

export function useConfig() {
  return useContext(ConfigContext);
}

/** 把主题落到 <html data-theme>；system 时跟随系统并订阅其变化。 */
function useTheme(theme: Config["theme"] | undefined) {
  useEffect(() => {
    const root = document.documentElement;
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved =
        theme === "system" || theme === undefined
          ? media.matches
            ? "dark"
            : "light"
          : theme;
      root.dataset.theme = resolved;
      root.style.colorScheme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}

function Shell() {
  const { t } = useI18n();
  const { config } = useConfig();
  const [view, setView] = useState<View>("home");
  // 从实例页点「内容」跳过来时带上是哪个实例；直接点侧栏时为空，由内容页自己选。
  const [contentSlug, setContentSlug] = useState("");
  /** 双击 .mrpack（或 --import）带进来的路径，交给实例页预填导入表单 */
  const [importPath, setImportPath] = useState<string | null>(null);
  const [account, setAccount] = useState<AccountView | null>(null);
  const [checking, setChecking] = useState(true);
  const [error, setError] = useState<UiError | null>(null);

  /** 带上稳定码，因为有些错误要配一个动作（`binding_required` → 打开网站的绑定页）。
   *  只存本地化文案的话，"这句话该不该带按钮"就得靠匹配文案来判断——那正是这个项目
   *  在别处已经明确避开的做法。 */
  const fail = (e: unknown) => {
    const code = toIpcError(e).code;
    setError({ code, message: t(errorKey(code)) });
  };

  // 启动时校验一次本地令牌：失效则清掉登录态，网络错误仍算已登录（Rust 侧的语义）
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const checked = await ipc.authCheck();
        if (cancelled) return;
        setAccount(checked.account);
        // "本来登录着、但令牌已失效"必须说出来：否则用户看到的是启动器莫名其妙
        // 回到了登录界面，而自己刚才明明还登录着
        if (checked.expired) setError({ code: "auth", message: t("errors.auth") });
      } catch (e) {
        if (!cancelled) {
          const code = toIpcError(e).code;
          setError({ code, message: t(errorKey(code)) });
        }
      } finally {
        if (!cancelled) setChecking(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [t]);

  // 切页面时重读账号列表：设置页里可能切了账号、加了离线账号或删了账号，
  // 侧栏与首页要跟着变。用 accountsList 而不是 authCheck——后者会打一次服务端校验，
  // 每次导航都校验既慢又没必要（启动时已经校验过）。依赖里带上 checking：
  // 启动时 authCheck 可能判定令牌失效并删掉账号，等它结束后再读一次才是准的。
  useEffect(() => {
    let cancelled = false;
    void ipc
      .accountsList()
      .then((list) => {
        if (cancelled) return;
        const active = list.accounts.find((a) => a.id === list.activeId);
        setAccount(
          active
            ? {
                playerName: active.playerName,
                playerUuid: active.playerUuid,
                apiUrl: active.apiUrl,
                signedInAt: active.signedInAt,
                kind: active.kind,
              }
            : null,
        );
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [view, checking]);

  const signIn = async (playerName: string, password: string) => {
    setError(null);
    try {
      setAccount(await ipc.authLogin(playerName, password));
    } catch (e) {
      fail(e);
    }
  };

  const signOut = async () => {
    setError(null);
    try {
      await ipc.authLogout();
      setAccount(null);
    } catch (e) {
      fail(e);
    }
  };

  const errorAction = error ? ERROR_ACTIONS[error.code] : undefined;

  /** 命令行意图（也可能是双击 `.mrpack` 带进来的路径）。只在启动时读一次。 */
  useEffect(() => {
    let cancelled = false;
    void ipc
      .cliIntent()
      .then((got) => {
        if (cancelled) return;
        // 打错的参数必须显示出来：静默忽略的话，用户会以为命令生效了，
        // 然后去查一个不存在的问题（比如"为什么 --launch 没启动游戏"）
        if (got.unknown.length > 0) {
          setError({ code: "cli_unknown", message: t("errors.cli_unknown", { args: got.unknown.join(" ") }) });
        }
        if (got.importModpack) {
          setImportPath(got.importModpack);
          setView("instances");
        } else if (got.instance && got.launch) {
          // 直接启动。没登录时会失败并显示原因——那是如实的，
          // 比"命令行启动"悄悄什么都不做要好
          void ipc.instanceLaunch(got.instance).catch((e) => {
            if (!cancelled) {
              const code = toIpcError(e).code;
              setError({ code, message: t(errorKey(code)) });
            }
          });
        } else if (got.instance) {
          setView("instances");
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [t]);

  return (
    <div className="shell">
      <nav className="sidebar">
        <div className="brand">
          <span className="dot" />
          <span>{t("app.name")}</span>
        </div>
        {VIEWS.map((v) => (
          <button
            key={v.id}
            className={view === v.id ? "nav-item active" : "nav-item"}
            onClick={() => setView(v.id)}
          >
            {t(v.labelKey)}
          </button>
        ))}
        <div className="sidebar-footer">
          {account ? (
            <>
              <span className="who" title={account.playerUuid}>
                {account.playerName}
              </span>
              <button className="link" onClick={() => void signOut()}>
                {t("account.signOut")}
              </button>
            </>
          ) : (
            <span className="muted">{t("account.notSignedIn")}</span>
          )}
        </div>
      </nav>

      <main className="content">
        {error && (
          <div className="error" role="alert">
            <span>{error.message}</span>
            {errorAction && config?.apiUrl && (
              <button
                type="button"
                className="link"
                onClick={() => void openUrl(`${config.apiUrl}${errorAction.path}`)}
              >
                {t(errorAction.labelKey)}
              </button>
            )}
          </div>
        )}
        {view === "home" && (
          <StartView account={account} checking={checking} onSignIn={signIn} />
        )}
        {view === "instances" && (
          <InstancesView
            initialImportPath={importPath}
            onImportPathConsumed={() => setImportPath(null)}
            onOpenContent={(slug) => {
              setContentSlug(slug);
              setView("content");
            }}
          />
        )}
        {view === "content" && <ContentView initialSlug={contentSlug} />}
        {view === "logs" && (
          <LogsView
            onOpenContent={(s) => {
              setContentSlug(s);
              setView("content");
            }}
            onOpenInstances={() => setView("instances")}
          />
        )}
        {view === "settings" && <SettingsView />}
      </main>
    </div>
  );
}

export default function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [locale, setLocale] = useState<Locale>(() => detectLocale(""));

  useTheme(config?.theme);

  // 日志/退出事件在这里订阅（而不是 LogsView）：日志页没打开时输出也进缓冲
  useEffect(() => {
    const un = [
      listen<LogLine>(EVENTS.gameLog, (e) => {
        logState.lines.push(e.payload);
        if (logState.lines.length > LOG_CAP) {
          logState.lines.splice(0, logState.lines.length - LOG_CAP);
        }
        notifyLogListeners();
      }),
      listen<GameExit>(EVENTS.gameExit, (e) => {
        logState.exit = e.payload;
        notifyLogListeners();
      }),
    ];
    return () => {
      void Promise.all(un).then((fs) => fs.forEach((f) => f()));
    };
  }, []);

  useEffect(() => {
    ipc
      .getConfig()
      .then((c) => {
        setConfig(c);
        setLocale(detectLocale(c.language));
      })
      // 配置读不出来也要能进设置页改地址，故不阻塞渲染
      .catch(() => setConfig(null));
  }, []);

  const onConfigSaved = useCallback((c: Config) => {
    setConfig(c);
    setLocale(detectLocale(c.language));
  }, []);

  return (
    <I18nProvider locale={locale}>
      <ConfigContext.Provider value={{ config, onConfigSaved }}>
        <Shell />
      </ConfigContext.Provider>
    </I18nProvider>
  );
}
