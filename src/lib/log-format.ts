/** 游戏日志的行解析：级别识别与 ANSI 处理。
 *
 *  级别识别**不假定一种行格式**，因为实测至少有三种（都在 `src-tauri/tests/fixtures/crash/`
 *  里，是真跑出来的日志，不是照文档写的）：
 *  - 原版与 Fabric：`[20:22:38] [main/INFO]: Loading Minecraft 1.21.4 …`
 *  - NeoForge/Forge：`[20:06:12.631] [main/INFO] [Launcher/MODLAUNCHER]: ModLauncher running…`
 *    （带毫秒，多一个 logger 段；NeoForge 从第一行起就用自己的 log4j 配置，
 *    无视我们传的那份，所以这个形状改不掉）
 *  - log4j 自己的状态行：`2026-10-30T12:17:12.617Z main WARN Advanced terminal features…`
 *
 *  还有大量**没有任何级别前缀**的行：Java 堆栈的 `\tat …`、mod 自己的 println。
 *  这类归到 `other`，而不是猜一个级别——猜错的后果是过滤器把关键行藏起来。
 */

export type LogLevel = "fatal" | "error" | "warn" | "info" | "debug" | "trace" | "other";

export const LEVELS: LogLevel[] = ["fatal", "error", "warn", "info", "debug", "trace", "other"];

/** 认出一行的级别；认不出来返回 `other`。 */
export function levelOf(line: string): LogLevel {
  const plain = stripAnsi(line);
  // `[thread/LEVEL]` 与 `[thread/LEVEL] [logger]`：log4j 的 `%t/%level`
  const bracketed = plain.match(/\[[^\]]*\/(FATAL|ERROR|WARN|INFO|DEBUG|TRACE)\]/);
  if (bracketed) return mapLevel(bracketed[1]);
  // log4j 内部状态行：`<时间戳> <thread> LEVEL <消息>`
  const status = plain.match(/^\S+\s+\S+\s+(FATAL|ERROR|WARN|INFO|DEBUG|TRACE)\s/);
  if (status) return mapLevel(status[1]);
  // 老版本（1.12.2 及更早）与一些 mod：`[HH:mm:ss] [main/ERROR]` 之外还有
  // `[HH:mm:ss] [main] [net.x/ERROR]:` 这种把级别放在 logger 段里的
  const trailing = plain.match(/\[(FATAL|ERROR|WARN|INFO|DEBUG|TRACE)\]/);
  if (trailing) return mapLevel(trailing[1]);
  return "other";
}

function mapLevel(raw: string): LogLevel {
  switch (raw) {
    case "FATAL":
      return "fatal";
    case "ERROR":
      return "error";
    case "WARN":
      return "warn";
    case "INFO":
      return "info";
    case "DEBUG":
      return "debug";
    case "TRACE":
      return "trace";
    default:
      return "other";
  }
}

// eslint-disable-next-line no-control-regex
const ANSI = /\x1b\[[0-9;]*m/g;

/** 去掉 ANSI 转义。搜索与级别识别都要先在纯文本上做，
 *  否则一个带颜色的 `ERROR` 会因为中间夹着转义而匹配不到。 */
export function stripAnsi(text: string): string {
  return text.replace(ANSI, "");
}

/** 一段带样式的文本。`fg` 是 CSS 类名后缀，不是颜色值——
 *  颜色交给样式表，跟着主题走，不在这里写死。 */
export interface AnsiSpan {
  text: string;
  fg: string | null;
  bold: boolean;
}

const SGR_FG: Record<number, string> = {
  30: "black",
  31: "red",
  32: "green",
  33: "yellow",
  34: "blue",
  35: "magenta",
  36: "cyan",
  37: "white",
  90: "bright-black",
  91: "bright-red",
  92: "bright-green",
  93: "bright-yellow",
  94: "bright-blue",
  95: "bright-magenta",
  96: "bright-cyan",
  97: "bright-white",
};

/** 把一行拆成带样式的片段。只认 SGR（`…m`）里的前景色与粗体：
 *  那是 mod 实际会用的全部；光标移动、清屏那些在**日志文件**里没有意义
 *  （日志是追加的文本，不是一个终端），直接丢掉。
 *
 *  没有任何转义时返回单个片段——这是绝大多数行，不该为此付出解析代价。 */
export function parseAnsi(line: string): AnsiSpan[] {
  if (!line.includes("\x1b[")) {
    return [{ text: line, fg: null, bold: false }];
  }
  const out: AnsiSpan[] = [];
  let fg: string | null = null;
  let bold = false;
  let buffer = "";
  let i = 0;
  const flush = () => {
    if (buffer) {
      out.push({ text: buffer, fg, bold });
      buffer = "";
    }
  };
  while (i < line.length) {
    if (line[i] === "\x1b" && line[i + 1] === "[") {
      const end = line.indexOf("m", i);
      if (end === -1) {
        // 半截转义（日志被截断在中间）：剩下的当普通文本，别把它吃掉
        buffer += line.slice(i);
        break;
      }
      flush();
      for (const part of line.slice(i + 2, end).split(";")) {
        const code = part === "" ? 0 : Number(part);
        if (!Number.isFinite(code)) continue;
        if (code === 0) {
          fg = null;
          bold = false;
        } else if (code === 1) {
          bold = true;
        } else if (code === 22) {
          bold = false;
        } else if (code === 39) {
          fg = null;
        } else if (SGR_FG[code]) {
          fg = SGR_FG[code];
        }
      }
      i = end + 1;
      continue;
    }
    buffer += line[i];
    i += 1;
  }
  flush();
  return out.length > 0 ? out : [{ text: "", fg: null, bold: false }];
}
