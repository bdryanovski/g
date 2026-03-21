import { useCallback, useEffect, useId, useState, type ReactNode } from "react";

function useReducedMotion(): boolean {
  const [reduced, setReduced] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    setReduced(mq.matches);
    const fn = () => setReduced(mq.matches);
    mq.addEventListener("change", fn);
    return () => mq.removeEventListener("change", fn);
  }, []);
  return reduced;
}

function StackDiagram({
  cycle,
  reduced,
  lineGradId,
  filterId,
}: {
  cycle: number;
  reduced: boolean;
  lineGradId: string;
  filterId: string;
}) {
  return (
    <svg
      key={cycle}
      viewBox="0 0 420 220"
      className="gfv-svg gfv-svg--stack"
      role="presentation"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={lineGradId} x1="0%" y1="100%" x2="0%" y2="0%">
          <stop offset="0%" stopColor="#00f5ff" />
          <stop offset="55%" stopColor="#a78bfa" />
          <stop offset="100%" stopColor="#fb7185" />
        </linearGradient>
        <filter id={`${filterId}-glow`} x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="1.8" result="b" />
          <feMerge>
            <feMergeNode in="b" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <path
        d="M 210 170 L 210 110"
        fill="none"
        stroke={`url(#${lineGradId})`}
        strokeWidth="3"
        strokeLinecap="round"
        className={reduced ? "" : "gfv-stack-seg gfv-stack-seg--1"}
      />
      <path
        d="M 210 90 L 210 52"
        fill="none"
        stroke={`url(#${lineGradId})`}
        strokeWidth="3"
        strokeLinecap="round"
        className={reduced ? "" : "gfv-stack-seg gfv-stack-seg--2"}
      />
      <g className={reduced ? "" : "gfv-stack-node-wrap gfv-stack-n1"}>
        <circle
          cx="210"
          cy="180"
          r="10"
          className="gfv-node gfv-node--glow"
          filter={reduced ? undefined : `url(#${filterId}-glow)`}
        />
      </g>
      <text x="230" y="186" className={`gfv-lbl ${reduced ? "" : "gfv-lbl--stack gfv-lbl--s1"}`}>
        main
      </text>
      <g className={reduced ? "" : "gfv-stack-node-wrap gfv-stack-n2"}>
        <circle
          cx="210"
          cy="100"
          r="10"
          className="gfv-node gfv-node--violet gfv-node--glow"
          filter={reduced ? undefined : `url(#${filterId}-glow)`}
        />
      </g>
      <text x="230" y="106" className={`gfv-lbl ${reduced ? "" : "gfv-lbl--stack gfv-lbl--s2"}`}>
        feat/models
      </text>
      <g className={reduced ? "" : "gfv-stack-node-wrap gfv-stack-n3"}>
        <circle
          cx="210"
          cy="40"
          r="10"
          className="gfv-node gfv-node--rose gfv-node--glow"
          filter={reduced ? undefined : `url(#${filterId}-glow)`}
        />
      </g>
      <text x="230" y="46" className={`gfv-lbl ${reduced ? "" : "gfv-lbl--stack gfv-lbl--s3"}`}>
        feat/api · HEAD
      </text>
      <text x="20" y="205" className={`gfv-cap ${reduced ? "" : "gfv-cap--fade"}`}>
        Each branch builds on the one below — PR #2 targets feat/models, PR #1 targets main.
      </text>
    </svg>
  );
}

function WorktreeDiagram({
  cycle,
  reduced,
  accentId,
}: {
  cycle: number;
  reduced: boolean;
  accentId: string;
}) {
  return (
    <svg
      key={cycle}
      viewBox="0 0 440 200"
      className="gfv-svg gfv-svg--wt"
      role="presentation"
      aria-hidden="true"
    >
      <defs>
        <linearGradient id={accentId} x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#00f5ff" />
          <stop offset="100%" stopColor="#c084fc" />
        </linearGradient>
      </defs>
      <g className={reduced ? "" : "gfv-wt-main"}>
        <rect x="140" y="20" width="160" height="52" rx="10" className="gfv-box gfv-box-main" />
        <text x="220" y="52" textAnchor="middle" className="gfv-boxtxt">
          myapp/ · main
        </text>
      </g>
      <path
        d="M 220 72 L 220 92"
        fill="none"
        stroke={`url(#${accentId})`}
        strokeWidth="2.5"
        strokeLinecap="round"
        className={reduced ? "" : "gfv-wt-path gfv-wt-path--a"}
      />
      <path
        d="M 220 92 L 120 118"
        fill="none"
        stroke={`url(#${accentId})`}
        strokeWidth="2.5"
        strokeLinecap="round"
        className={reduced ? "" : "gfv-wt-path gfv-wt-path--b"}
      />
      <path
        d="M 220 92 L 320 118"
        fill="none"
        stroke={`url(#${accentId})`}
        strokeWidth="2.5"
        strokeLinecap="round"
        className={reduced ? "" : "gfv-wt-path gfv-wt-path--c"}
      />
      <g className={reduced ? "" : "gfv-wt-leaf gfv-wt-leaf--L"}>
        <rect x="40" y="120" width="160" height="52" rx="10" className="gfv-box gfv-box-wt" />
        <text x="120" y="152" textAnchor="middle" className="gfv-boxtxt">
          myapp--auth/
        </text>
        <text x="120" y="188" textAnchor="middle" className="gfv-sub">
          feat/auth
        </text>
      </g>
      <g className={reduced ? "" : "gfv-wt-leaf gfv-wt-leaf--R"}>
        <rect x="240" y="120" width="160" height="52" rx="10" className="gfv-box gfv-box-wt" />
        <text x="320" y="152" textAnchor="middle" className="gfv-boxtxt">
          myapp--hotfix/
        </text>
        <text x="320" y="188" textAnchor="middle" className="gfv-sub">
          fix/login
        </text>
      </g>
    </svg>
  );
}

function SyncDiagram({
  cycle,
  reduced,
  gradId,
}: {
  cycle: number;
  reduced: boolean;
  gradId: string;
}) {
  return (
    <div key={cycle} className="gfv-sync">
      <div className={`gfv-sync-row gfv-sync-before ${reduced ? "" : "gfv-sync-before--play"}`}>
        <span className="gfv-sync-label">Before</span>
        <svg viewBox="0 0 200 100" className="gfv-svg gfv-svg--sm" aria-hidden="true">
          <path
            d="M 100 80 L 100 52 L 132 28 M 100 52 L 68 28"
            fill="none"
            stroke="#6b7280"
            strokeWidth="2.5"
            strokeLinecap="round"
            className={reduced ? "" : "gfv-sync-messy"}
          />
          <circle cx="100" cy="80" r="8" className={reduced ? "" : "gfv-sync-dot gfv-sync-dot--dim"} />
          <circle cx="132" cy="28" r="8" className={reduced ? "" : "gfv-sync-dot gfv-sync-dot--dim gfv-sync-dot--d1"} />
          <circle cx="68" cy="28" r="8" className={reduced ? "" : "gfv-sync-dot gfv-sync-dot--dim gfv-sync-dot--d2"} />
        </svg>
        <span className="gfv-sync-hint">Tips diverge — stack is not a straight spine.</span>
      </div>
      <div className={`gfv-sync-arrow ${reduced ? "" : "gfv-sync-arrow--play"}`} aria-hidden="true">
        <span className="gfv-sync-cmd">
          <span className="gfv-sync-cmd-inner">g stack sync</span>
        </span>
      </div>
      <div className={`gfv-sync-row gfv-sync-after ${reduced ? "" : "gfv-sync-after--play"}`}>
        <span className="gfv-sync-label">After</span>
        <svg viewBox="0 0 200 100" className="gfv-svg gfv-svg--sm" aria-hidden="true">
          <defs>
            <linearGradient id={gradId} x1="0%" y1="100%" x2="0%" y2="0%">
              <stop offset="0%" stopColor="#00f5ff" />
              <stop offset="100%" stopColor="#c084fc" />
            </linearGradient>
          </defs>
          <path
            d="M 100 82 L 100 56 L 100 30"
            fill="none"
            stroke={`url(#${gradId})`}
            strokeWidth="3"
            strokeLinecap="round"
            className={reduced ? "" : "gfv-sync-clean"}
          />
          <circle cx="100" cy="82" r="8.5" className={reduced ? "" : "gfv-sync-pop gfv-sync-pop--1"} fill="#00e5ff" />
          <circle cx="100" cy="56" r="8.5" className={reduced ? "" : "gfv-sync-pop gfv-sync-pop--2"} fill="#c084fc" />
          <circle cx="100" cy="30" r="8.5" className={reduced ? "" : "gfv-sync-pop gfv-sync-pop--3"} fill="#fb7185" />
        </svg>
        <span className="gfv-sync-hint">Linear again: each branch rebased onto the one below.</span>
      </div>
    </div>
  );
}

function FlowFigure({
  caption,
  children,
}: {
  caption: string;
  children: (ctx: { cycle: number; reduced: boolean }) => ReactNode;
}) {
  const reduced = useReducedMotion();
  const [cycle, setCycle] = useState(0);
  const replay = useCallback(() => setCycle((c) => c + 1), []);

  return (
    <figure className={`doc-flow gfv${reduced ? " gfv--reduced" : ""}`}>
      <div className="doc-flow-toolbar">
        <figcaption className="doc-flow-legend">{caption}</figcaption>
        <button type="button" className="gfv-replay" onClick={replay}>
          Replay
        </button>
      </div>
      <div className="doc-flow-canvas">{children({ cycle, reduced })}</div>
    </figure>
  );
}

/** Linear stacked branches — place after explaining stack order / PR bases */
export function DocFlowStack({ caption }: { caption: string }) {
  const uid = useId().replace(/:/g, "");
  const lineGradId = `gfv-lg-${uid}`;
  const filterId = `gfv-fl-${uid}`;

  return (
    <FlowFigure caption={caption}>
      {({ cycle, reduced }) => (
        <StackDiagram cycle={cycle} reduced={reduced} lineGradId={lineGradId} filterId={filterId} />
      )}
    </FlowFigure>
  );
}

/** Main repo + sibling worktrees — place after explaining worktree layout */
export function DocFlowWorktree({ caption }: { caption: string }) {
  const uid = useId().replace(/:/g, "");
  const accentId = `gfv-wtg-${uid}`;

  return (
    <FlowFigure caption={caption}>
      {({ cycle, reduced }) => <WorktreeDiagram cycle={cycle} reduced={reduced} accentId={accentId} />}
    </FlowFigure>
  );
}

/** Before/after stack sync — place where `g stack sync` is explained */
export function DocFlowSync({ caption }: { caption: string }) {
  const uid = useId().replace(/:/g, "");
  const gradId = `gfv-sync-${uid}`;

  return (
    <FlowFigure caption={caption}>
      {({ cycle, reduced }) => <SyncDiagram cycle={cycle} reduced={reduced} gradId={gradId} />}
    </FlowFigure>
  );
}
