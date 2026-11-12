import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { errorKey, useI18n } from "../i18n";
import { clearGameLogs, useGameLogs } from "../App";
import {
  ipc,
  toIpcError,
  type Diagnosis,
  type Finding,
  type Instance,
  type LogArchive,
} from "../lib/ipc";
import { LEVELS, levelOf, parseAnsi, stripAnsi, type LogLevel } from "../lib/log-format";

// 实时输出的订阅与环形缓冲在 App.tsx（应用启动就开始记录，早于本页挂载）；
// 这里渲染快照，并通过 useGameLogs 保持实时追加。

export interface LogsViewProps {
  onOpenContent: (slug: string) => void;
  onOpenInstances: () => void;
}

export function LogsView({ onOpenContent, onOpenInstances }: LogsViewProps) {
  const { t } = useI18n();
  const { lines, exit } = useGameLogs();
  const endRef = useRef<HTMLDivElement>(null);

  const [instances, setInstances] = useState<Instance[]>([]);
  const [slug, setSlug] = useState("");
  const [archives, setArchives] = useState<LogArchive[] | null>(null);
  /** null = 看实时输出；否则是归档里的某一份 */
  const [open, setOpen] = useState<string | null>(null);
  const [doc, setDoc] = useState<string | null>(null);
  const [truncated, setTruncated] = useState(false);
  const [diag, setDiag] = useState<Diagnosis | null>(null);
  const [oplog, setOplog] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [hidden, setHidden] = useState<ReadonlySet<LogLevel>>(new Set());
  const [follow, setFollow] = useState(true);

  const fail = useCallback(
    (e: unknown) => setError(t(errorKey(toIpcError(e).code))),
    [t],
  );

  useEffect(() => {
    void ipc
      .instanceList()
      .then((list) => {
        setInstances(list);
        // 默认选中最近启动过的那个：来这一页的人多半是想看刚跑过的那次
        const recent = [...list].sort(
          (a, b) => (b.lastLaunchedAt ?? 0) - (a.lastLaunchedAt ?? 0),
        )[0];
        setSlug((cur) => cur || recent?.slug || list[0]?.slug || "");
      })
      .catch(fail);
    void ipc
      .operationLog()
      .then(setOplog)
      .catch(() => setOplog(null));
  }, [fail]);

  const reload = useCallback(async () => {
    if (!slug) {
      setArchives([]);
      setDiag(null);
      return;
    }
    try {
      setArchives(await ipc.instanceLogs(slug));
      setDiag(await ipc.instanceDiagnose(slug));
      setError(null);
    } catch (e) {
      fail(e);
    }
  }, [slug, fail]);

  useEffect(() => {
    void reload();
  }, [reload]);

  // 切换要看的归档；null 表示回到实时输出
  useEffect(() => {
    let cancelled = false;
    if (!slug || !open) {
      setDoc(null);
      setTruncated(false);
      return;
    }
    void ipc
      .instanceLogRead(slug, open)
      .then((got) => {
        if (cancelled) return;
        setDoc(got.text);
        setTruncated(got.truncated);
      })
      .catch((e) => {
        if (!cancelled) {
          setDoc(null);
          fail(e);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [slug, open, fail]);

  useEffect(() => {
    if (follow && !open) endRef.current?.scrollIntoView({ block: "end" });
  }, [lines, follow, open]);

  const toggleLevel = (lvl: LogLevel) =>
    setHidden((prev) => {
      const next = new Set(prev);
      if (next.has(lvl)) next.delete(lvl);
      else next.add(lvl);
      return next;
    });

  /** 实时输出与归档共用一套渲染：两边都是纯文本行（见 Rust 侧 logconfig 的说明，
   *  控制台布局被换成纯文本就是为了这里不必写两个查看器）。 */
  const visible = useMemo(() => {
    const source: string[] = open ? (doc ?? "").split("\n") : lines.map((l) => l.line);
    const q = query.trim().toLowerCase();
    const out: { text: string; level: LogLevel; stderr: boolean }[] = [];
    for (let i = 0; i < source.length; i += 1) {
      const text = source[i];
      const level = levelOf(text);
      if (hidden.has(level)) continue;
      if (q && !stripAnsi(text).toLowerCase().includes(q)) continue;
      const stderr = !open && lines[i]?.stream === "stderr";
      out.push({ text, level, stderr });
    }
    return out;
  }, [open, doc, lines, query, hidden]);

  const copyAll = async () => {
    const text = open ? (doc ?? "") : lines.map((l) => l.line).join("\n");
    try {
      await navigator.clipboard.writeText(text);
    } catch (e) {
      fail(e);
    }
  };

  const openLogsDir = async () => {
    if (!slug) return;
    try {
      await revealItemInDir(await ipc.instanceLogsDir(slug));
    } catch (e) {
      fail(e);
    }
  };

  const openOplog = async () => {
    try {
      await revealItemInDir(await ipc.operationLogPath());
    } catch (e) {
      fail(e);
    }
  };

  const runAction = (f: Finding) => {
    if (!f.action) return;
    switch (f.action.kind) {
      case "openContent":
        onOpenContent(slug);
        break;
      case "openLogs":
        setOpen(null);
        break;
      case "installMod":
        // 内容页会从 Hoard 搜这个 id；这里只负责把人送过去
        onOpenContent(slug);
        break;
      case "installJava":
      case "adjustMemory":
        onOpenInstances();
        break;
    }
  };

  const shown = instances.find((i) => i.slug === slug);

  return (
    <div className="stack">
      {error && (
        <p className="error small" role="alert">
          {error}
        </p>
      )}

      {slug && (
        <section className="card">
          <div className="row between">
            <h2>{t("logs.diagnosis")}</h2>
            <div className="row">
              <select
                aria-label={t("logs.instance")}
                value={slug}
                onChange={(e) => {
                  setSlug(e.target.value);
                  setOpen(null);
                }}
              >
                {instances.map((i) => (
                  <option key={i.slug} value={i.slug}>
                    {i.name}
                  </option>
                ))}
              </select>
              <button type="button" className="link" onClick={() => void reload()}>
                {t("logs.refresh")}
              </button>
            </div>
          </div>
          {diag === null ? (
            <p className="muted small">{t("logs.diagnosisLoading")}</p>
          ) : diag.source === null ? (
            // "没有日志"与"没查出问题"是两件事：前者说明还没跑过，
            // 后者说明跑了但规则库没认出原因
            <p className="muted small">{t("logs.diagnosisNoLogs", { instance: shown?.name ?? slug })}</p>
          ) : diag.findings.length === 0 ? (
            <p className="muted small">
              {t("logs.diagnosisClean", { source: diag.source })}
              {diag.partial ? ` ${t("logs.diagnosisPartial")}` : ""}
            </p>
          ) : (
            <div className="stack">
              <p className="muted small">{t("logs.diagnosisSource", { source: diag.source })}</p>
              {diag.findings.map((f) => (
                <div key={f.code} className="finding">
                  <div className="row between">
                    <strong className={f.severity === "fatal" ? "error" : undefined}>
                      {t(`diagnose.${f.code}`, {
                        major: f.action?.kind === "installJava" ? String(f.action.major) : "",
                        id: f.action?.kind === "installMod" ? f.action.id : "",
                      })}
                    </strong>
                    {f.action && (
                      <button type="button" className="link" onClick={() => runAction(f)}>
                        {t(`diagnose.action.${f.action.kind}`)}
                      </button>
                    )}
                  </div>
                  <span className={`badge ${f.severity === "fatal" ? "" : "off"}`}>
                    {t(`logs.severity.${f.severity}`)}
                  </span>
                  {/* 证据是日志里的原文行：用户要能核对我们的判断，
                      规则错了也要能被看出来——只给一段我们自己的转述就做不到这两点 */}
                  <pre className="evidence">{f.evidence}</pre>
                </div>
              ))}
              {diag.partial && <p className="muted small">{t("logs.diagnosisPartial")}</p>}
            </div>
          )}
        </section>
      )}

      <section className="card">
        <div className="row between">
          <h2>{t("logs.title")}</h2>
          <div className="row">
            <button type="button" className="link" onClick={() => void copyAll()}>
              {t("logs.copy")}
            </button>
            {slug && (
              <button type="button" className="link" onClick={() => void openLogsDir()}>
                {t("logs.openDir")}
              </button>
            )}
            {!open && (
              <button type="button" className="link" onClick={clearGameLogs}>
                {t("logs.clear")}
              </button>
            )}
          </div>
        </div>
        <p className="muted small">{t("logs.hint")}</p>

        {exit && !open && (
          <p
            className={exit.code === 0 || exit.code === null ? "ok small" : "error small"}
            role="status"
          >
            {exit.code === null
              ? t("logs.stopped", { instance: exit.instance })
              : t("logs.exited", { instance: exit.instance, code: exit.code })}
          </p>
        )}
        {truncated && (
          <p className="muted small" role="status">
            {t("logs.truncated")}
          </p>
        )}

        <div className="row sub">
          <input
            type="search"
            value={query}
            placeholder={t("logs.search")}
            aria-label={t("logs.search")}
            onChange={(e) => setQuery(e.target.value)}
          />
          <label className="field inline">
            <input
              type="checkbox"
              checked={follow && !open}
              disabled={!!open}
              onChange={(e) => setFollow(e.target.checked)}
            />
            <span>{t("logs.follow")}</span>
          </label>
        </div>
        <div className="chips">
          {LEVELS.map((lvl) => (
            <button
              key={lvl}
              type="button"
              className={hidden.has(lvl) ? "chip off" : "chip"}
              aria-pressed={!hidden.has(lvl)}
              onClick={() => toggleLevel(lvl)}
            >
              {t(`logs.level.${lvl}`)}
            </button>
          ))}
        </div>

        <div className="log">
          {visible.length === 0 ? (
            <p className="muted small">
              {open ? t("logs.archiveEmpty") : t("logs.empty")}
            </p>
          ) : (
            visible.map((l, i) => (
              <div key={i} className={`log-line lvl-${l.level}${l.stderr ? " err" : ""}`}>
                {parseAnsi(l.text).map((s, j) => (
                  <span
                    key={j}
                    className={s.fg ? `ansi-${s.fg}${s.bold ? " b" : ""}` : s.bold ? "b" : undefined}
                  >
                    {s.text}
                  </span>
                ))}
              </div>
            ))
          )}
          <div ref={endRef} />
        </div>
      </section>

      {slug && (
        <section className="card">
          <h2>{t("logs.archives")}</h2>
          <p className="muted small">{t("logs.archivesHint")}</p>
          {archives === null ? (
            <p className="muted small">{t("logs.diagnosisLoading")}</p>
          ) : archives.length === 0 ? (
            <p className="muted small">{t("logs.archivesEmpty")}</p>
          ) : (
            <table className="items">
              <thead>
                <tr>
                  <th>{t("logs.archiveColName")}</th>
                  <th>{t("logs.archiveColKind")}</th>
                  <th>{t("logs.archiveColSize")}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {archives.map((a) => (
                  <tr key={a.name} className={open === a.name ? undefined : "off"}>
                    <td>
                      <span title={a.name}>{a.name.split("/").pop()}</span>
                    </td>
                    <td className="muted small">{t(`logs.kind.${a.kind}`)}</td>
                    <td className="muted small">
                      {(a.size / 1024).toFixed(0)} KB{a.compressed ? " (gz)" : ""}
                    </td>
                    <td>
                      <button
                        type="button"
                        className="link"
                        onClick={() => {
                          setOpen(open === a.name ? null : a.name);
                          setFollow(false);
                        }}
                      >
                        {open === a.name ? t("logs.archiveClose") : t("logs.archiveOpen")}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>
      )}

      <section className="card">
        <div className="row between">
          <h2>{t("logs.oplog")}</h2>
          <button type="button" className="link" onClick={() => void openOplog()}>
            {t("logs.openDir")}
          </button>
        </div>
        <p className="muted small">{t("logs.oplogHint")}</p>
        <pre className="log oplog">{oplog && oplog.trim() ? oplog : t("logs.oplogEmpty")}</pre>
      </section>
    </div>
  );
}
