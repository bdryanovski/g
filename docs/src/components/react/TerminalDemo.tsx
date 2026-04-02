import { useEffect, useMemo, useState } from "react";

type DemoId = "log" | "diff" | "workspace" | "stack";

// Lines are faithful to the actual CLI output format:
//
// log       — CommitEntry::render(): graph + hash (yellow) + subject (conventional
//             commit type colored) + author (cyan) + date (dim)
// diff      — enhanced_diff() routed through delta: file header (▌, cyan),
//             + lines (green), - lines (red), context note (dim)
// workspace — workspace::list() Table with columns: marker ◉/◯, Name, Branch,
//             Path (dim), HEAD hash (yellow·dim), Created (dim)
// stack     — stack::list(): "Stack: <name> (root: …)", then ├──/└── tree with
//             ◉ on current branch and "← you are here" (all dim connectors)
const DEMOS: Record<
  DemoId,
  { label: string; cmd: string; lines: string[] }
> = {
  log: {
    label: "Log",
    cmd: "g log",
    lines: [
      "\x1b[33m*\x1b[0m \x1b[33ma4f2b3c\x1b[0m  \x1b[32mfeat\x1b[0m(auth): add session refresh  \x1b[36mJane Doe\x1b[0m  \x1b[90m2 hours ago\x1b[0m",
      "\x1b[33m│\x1b[0m \x1b[32m*\x1b[0m \x1b[33m9d1e56f\x1b[0m  \x1b[31mfix\x1b[0m(ui): align modal footer   \x1b[36mBob Smith\x1b[0m  \x1b[90m5 hours ago\x1b[0m",
      "\x1b[33m│\x1b[0m\x1b[32m/\x1b[0m",
      "  \x1b[33m*\x1b[0m \x1b[33m3c8a901\x1b[0m  \x1b[90mchore\x1b[0m: bump lockfile          \x1b[36mJane Doe\x1b[0m  \x1b[90m1 day ago\x1b[0m",
    ],
  },
  diff: {
    label: "Diff",
    cmd: "g diff",
    lines: [
      "\x1b[36m▌ src/commands/git.rs\x1b[0m",
      "\x1b[32m+  args.push(format!(\"-n{}\", cfg.ui.log_limit));\x1b[0m",
      "\x1b[31m-  args.push(\"-n20\".to_string());\x1b[0m",
      "\x1b[90m  … 4 lines context · delta\x1b[0m",
    ],
  },
  workspace: {
    label: "Workspace",
    cmd: "g workspace list",
    lines: [
      "  \x1b[1m  Name          Branch      Path            HEAD     Created\x1b[0m",
      "  \x1b[90m── ──────────── ─────────── ─────────────── ──────── ─────────────\x1b[0m",
      "  \x1b[32m◉\x1b[0m \x1b[32mmain\x1b[0m          \x1b[32mmain\x1b[0m        ~/vcli/main     a4f2b3c  \x1b[90mjust now\x1b[0m",
      "  \x1b[90m◯\x1b[0m feature-auth  \x1b[32mfeat/auth\x1b[0m   ~/vcli/f-auth   9d1e567  \x1b[90m2 hours ago\x1b[0m",
    ],
  },
  stack: {
    label: "Stack",
    cmd: "g stack view",
    lines: [
      "  \x1b[90mStack:\x1b[0m \x1b[36mauth-stack\x1b[0m  \x1b[90m(root: main)\x1b[0m",
      "  \x1b[90m├──\x1b[0m \x1b[90m◯\x1b[0m main",
      "  \x1b[90m├──\x1b[0m \x1b[90m◯\x1b[0m feat/models",
      "  \x1b[90m└──\x1b[0m \x1b[32m◉\x1b[0m \x1b[32mfeat/api\x1b[0m  \x1b[90m← you are here\x1b[0m",
    ],
  },
};

function ansiToHtml(s: string) {
  let out = s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  out = out.replace(/\x1b\[36m/g, '<span class="t-cyan">');
  out = out.replace(/\x1b\[33m/g, '<span class="t-yellow">');
  out = out.replace(/\x1b\[32m/g, '<span class="t-green">');
  out = out.replace(/\x1b\[31m/g, '<span class="t-red">');
  out = out.replace(/\x1b\[35m/g, '<span class="t-magenta">');
  out = out.replace(/\x1b\[1m/g, '<span class="t-bold">');
  out = out.replace(/\x1b\[90m/g, '<span class="t-dim">');
  out = out.replace(/\x1b\[0m/g, "</span>");
  return out;
}

export default function TerminalDemo() {
  const [active, setActive] = useState<DemoId>("log");
  const [typed, setTyped] = useState("");
  const [phase, setPhase] = useState<"cmd" | "out">("cmd");
  const [visibleLines, setVisibleLines] = useState(0);

  const demo = DEMOS[active];

  useEffect(() => {
    setTyped("");
    setPhase("cmd");
    setVisibleLines(0);
    let i = 0;
    const cmd = demo.cmd;
    const tick = window.setInterval(() => {
      i += 1;
      setTyped(cmd.slice(0, i));
      if (i >= cmd.length) {
        window.clearInterval(tick);
        window.setTimeout(() => setPhase("out"), 280);
      }
    }, 42);
    return () => window.clearInterval(tick);
  }, [active, demo.cmd]);

  useEffect(() => {
    if (phase !== "out") {
      setVisibleLines(0);
      return;
    }
    if (visibleLines >= demo.lines.length) return;
    const t = window.setTimeout(
      () => setVisibleLines((n) => n + 1),
      visibleLines === 0 ? 120 : 200,
    );
    return () => window.clearTimeout(t);
  }, [phase, visibleLines, demo.lines.length]);

  const tabs = useMemo(() => (Object.keys(DEMOS) as DemoId[]), []);

  return (
    <div className="td-root">
      <div className="td-tabs" role="tablist" aria-label="Demo scenarios">
        {tabs.map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={active === id}
            className={active === id ? "td-tab td-tab-on" : "td-tab"}
            onClick={() => setActive(id)}
          >
            {DEMOS[id].label}
          </button>
        ))}
      </div>
      <div className="td-shell" aria-live="polite">
        <div className="td-chrome">
          <span className="td-dot td-r" />
          <span className="td-dot td-y" />
          <span className="td-dot td-g" />
          <span className="td-title">terminal</span>
        </div>
        <div className="td-body">
          <div className="td-line">
            <span className="td-prompt">❯</span>{" "}
            <span
              className="td-cmd"
              dangerouslySetInnerHTML={{
                __html: ansiToHtml(typed) + (phase === "cmd" ? "▌" : ""),
              }}
            />
          </div>
          {phase === "out" && (
            <div className="td-out">
              {demo.lines.slice(0, visibleLines).map((line, idx) => (
                <pre
                  key={idx}
                  className="td-pre"
                  dangerouslySetInnerHTML={{ __html: ansiToHtml(line) }}
                />
              ))}
            </div>
          )}
        </div>
      </div>
      <style>{`
        .td-root { display: flex; flex-direction: column; gap: 1rem; }
        .td-tabs { display: flex; flex-wrap: wrap; gap: 0.5rem; }

        /* Tab buttons live outside the dark shell — use light-theme colours */
        .td-tab {
          font: inherit;
          cursor: pointer;
          border: 1px solid rgba(0,0,0,0.1);
          background: rgba(255,255,255,0.72);
          color: #52526a;
          padding: 0.45rem 0.9rem;
          border-radius: 999px;
          font-size: 0.84rem;
          font-weight: 500;
          transition: background 0.15s, border-color 0.15s, color 0.15s;
        }
        .td-tab:hover {
          color: #1e1e30;
          background: rgba(0,0,0,0.06);
          border-color: rgba(37,99,235,0.3);
        }
        .td-tab-on {
          color: #fff;
          background: linear-gradient(115deg, #2563eb, #7c3aed, #e11d48);
          background-size: 160% 160%;
          border-color: transparent;
          font-weight: 600;
          box-shadow: 0 2px 12px rgba(37,99,235,0.25);
        }
        .td-tab-on:hover {
          color: #fff;
          filter: brightness(1.06);
        }

        /* The terminal window itself stays dark */
        .td-shell {
          border-radius: 14px;
          border: 1px solid rgba(255,255,255,0.1);
          background: linear-gradient(165deg, #0d0f1c, #08090f);
          box-shadow: 0 16px 48px rgba(0,0,0,0.28), 0 0 32px rgba(124,58,237,0.05), inset 0 1px 0 rgba(255,255,255,0.05);
          overflow: hidden;
        }
        .td-chrome {
          display: flex; align-items: center; gap: 0.45rem;
          padding: 0.65rem 1rem;
          border-bottom: 1px solid rgba(255,255,255,0.06);
          background: rgba(0,0,0,0.28);
        }
        .td-dot { width: 10px; height: 10px; border-radius: 50%; opacity: 0.85; }
        .td-r { background: #fb7185; }
        .td-y { background: #fbbf24; }
        .td-g { background: #4ade80; }
        .td-title {
          margin-left: 0.5rem; font-size: 0.72rem; letter-spacing: 0.12em;
          text-transform: uppercase; color: #6b6d7a;
        }
        .td-body {
          padding: 1rem 1.1rem 1.25rem;
          font-family: "JetBrains Mono", ui-monospace, monospace;
          font-size: 0.82rem;
          line-height: 1.6;
        }
        .td-prompt { color: #60a5fa; margin-right: 0.25rem; }
        .td-cmd { color: #e8e9ef; white-space: pre-wrap; word-break: break-all; }
        .td-out {
          margin-top: 0.65rem;
          padding-top: 0.65rem;
          border-top: 1px dashed rgba(255,255,255,0.07);
        }
        /* Reset every global pre override — each line is just a terminal row */
        .td-pre {
          margin: 0;
          padding: 0 !important;
          background: none !important;
          border: none !important;
          box-shadow: none !important;
          border-radius: 0 !important;
          font-family: inherit;
          font-size: inherit;
          line-height: 1.6;
          white-space: pre-wrap;
          word-break: break-word;
          color: #c8cad8;
        }

        /* ANSI colour spans */
        :global(.t-cyan)    { color: #67e8f9; }
        :global(.t-yellow)  { color: #fcd34d; }
        :global(.t-green)   { color: #86efac; }
        :global(.t-red)     { color: #fca5a5; }
        :global(.t-magenta) { color: #e879f9; }
        :global(.t-bold)    { font-weight: 600; color: #f4f4f8; }
        :global(.t-dim)     { color: #7c7f8e; }
      `}</style>
    </div>
  );
}
