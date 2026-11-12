import { useEffect, useState, type FormEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useConfig } from "../App";
import { useI18n, errorKey } from "../i18n";
import { ipc, EVENTS, toIpcError, type Instance, type AccountView, type FileProgress, type ServerCompatibility } from "../lib/ipc";
import { SkinPreview } from "../components/skin-preview";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Progress } from "../components/ui/progress";
import { getMinecraftSoundVolume, setMinecraftSoundVolume } from "../components/ui/minecraft-sound";
import { startMessages } from "./start-messages";
import "./start.css";

interface Props {
  account: AccountView | null;
  checking: boolean;
  onSignIn: (playerName: string, password: string) => Promise<void>;
}

export function StartView({ account, checking, onSignIn }: Props) {
  const { t, locale } = useI18n();
  const text = startMessages[locale];
  const { config } = useConfig();
  const [instances, setInstances] = useState<Instance[] | null>(null);
  const [selected, setSelected] = useState("");
  const [running, setRunning] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<FileProgress | null>(null);
  const [skin, setSkin] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [password, setPassword] = useState("");
  const [signingIn, setSigningIn] = useState(false);
  const [sound, setSound] = useState(() => getMinecraftSoundVolume() > 0);
  const [compatibility, setCompatibility] = useState<{ slug: string; site: string; status: ServerCompatibility | null } | null>(null);

  useEffect(() => {
    let active = true;
    void ipc.instanceList().then((list) => { if (active) { setInstances(list); setSelected(list[0]?.slug ?? ""); } }).catch((e: unknown) => { if (active) setError(t(errorKey(toIpcError(e).code))); });
    const update = () => { void ipc.gameRunning().then((slug) => { if (active) setRunning(slug); }).catch(() => {}); };
    update(); const timer = setInterval(update, 2000);
    return () => { active = false; clearInterval(timer); };
  }, [t]);
  useEffect(() => {
    const subscription = listen<FileProgress>(EVENTS.progress, (event) => setProgress(event.payload));
    return () => { void subscription.then((unlisten) => unlisten()); };
  }, []);
  useEffect(() => {
    let active = true;
    if (!account || account.kind !== "yggdrasil" || account.apiUrl !== config?.apiUrl) return;
    void ipc.skinDataUrl(account.playerName).then((url) => { if (active) setSkin(url); }).catch(() => { if (active) setSkin(null); });
    return () => { active = false; };
  }, [account, config?.apiUrl]);

  const current = instances?.find((i) => i.slug === selected);
  useEffect(() => {
    if (!selected || !config?.apiUrl) return;
    const site = config.apiUrl;
    let active = true;
    const load = () => {
      void ipc.instanceServerStatus(selected)
        .then((status) => { if (active) setCompatibility({ slug: selected, site, status }); })
        .catch(() => { if (active) setCompatibility({ slug: selected, site, status: null }); });
    };
    load();
    const timer = setInterval(load, 30_000);
    return () => { active = false; clearInterval(timer); };
  }, [selected, config?.apiUrl]);
  const serverHint = compatibility?.slug === selected && compatibility.site === config?.apiUrl ? compatibility : null;
  const launch = async () => {
    if (!current || busy || !account || running) return;
    setBusy(true); setError(""); setProgress(null);
    try { await ipc.instanceLaunch(current.slug); setRunning(await ipc.gameRunning()); }
    catch (e) { setError(t(errorKey(toIpcError(e).code))); }
    finally { setBusy(false); setProgress(null); }
  };
  const stop = async () => {
    if (busy) return; setBusy(true); setError("");
    try { await ipc.instanceStop(); setRunning(await ipc.gameRunning()); }
    catch (e) { setError(t(errorKey(toIpcError(e).code))); }
    finally { setBusy(false); }
  };
  const login = async (e: FormEvent) => {
    e.preventDefault(); if (signingIn || !name.trim() || !password) return;
    setSigningIn(true); const secret = password; setPassword("");
    try { await onSignIn(name.trim(), secret); } finally { setSigningIn(false); }
  };

  return <div className="start-page">
    <header className="start-heading"><div><h1>{text.title}</h1><p>{text.intro}</p></div><label className="start-sound"><input type="checkbox" checked={sound} onChange={(e) => { const enabled = e.target.checked; setSound(enabled); setMinecraftSoundVolume(enabled ? 1 : 0); }} />{text.sound}</label></header>
    <div className="start-columns">
      <section className="start-play" aria-labelledby="start-instance-title">
        <div className="start-world" aria-hidden="true"><span className="start-block start-block-back" /><span className="start-block start-block-front" /><span className="start-block start-block-top" /></div>
        <div className="start-play-content"><p className="start-kicker">Minecraft: Java Edition</p><h2 id="start-instance-title">{current?.name ?? text.selectInstance}</h2><p className="start-version">{current?.versionId ?? "—"}</p>
          <label className="start-field"><span>{text.instance}</span><select className="start-select" value={selected} disabled={!instances?.length || busy} onChange={(e) => setSelected(e.target.value)}><option value="" disabled>{text.selectInstance}</option>{instances?.map((i) => <option key={i.slug} value={i.slug}>{i.name}</option>)}</select></label>
          {instances === null ? <p role="status">{text.loading}</p> : instances.length === 0 ? <p className="start-hint">{t("home.noInstances")}</p> : null}
          {current?.server && <div className="start-hint" role="status">
            <p>{text.clientVersion}: {current.loader?.gameVersion ?? current.versionId} · {current.server.host}:{current.server.port ?? 25565}</p>
            {!serverHint ? <p>{config?.apiUrl ? text.checkingCompatibility : text.compatibilityUnknown}</p> : serverHint.status ? <>
              <p>{serverHint.status.reachable ? text.serverVersion : text.serverUnreachable}: {serverHint.status.version ?? "—"}</p>
              {!!serverHint.status.protocolVersions.length && <p>{text.allowedProtocols}: {serverHint.status.protocolVersions.join(", ")}</p>}
              <p>{text.compatibilityHint}</p>
            </> : <p>{text.compatibilityUnknown}</p>}
          </div>}
          <div className="start-main-action"><Button style={{ width: "100%", maxWidth: "400px" }} disabled={!current || !account || !!running || busy || checking} onClick={() => void launch()}>{busy ? text.starting : running ? text.running : text.launch}</Button></div>
          {running && <div className="start-running"><p role="status">{text.running}: {instances?.find((i) => i.slug === running)?.name ?? running}</p><Button scale={1} style={{ width: "180px" }} disabled={busy} onClick={() => void stop()}>{text.stop}</Button></div>}
          {!account && !checking && <p className="start-hint">{text.loginFirst}</p>}
          {busy && progress && <div className="start-download" role="status"><p>{text.downloading}</p>{progress.totalBytes > 0 ? <Progress value={Math.min(progress.doneBytes, progress.totalBytes)} max={progress.totalBytes} color="purple" style={{ width: "100%", maxWidth: "364px" }} aria-label={text.downloading} /> : <p>{text.unknownSize}</p>}<p>{(progress.doneBytes / 1048576).toFixed(1)} MiB{progress.totalBytes > 0 ? ` / ${(progress.totalBytes / 1048576).toFixed(1)} MiB` : ""}</p></div>}
          {error && <p className="start-error" role="alert">{error}</p>}
        </div>
      </section>
      <aside className="start-identity"><h2>{account ? account.playerName : t("account.signIn")}</h2>
        {account ? <><p className="start-hint">{account.kind === "offline" ? text.offline : text.siteAccount}</p><div className="start-skin"><SkinPreview key={`${account.apiUrl}/${account.playerName}`} skinUrl={account.kind === "yggdrasil" && account.apiUrl === config?.apiUrl ? skin : null} /></div></> : checking ? <p role="status">{t("account.checking")}</p> : <form onSubmit={(e) => void login(e)} className="start-login"><label className="start-field"><span>{t("account.playerName")}</span><Input autoComplete="username" required value={name} onChange={(e) => setName(e.target.value)} style={{ width: "100%" }} /></label><label className="start-field"><span>{text.password}</span><Input type="password" autoComplete="current-password" required value={password} onChange={(e) => setPassword(e.target.value)} style={{ width: "100%" }} /></label><p className="start-hint">{text.passwordHint}</p><Button disabled={signingIn} type="submit" style={{ width: "100%" }}>{signingIn ? text.loading : t("account.signIn")}</Button></form>}
        {config?.apiUrl && <button type="button" className="start-site-link" onClick={() => { void openUrl(`${config.apiUrl}/servers`).catch(() => setError(text.openFailed)); }}>{text.site}</button>}
      </aside>
    </div>
    <p className="start-footer">{text.manageHint}</p>
  </div>;
}
