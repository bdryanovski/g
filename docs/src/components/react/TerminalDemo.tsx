import { useEffect, useMemo, useState } from "react";

type DemoId = "log" | "diff" | "workspace" | "stack";

const DEMOS: Record<
  DemoId,
  { label: string; cmd: string; lines: string[] }
> = {
  log: {
    label: "Log",
    cmd: "g log -n 4 --oneline",
    lines: [
      "* ··\x1b[36m●\x1b[0m \x1b[33mfeat(auth)\x1b[0m add session refresh   \x1b[90m· 2h\x1b[0m",
      "│ * \x1b[33mfix(ui)\x1b[0m align modal footer        \x1b[90m· 5h\x1b[0m",
      "│/  ",
      "* \x1b[33mchore\x1b[0m bump deps                    \x1b[90m· 1d\x1b[0m",
    ],
  },
  diff: {
    label: "Diff",
    cmd: "g diff src/main.rs",
    lines: [
      "\x1b[36m▌ src/main.rs\x1b[0m",
      "\x1b[32m+    println!(\"g\");\x1b[0m",
      "\x1b[31m-    println!(\"hello\");\x1b[0m",
      "\x1b[90m  … 3 lines hidden · delta\x1b[0m",
    ],
  },
  workspace: {
    label: "Workspace",
    cmd: "g workspace list",
    lines: [
      "\x1b[1mworkspaces\x1b[0m",
      "  \x1b[32m●\x1b[0m \x1b[1mmyapp\x1b[0m        \x1b[90mmain\x1b[0m     \x1b[90m~/proj/myapp\x1b[0m",
      "  \x1b[36m○\x1b[0m \x1b[1mfeature-auth\x1b[0m \x1b[90mfeat/a\x1b[0m \x1b[90m~/proj/myapp--feature-auth\x1b[0m",
    ],
  },
  stack: {
    label: "Stack",
    cmd: "g stack view",
    lines: [
      "\x1b[1mauth-stack\x1b[0m",
      "  \x1b[90m├\x1b[0m \x1b[36mmain\x1b[0m",
      "  \x1b[90m├\x1b[0m \x1b[35mfeat/models\x1b[0m",
      "  \x1b[90m└\x1b[0m \x1b[35mfeat/api\x1b[0m  \x1b[90m← HEAD\x1b[0m",
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
        .td-tabs {
          display: flex; flex-wrap: wrap; gap: 0.5rem;
        }
        .td-tab {
          font: inherit;
          cursor: pointer;
          border: 1px solid rgba(255,255,255,0.12);
          background: rgba(255,255,255,0.04);
          color: #b7b9c5;
          padding: 0.45rem 0.9rem;
          border-radius: 999px;
          transition: background 0.15s, border-color 0.15s, color 0.15s;
        }
        .td-tab:hover { color: #f4f4f8; border-color: rgba(0,245,255,0.45); }
        .td-tab-on {
          color: #050508;
          background: linear-gradient(115deg, #00f5ff, #c084fc, #fb7185);
          background-size: 160% 160%;
          border-color: transparent;
          font-weight: 600;
          box-shadow: 0 0 20px rgba(0,245,255,0.25);
        }
        .td-shell {
          border-radius: 14px;
          border: 1px solid rgba(0,245,255,0.12);
          background: linear-gradient(165deg, rgba(10,12,22,0.96), rgba(5,6,12,0.99));
          box-shadow: 0 28px 90px rgba(0,0,0,0.55), 0 0 40px rgba(192,132,252,0.06), inset 0 1px 0 rgba(255,255,255,0.05);
          overflow: hidden;
        }
        .td-chrome {
          display: flex; align-items: center; gap: 0.45rem;
          padding: 0.65rem 1rem;
          border-bottom: 1px solid rgba(255,255,255,0.06);
          background: rgba(0,0,0,0.25);
        }
        .td-dot { width: 10px; height: 10px; border-radius: 50%; opacity: 0.85; }
        .td-r { background: #fb7185; }
        .td-y { background: #fbbf24; }
        .td-g { background: #4ade80; }
        .td-title {
          margin-left: 0.5rem; font-size: 0.72rem; letter-spacing: 0.12em;
          text-transform: uppercase; color: #6b6d7a;
        }
        .td-body { padding: 1rem 1.1rem 1.25rem; font-family: "JetBrains Mono", ui-monospace, monospace; font-size: 0.82rem; line-height: 1.55; }
        .td-prompt { color: #00f5ff; margin-right: 0.25rem; text-shadow: 0 0 12px rgba(0,245,255,0.35); }
        .td-cmd { color: #e8e9ef; white-space: pre-wrap; word-break: break-all; }
        .td-out { margin-top: 0.75rem; padding-top: 0.75rem; border-top: 1px dashed rgba(255,255,255,0.08); }
        .td-pre {
          margin: 0 0 0.2rem;
          font-family: inherit;
          font-size: inherit;
          white-space: pre-wrap;
          word-break: break-word;
          color: #d1d3de;
        }
        :global(.t-cyan) { color: #67e8f9; }
        :global(.t-yellow) { color: #fcd34d; }
        :global(.t-green) { color: #86efac; }
        :global(.t-red) { color: #fca5a5; }
        :global(.t-magenta) { color: #e879f9; }
        :global(.t-bold) { font-weight: 600; color: #f4f4f8; }
        :global(.t-dim) { color: #7c7f8e; }
      `}</style>
    </div>
  );
}
