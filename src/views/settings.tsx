import { useCallback, useEffect, useState, type FormEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { check as checkForUpdate } from "@tauri-apps/plugin-updater";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { errorKey, useI18n } from "../i18n";
import { useConfig } from "../App";
import {
  EVENTS,
  ipc,
  toIpcError,
  type AccountEntry,
  type AccountsView,
  type Config,
  type CredentialView,
  type FileProgress,
  type JavaOverview,
  type PathsInfo,
} from "../lib/ipc";

const PATH_FIELDS: { key: keyof PathsInfo; label: string }[] = [
  { key: "root", label: "root" },
  { key: "configFile", label: "compass.json" },
  { key: "accountsFile", label: "accounts.json" },
  { key: "secretsFile", label: "secrets.json" },
  { key: "cache", label: "cache/" },
  { key: "assets", label: "cache/assets/" },
  { key: "libraries", label: "cache/libraries/" },
  { key: "versions", label: "cache/versions/" },
  { key: "java", label: "cache/java/" },
  { key: "tools", label: "cache/tools/" },
  { key: "instances", label: "instances/" },
];

export function SettingsView() {
  const { t } = useI18n();
  const { config, onConfigSaved } = useConfig();
  const [form, setForm] = useState<Config | null>(config);
  const [paths, setPaths] = useState<PathsInfo | null>(null);
  const [warning, setWarning] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accounts, setAccounts] = useState<AccountsView | null>(null);
  const [offlineName, setOfflineName] = useState("");
  const [accountBusy, setAccountBusy] = useState<string | null>(null);
  const [accountError, setAccountError] = useState<string | null>(null);
  const [credentials, setCredentials] = useState<CredentialView[] | null>(null);
  const [credBusy, setCredBusy] = useState<string | null>(null);
  const [credError, setCredError] = useState<string | null>(null);
  const [credLabel, setCredLabel] = useState("");
  const [credPassword, setCredPassword] = useState("");
  const [java, setJava] = useState<JavaOverview | null>(null);
  const [javaBusy, setJavaBusy] = useState<string | null>(null);
  const [javaError, setJavaError] = useState<string | null>(null);
  const [javaProgress, setJavaProgress] = useState<FileProgress | null>(null);
  /** 自动更新的状态机。`unconfigured` 与 `error` 必须是两档：
   *  前者是"这个构建没接自动更新"（正常，本地构建就是这样），后者是"查了但失败了"（该重试）。
   *  合成一句"检查更新失败"会让本地构建看起来像坏了。 */
  const [appVersion, setAppVersion] = useState("");
  const [update, setUpdate] = useState<{
    status:
      | "unknown"
      | "unconfigured"
      | "checking"
      | "current"
      | "available"
      | "installing"
      | "installed"
      | "error";
    version?: string;
    notes?: string;
    error?: string;
  }>({ status: "unknown" });

  const reloadAccounts = useCallback(async () => {
    try {
      setAccounts(await ipc.accountsList());
    } catch {
      setAccounts(null);
    }
  }, []);

  /** 独立密码只对站点账号有意义：离线账号既没有站点也没有令牌。
   *  Rust 侧也拦，但这里先不渲染面板——给用户一个点了才报错的按钮是没必要的。 */
  const activeEntry = accounts?.accounts.find((a) => a.id === accounts.activeId) ?? null;
  const siteAccount = activeEntry?.kind === "yggdrasil" ? activeEntry : null;
  const siteAccountId = siteAccount?.id ?? null;

  const reloadCredentials = useCallback(async () => {
    if (!siteAccountId) {
      setCredentials(null);
      return;
    }
    try {
      setCredentials(await ipc.credentialsList());
      setCredError(null);
    } catch (e) {
      // 读不到就显示空列表 + 原因，而不是让整块面板消失：
      // "这个玩家有没有独立密码"正是用户来这里想知道的事
      setCredentials([]);
      setCredError(t(errorKey(toIpcError(e).code)));
    }
  }, [siteAccountId, t]);

  /** 账号操作的统一包装：一次只允许一个在跑，失败要显示出来说而不是静默。 */
  const accountAct = async (key: string, fn: () => Promise<void>) => {
    if (accountBusy) return;
    setAccountBusy(key);
    setAccountError(null);
    try {
      await fn();
      await reloadAccounts();
    } catch (e) {
      setAccountError(t(errorKey(toIpcError(e).code)));
    } finally {
      setAccountBusy(null);
    }
  };

  const switchAccount = (id: string) =>
    accountAct(`switch:${id}`, async () => {
      await ipc.accountSetActive(id);
    });

  const addOffline = () =>
    accountAct("add-offline", async () => {
      await ipc.accountAddOffline(offlineName.trim());
      setOfflineName("");
    });

  const removeAccount = (a: AccountEntry) =>
    accountAct(`remove:${a.id}`, async () => {
      if (!window.confirm(t("settings.accountRemoveConfirm", { name: a.playerName }))) return;
      await ipc.accountRemove(a.id);
    });

  /** 独立密码操作的统一包装。与 accountAct 分开是因为失败要显示在**这一块**里，
   *  而不是混进账号列表的错误位——两块面板的补救动作完全不同。 */
  const credAct = async (key: string, fn: () => Promise<void>) => {
    if (credBusy) return;
    setCredBusy(key);
    setCredError(null);
    try {
      await fn();
      await reloadCredentials();
    } catch (e) {
      setCredError(t(errorKey(toIpcError(e).code)));
    } finally {
      setCredBusy(null);
    }
  };

  const createCredential = () =>
    credAct("create", async () => {
      const label = credLabel.trim();
      const password = credPassword;
      // 明文在离开这个函数之前就从 state 里清掉：请求失败也不该让它继续躺在
      // React state 里（更不该被下一次渲染写回 DOM）。代价是失败时要重输一遍，
      // 而对一个密码字段来说这个代价是对的。
      setCredPassword("");
      await ipc.credentialsCreate(label, password);
      setCredLabel("");
    });

  const revokeCredential = (c: CredentialView) =>
    credAct(`revoke:${c.id}`, async () => {
      if (!window.confirm(t("settings.credentialsRevokeConfirm", { name: c.name }))) return;
      await ipc.credentialsDelete(c.id);
    });

  useEffect(() => setForm(config), [config]);

  useEffect(() => {
    void ipc.pathsInfo().then(setPaths).catch(() => setPaths(null));
    // 启动期的问题（如损坏的 compass.json 已回落默认值）只在第一次进设置页时提示一次
    void ipc
      .takeStartupWarning()
      .then(setWarning)
      .catch(() => setWarning(null));
    void reloadAccounts();
  }, [reloadAccounts]);

  // 切换当前账号时要跟着换一批独立密码（它们是按玩家的，不是按站点的）
  useEffect(() => {
    void reloadCredentials();
  }, [reloadCredentials]);

  /** Java 管理页的数据。探测要真跑 `java -version`（一个坏掉的 JDK 或网络盘上的
   *  java.exe 会让"路径存在"这个判断毫无意义），所以这一步可能慢，要有加载态。 */
  const reloadJava = useCallback(async () => {
    try {
      setJava(await ipc.javaOverview());
      setJavaError(null);
    } catch (e) {
      setJavaError(t(errorKey(toIpcError(e).code)));
    }
  }, [t]);

  useEffect(() => {
    void reloadJava();
  }, [reloadJava]);

  // 下载进度只在**我们正在下载**时显示：progress 事件是全局的，
  // 版本安装也会走它，不加以区分的话这里会显示一个与本页无关的进度条。
  useEffect(() => {
    let un: (() => void) | undefined;
    let cancelled = false;
    void listen<FileProgress>(EVENTS.progress, (e) => {
      if (!cancelled) setJavaProgress(e.payload);
    }).then((f) => {
      if (cancelled) f();
      else un = f;
    });
    return () => {
      cancelled = true;
      un?.();
    };
  }, []);

  const downloadJava = (component: string) => {
    if (javaBusy) return;
    setJavaBusy(`dl:${component}`);
    setJavaError(null);
    setJavaProgress(null);
    void ipc
      .javaDownload(component)
      .then(async () => {
        await reloadJava();
      })
      .catch((e) => setJavaError(t(errorKey(toIpcError(e).code))))
      .finally(() => {
        setJavaBusy(null);
        setJavaProgress(null);
      });
  };

  const removeJava = (component: string) => {
    if (javaBusy) return;
    if (!window.confirm(t("settings.javaRemoveConfirm", { component }))) return;
    setJavaBusy(`rm:${component}`);
    setJavaError(null);
    void ipc
      .javaRemove(component)
      .then(async () => {
        await reloadJava();
      })
      .catch((e) => setJavaError(t(errorKey(toIpcError(e).code))))
      .finally(() => setJavaBusy(null));
  };

  // 进页面先问一次"这个构建配了自动更新吗"，别直接去 check()：
  // 没配的时候 check() 会抛一个来自插件的错误，而那是配置状态不是故障。
  useEffect(() => {
    void getVersion().then(setAppVersion).catch(() => setAppVersion(""));
  }, []);

  useEffect(() => {
    let cancelled = false;
    void ipc
      .updateConfigured()
      .then((ok) => {
        if (!cancelled) setUpdate({ status: ok ? "current" : "unconfigured" });
      })
      .catch(() => {
        if (!cancelled) setUpdate({ status: "unconfigured" });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const checkUpdate = async () => {
    setUpdate({ status: "checking" });
    try {
      const found = await checkForUpdate();
      if (!found) {
        setUpdate({ status: "current" });
        return;
      }
      setUpdate({
        status: "available",
        version: found.version,
        notes: found.body ?? "",
      });
    } catch (e) {
      setUpdate({ status: "error", error: toIpcError(e).message });
    }
  };

  const installUpdate = async () => {
    setUpdate((prev) => ({ ...prev, status: "installing" }));
    try {
      const found = await checkForUpdate();
      if (!found) {
        setUpdate({ status: "current" });
        return;
      }
      await found.downloadAndInstall();
      // 不调 relaunch：那需要 process 插件与相应权限，而安装器本身会处理重启。
      // 与其为了省一次点击多引一个能杀进程的权限，不如把话说清楚。
      setUpdate({ status: "installed", version: found.version });
    } catch (e) {
      setUpdate({ status: "error", error: toIpcError(e).message });
    }
  };

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (!form) return;
    setError(null);
    setSaved(false);
    try {
      onConfigSaved(await ipc.setConfig(form));
      setSaved(true);
    } catch (err) {
      setError(t(errorKey(toIpcError(err).code)));
    }
  };

  if (!form) return <p className="muted">{t("errors.config")}</p>;

  const patch = (p: Partial<Config>) => setForm({ ...form, ...p });

  return (
    <form className="stack" onSubmit={(e) => void submit(e)}>
      <section className="card">
        <h2>{t("settings.servers")}</h2>
        <label className="field">
          <span>{t("settings.apiUrl")}</span>
          <input
            value={form.apiUrl}
            onChange={(e) => patch({ apiUrl: e.target.value })}
            spellCheck={false}
          />
        </label>
        <label className="field">
          <span>{t("settings.hoardUrl")}</span>
          <input
            value={form.hoardUrl}
            onChange={(e) => patch({ hoardUrl: e.target.value })}
            spellCheck={false}
          />
        </label>
      </section>

      <section className="card">
        <h2>{t("settings.appearance")}</h2>
        <label className="field">
          <span>{t("settings.language")}</span>
          <select
            value={form.language}
            onChange={(e) => patch({ language: e.target.value })}
          >
            <option value="">{t("settings.languageAuto")}</option>
            <option value="zh-CN">中文（简体）</option>
            <option value="en">English</option>
          </select>
        </label>
        <label className="field">
          <span>{t("settings.theme")}</span>
          <select
            value={form.theme}
            onChange={(e) => patch({ theme: e.target.value as Config["theme"] })}
          >
            <option value="system">{t("settings.themeSystem")}</option>
            <option value="dark">{t("settings.themeDark")}</option>
            <option value="light">{t("settings.themeLight")}</option>
          </select>
        </label>
      </section>

      <section className="card">
        <h2>{t("settings.downloads")}</h2>
        <label className="field">
          <span>{t("settings.concurrency")}</span>
          <input
            type="number"
            min={1}
            max={64}
            value={form.download.concurrency}
            onChange={(e) =>
              patch({
                download: { ...form.download, concurrency: Number(e.target.value) },
              })
            }
          />
        </label>
        <label className="field">
          <span>{t("settings.mirror")}</span>
          <input
            value={form.download.mirror}
            onChange={(e) => patch({ download: { ...form.download, mirror: e.target.value } })}
            spellCheck={false}
            placeholder="https://bmclapi2.bangbang93.com"
          />
        </label>
      </section>

      <section className="card">
        <h2>{t("settings.defaults")}</h2>
        <label className="field">
          <span>{t("settings.memoryMb")}</span>
          <input
            type="number"
            min={512}
            max={65536}
            step={512}
            value={form.defaults.memoryMb}
            onChange={(e) =>
              patch({ defaults: { ...form.defaults, memoryMb: Number(e.target.value) } })
            }
          />
        </label>
      </section>

      <div className="row">
        <button type="submit">{t("settings.save")}</button>
        {saved && <span className="ok">{t("settings.saved")}</span>}
        {error && (
          <span className="error" role="alert">
            {error}
          </span>
        )}
      </div>

      {warning && (
        <section className="card warn">
          <h2>{t("settings.startupWarning")}</h2>
          <p className="small">{warning}</p>
        </section>
      )}

      {accounts && (
        <section className="card">
          <h2>{t("settings.accounts")}</h2>
          <p className="muted small">
            {accounts.store.encrypted
              ? t("settings.accountsStoreKeyring")
              : t("settings.accountsStorePlain", { path: accounts.store.path })}
          </p>
          {accountError && (
            <p className="error small" role="alert">
              {accountError}
            </p>
          )}
          {accounts.accounts.length === 0 ? (
            <p className="muted small">{t("settings.accountsEmpty")}</p>
          ) : (
            <table className="items">
              <thead>
                <tr>
                  <th>{t("settings.accountColName")}</th>
                  <th>{t("settings.accountColKind")}</th>
                  <th>{t("settings.accountColSite")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {accounts.accounts.map((a) => (
                  <tr key={a.id} className={a.id === accounts.activeId ? undefined : "off"}>
                    <td>
                      <span title={a.playerUuid}>{a.playerName}</span>
                      {a.id === accounts.activeId && (
                        <em className="muted small"> · {t("settings.accountActive")}</em>
                      )}
                    </td>
                    <td className="muted small">{t(`settings.accountKind.${a.kind}`)}</td>
                    <td className="muted small">{a.apiUrl || "—"}</td>
                    <td>
                      <div className="row">
                        {a.id !== accounts.activeId && (
                          <button
                            type="button"
                            className="link"
                            disabled={accountBusy !== null}
                            onClick={() => void switchAccount(a.id)}
                          >
                            {accountBusy === `switch:${a.id}` ? "…" : t("settings.accountUse")}
                          </button>
                        )}
                        <button
                          type="button"
                          className="link"
                          disabled={accountBusy !== null}
                          onClick={() => void removeAccount(a)}
                        >
                          {accountBusy === `remove:${a.id}` ? "…" : t("settings.accountRemove")}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div className="row sub">
            <label className="field inline grow">
              <span>{t("settings.accountOfflineName")}</span>
              <input
                value={offlineName}
                maxLength={16}
                placeholder="Steve"
                onChange={(e) => setOfflineName(e.target.value)}
              />
            </label>
            <button
              type="button"
              disabled={accountBusy !== null || !offlineName.trim()}
              onClick={() => void addOffline()}
            >
              {accountBusy === "add-offline" ? "…" : t("settings.accountAddOffline")}
            </button>
          </div>
          <p className="muted small">{t("settings.accountOfflineHint")}</p>
        </section>
      )}

      {siteAccount && (
        <section className="card">
          <h2>{t("settings.credentials")}</h2>
          <p className="muted small">
            {t("settings.credentialsHint", { name: siteAccount.playerName })}
          </p>
          {credError && (
            <p className="error small" role="alert">
              {credError}
            </p>
          )}
          {credentials === null ? (
            <p className="muted small">{t("settings.credentialsLoading")}</p>
          ) : credentials.length === 0 ? (
            // 这一句是这块面板最要紧的内容：ADR-0012 之后，没有独立密码的玩家
            // 从任何启动器都登不进来，而登录失败只会说"名字或密码不对"
            <p className="small">{t("settings.credentialsEmpty")}</p>
          ) : (
            <table className="items">
              <thead>
                <tr>
                  <th>{t("settings.credentialsColLabel")}</th>
                  <th>{t("settings.credentialsColCreated")}</th>
                  <th>{t("settings.credentialsColUsed")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {credentials.map((c) => (
                  <tr key={c.id}>
                    <td>{c.name}</td>
                    <td className="muted small">{c.createdAt}</td>
                    <td className="muted small">
                      {c.lastUsedAt ?? t("settings.credentialsNeverUsed")}
                    </td>
                    <td>
                      <button
                        type="button"
                        className="link"
                        disabled={credBusy !== null}
                        onClick={() => void revokeCredential(c)}
                      >
                        {credBusy === `revoke:${c.id}` ? "…" : t("settings.credentialsRevoke")}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div className="row sub">
            <label className="field inline grow">
              <span>{t("settings.credentialsLabel")}</span>
              <input
                value={credLabel}
                maxLength={64}
                placeholder={t("settings.credentialsLabelPlaceholder")}
                onChange={(e) => setCredLabel(e.target.value)}
              />
            </label>
            <label className="field inline grow">
              <span>{t("settings.credentialsPassword")}</span>
              <input
                type="password"
                value={credPassword}
                minLength={8}
                autoComplete="new-password"
                onChange={(e) => setCredPassword(e.target.value)}
                onKeyDown={(e) => {
                  // 外层是设置页那个大 form：不拦的话回车会去提交设置
                  if (e.key !== "Enter") return;
                  e.preventDefault();
                  void createCredential();
                }}
              />
            </label>
            <button
              type="button"
              disabled={credBusy !== null || !credLabel.trim() || credPassword.length < 8}
              onClick={() => void createCredential()}
            >
              {credBusy === "create" ? "…" : t("settings.credentialsCreate")}
            </button>
          </div>
          <p className="muted small">{t("settings.credentialsPasswordHint")}</p>
        </section>
      )}

      <section className="card">
        <div className="row between">
          <h2>{t("settings.java")}</h2>
          <button type="button" className="link" onClick={() => void reloadJava()}>
            {t("logs.refresh")}
          </button>
        </div>
        <p className="muted small">{t("settings.javaHint")}</p>
        {javaError && (
          <p className="error small" role="alert">
            {javaError}
          </p>
        )}
        {javaProgress && javaBusy && (
          <p className="muted small" role="status">
            {t("settings.javaDownloading", {
              done: (javaProgress.doneBytes / 1048576).toFixed(1),
              total: (javaProgress.totalBytes / 1048576).toFixed(1),
            })}
          </p>
        )}

        {java === null ? (
          <p className="muted small">{t("logs.diagnosisLoading")}</p>
        ) : (
          <>
            <h3>{t("settings.javaDetected")}</h3>
            {java.runtimes.length === 0 ? (
              <p className="muted small">{t("settings.javaNoneDetected")}</p>
            ) : (
              <table className="items">
                <thead>
                  <tr>
                    <th>{t("settings.javaColVersion")}</th>
                    <th>{t("settings.javaColPath")}</th>
                  </tr>
                </thead>
                <tbody>
                  {java.runtimes.map((r) => (
                    <tr key={r.path}>
                      <td>
                        Java {r.version}
                        {r.is64bit ? " (64-bit)" : " (32-bit)"}
                      </td>
                      <td className="muted small">{r.path}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}

            <h3>{t("settings.javaOfficial")}</h3>
            {java.available.length === 0 ? (
              // 两种"一个都没有"必须分开说：查不到（可重试）与这个平台根本没有
              // （只能手动指定路径）。合成一句会让后者看起来像个临时故障。
              <p className="muted small">
                {java.availableError === null
                  ? t("settings.javaNoOfficialForPlatform")
                  : t("settings.javaIndexUnavailable", { error: java.availableError })}
              </p>
            ) : (
              <>
                <p className="muted small">{t("settings.javaOfficialHint")}</p>
                <table className="items">
                  <thead>
                    <tr>
                      <th>{t("settings.javaColComponent")}</th>
                      <th>{t("settings.javaColOfferVersion")}</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {java.available.map((o) => {
                      const have = java.downloaded.includes(o.component);
                      return (
                        <tr key={o.component}>
                          <td className="small">{o.component}</td>
                          <td className="muted small">{o.versionName || "—"}</td>
                          <td>
                            <div className="row">
                              <button
                                type="button"
                                className="link"
                                disabled={javaBusy !== null}
                                onClick={() => downloadJava(o.component)}
                              >
                                {javaBusy === `dl:${o.component}`
                                  ? "…"
                                  : have
                                    ? t("settings.javaReinstall")
                                    : t("settings.javaDownload")}
                              </button>
                              {have && (
                                <button
                                  type="button"
                                  className="link"
                                  disabled={javaBusy !== null}
                                  onClick={() => removeJava(o.component)}
                                >
                                  {javaBusy === `rm:${o.component}` ? "…" : t("settings.javaRemove")}
                                </button>
                              )}
                            </div>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </>
            )}
          </>
        )}
      </section>

      <section className="card">
        <div className="row between">
          <h2>{t("settings.update")}</h2>
          {update.status !== "unconfigured" && (
            <button
              type="button"
              className="link"
              disabled={update.status === "checking" || update.status === "installing"}
              onClick={() => void checkUpdate()}
            >
              {update.status === "checking" ? "…" : t("settings.updateCheck")}
            </button>
          )}
        </div>
        {update.status === "unconfigured" && (
          // 本地构建（或没有注入签名公钥的构建）就是这个状态。要说清楚它是正常的，
          // 否则用户会以为自动更新坏了。
          <p className="muted small">{t("settings.updateUnconfigured")}</p>
        )}
        {update.status === "current" && <p className="muted small">{t("settings.updateCurrent", { version: appVersion })}</p>}
        {update.status === "checking" && <p className="muted small">{t("settings.updateChecking")}</p>}
        {update.status === "installing" && <p className="muted small">{t("settings.updateInstalling")}</p>}
        {update.status === "installed" && (
          <p className="ok small" role="status">
            {t("settings.updateInstalled", { version: update.version ?? "" })}
          </p>
        )}
        {update.status === "error" && (
          <p className="error small" role="alert">
            {t("settings.updateError", { error: update.error ?? "" })}
          </p>
        )}
        {update.status === "available" && (
          <div className="stack">
            <p className="small">{t("settings.updateAvailable", { version: update.version ?? "" })}</p>
            {update.notes && <pre className="evidence">{update.notes}</pre>}
            <button type="button" onClick={() => void installUpdate()}>
              {t("settings.updateInstall")}
            </button>
          </div>
        )}
      </section>

      {paths && (
        <section className="card">
          <div className="row between">
            <h2>{t("settings.paths")}</h2>
            <button
              type="button"
              className="link"
              onClick={() => void revealItemInDir(paths.configFile)}
            >
              {t("settings.openDataDir")}
            </button>
          </div>
          <p className="muted small">{t("settings.pathsHint")}</p>
          <dl className="paths">
            {PATH_FIELDS.map((f) => (
              <div key={f.key}>
                <dt>{f.label}</dt>
                <dd>{paths[f.key]}</dd>
              </div>
            ))}
          </dl>
        </section>
      )}
    </form>
  );
}
