import { useEffect, useMemo, useState, type FormEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useI18n } from "../i18n";
import { useConfig } from "../App";
import { ipc, type AccountEntry, type AccountView, type Instance } from "../lib/ipc";
import { SkinPreview } from "../components/skin-preview";

interface Props {
  account: AccountView | null;
  checking: boolean;
  onSignIn: (playerName: string, password: string) => Promise<void>;
}

export function HomeView({ account, checking, onSignIn }: Props) {
  const { t } = useI18n();
  const { config } = useConfig();
  const [playerName, setPlayerName] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [skinUrl, setSkinUrl] = useState<string | null>(null);
  const [instances, setInstances] = useState<Instance[] | null>(null);
  const [accounts, setAccounts] = useState<AccountEntry[]>([]);
  /** 选中的预览对象；null = 跟着当前账号走（切换账号时不必重新选） */
  const [previewId, setPreviewId] = useState<string | null>(null);

  // 首页只做概览：有实例就列名字与版本，管理操作都在「实例」页
  useEffect(() => {
    let cancelled = false;
    void ipc
      .instanceList()
      .then((list) => {
        if (!cancelled) setInstances(list);
      })
      .catch(() => {
        if (!cancelled) setInstances([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 可选的预览对象 = **当前站点上**的已存账号。
  //
  // 皮肤是按玩家名从 `config.apiUrl` 取的（`mirror_skin_data_url` 用的是设置里的站点，
  // 不是账号自己的），所以别的站点上的账号会取到那个站点上同名玩家的皮肤——一张看着
  // 完全正常、其实属于另一个人的图。离线账号同理（apiUrl 为空，站点上没有这个身份）。
  // 过滤掉比给 IPC 加一个"用哪个站点"的参数更省事，也不会显示错的图。
  const previewChoices = useMemo(
    () => accounts.filter((a) => a.kind === "yggdrasil" && a.apiUrl === config?.apiUrl),
    [accounts, config?.apiUrl],
  );
  const previewName =
    previewChoices.find((a) => a.id === previewId)?.playerName ?? account?.playerName ?? null;
  /** 有多个可选对象、且当前账号本身就在候选里（于是"默认预览谁"是明确的） */
  const canChoosePreview =
    previewChoices.length > 1 &&
    previewChoices.some((a) => a.playerName === account?.playerName);

  useEffect(() => {
    let cancelled = false;
    void ipc
      .accountsList()
      .then((list) => {
        if (!cancelled) setAccounts(list.accounts);
      })
      .catch(() => {
        if (!cancelled) setAccounts([]);
      });
    return () => {
      cancelled = true;
    };
    // 依赖 account：登录/登出/切换账号都会换掉它，那时账号列表也可能变了
  }, [account]);

  // 皮肤纹理由 Rust 取回并转成 data: URL：webview 不直连后端，CSP 的 img-src 也就不用放行外站
  useEffect(() => {
    let cancelled = false;
    if (!previewName) {
      setSkinUrl(null);
      return;
    }
    void ipc
      .skinDataUrl(previewName)
      .then((url) => {
        if (!cancelled) setSkinUrl(url);
      })
      .catch(() => {
        if (!cancelled) setSkinUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [previewName]);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (busy || !playerName.trim() || !password) return;
    setBusy(true);
    try {
      await onSignIn(playerName.trim(), password);
      setPassword("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid-2">
      <section className="card">
        <h2>{t("account.signIn")}</h2>
        {checking ? (
          <p className="muted">{t("account.checking")}</p>
        ) : account ? (
          <div className="stack">
            <p className="ok">{t("account.signedInAs", { player: account.playerName })}</p>
            <p className="muted small">
              {t("account.since", { time: new Date(account.signedInAt * 1000).toLocaleString() })}
            </p>
            <p className="muted small">{account.apiUrl}</p>
          </div>
        ) : (
          <form className="stack" onSubmit={(e) => void submit(e)}>
            <label className="field">
              <span>{t("account.playerName")}</span>
              <input
                value={playerName}
                onChange={(e) => setPlayerName(e.target.value)}
                autoComplete="username"
                placeholder="Steve"
              />
            </label>
            <label className="field">
              <span>{t("account.launcherPassword")}</span>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
              />
            </label>
            <p className="muted small">{t("account.signInHint")}</p>
            <button type="submit" disabled={busy || !playerName.trim() || !password}>
              {busy ? "…" : t("account.signIn")}
            </button>
            {config && (
              <button
                type="button"
                className="link"
                onClick={() => void openUrl(`${config.apiUrl}/profile`)}
              >
                {t("account.openSite")}
              </button>
            )}
          </form>
        )}
      </section>

      <section className="card center">
        <h2>{t("home.skinPreview")}</h2>
        {/* 只有一个可选对象时不渲染选择器：一个只有一项的下拉框是纯噪音。
            当前账号不在候选里时也不渲染（离线账号、或换了站点）——那时"默认预览谁"
            本身就是含糊的，宁可只显示预览，也不要给一个选不中的下拉框。 */}
        {canChoosePreview && (
          <label className="field inline">
            <span>{t("home.skinPreviewPlayer")}</span>
            <select
              value={previewChoices.find((a) => a.playerName === previewName)?.id ?? ""}
              onChange={(e) => setPreviewId(e.target.value)}
            >
              {previewChoices.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.playerName}
                </option>
              ))}
            </select>
          </label>
        )}
        <SkinPreview skinUrl={skinUrl} />
      </section>

      <section className="card span-2">
        <h2>{t("home.instances")}</h2>
        {instances && instances.length > 0 ? (
          <div className="stack">
            {instances.map((inst) => (
              <div key={inst.slug} className="row between">
                <strong>{inst.name}</strong>
                <span className="muted small">{inst.versionId}</span>
              </div>
            ))}
          </div>
        ) : (
          <p className="muted">{t("home.noInstances")}</p>
        )}
      </section>
    </div>
  );
}
