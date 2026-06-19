/**
 * ChatPanel — the AI-only control surface. The human types intent in natural
 * language; the agent loop (core/llm/agent) drives the SAME command registry the
 * workbench exposes, and the 3D ViewportV2 (subscribed to AppState) shows the
 * result. The conversation also shows the *process*: every command the AI runs
 * appears as an action chip with its outcome.
 *
 * No ribbon, no forms — this is the "operate by talking to the AI" workbench.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  createClaudeProvider,
  createMcpBridge,
  createMockProvider,
  createOllamaProvider,
  runAgentTurn,
  type AgentEvent,
  type LlmMessage,
  type LlmProvider,
} from '../core';
import { useCore } from './CoreContext';
import { SYSTEM_PROMPT, sceneContext } from './agentPrompt';

type ProviderId = 'claude' | 'ollama' | 'mock';

type ChatItem =
  | { kind: 'user'; id: number; text: string }
  | { kind: 'assistant'; id: number; text: string }
  | { kind: 'action'; id: number; callId: string; name: string; input: string; status: 'running' | 'ok' | 'fail'; detail?: string; startedAt?: number }
  | { kind: 'error'; id: number; text: string };

const SUGGESTIONS = [
  'Create a 2×1×1 box at the origin',
  'Make a sphere of radius 0.5, then mesh it 20×20×20',
  'List everything currently in the scene',
];

function readStore(key: string, fallback: string): string {
  try {
    if (typeof window !== 'undefined' && window.localStorage) {
      return window.localStorage.getItem(key) ?? fallback;
    }
  } catch {
    // ignore
  }
  return fallback;
}

function writeStore(key: string, value: string): void {
  try {
    if (typeof window !== 'undefined' && window.localStorage) window.localStorage.setItem(key, value);
  } catch {
    // ignore quota / availability
  }
}

function summarize(value: unknown): string {
  let s: string;
  try {
    s = typeof value === 'string' ? value : JSON.stringify(value);
  } catch {
    s = String(value);
  }
  return s.length > 280 ? `${s.slice(0, 280)}…` : s;
}

export function ChatPanel() {
  const core = useCore();
  const bridge = useMemo(() => createMcpBridge(core), [core]);

  // Persist provider + API key across restarts so the live loop is usable.
  // (localStorage is plaintext — fine for a local desktop tool; Electron
  // safeStorage would be the hardened option.)
  const [providerId, setProviderIdState] = useState<ProviderId>(() => readStore('gfd.llm.provider', 'claude') as ProviderId);
  const [apiKey, setApiKeyState] = useState(() => readStore('gfd.llm.claudeKey', ''));
  const setProviderId = useCallback((p: ProviderId) => {
    setProviderIdState(p);
    writeStore('gfd.llm.provider', p);
  }, []);
  const setApiKey = useCallback((k: string) => {
    setApiKeyState(k);
    writeStore('gfd.llm.claudeKey', k);
  }, []);
  const [items, setItems] = useState<ChatItem[]>([]);
  const [input, setInput] = useState('');
  const [busy, setBusy] = useState(false);

  const historyRef = useRef<LlmMessage[]>([]);
  const idRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const nextId = () => (idRef.current += 1);

  const makeProvider = useCallback((): LlmProvider => {
    if (providerId === 'claude') return createClaudeProvider({ apiKey });
    if (providerId === 'ollama') return createOllamaProvider();
    return createMockProvider('mock', [
      { type: 'text', text: '(mock provider — pick Claude with an API key, or run a local Ollama model, to actually drive the workbench.)' },
      { type: 'done' },
    ]);
  }, [providerId, apiKey]);

  const send = useCallback(async () => {
    const text = input.trim();
    if (!text || busy) return;
    if (providerId === 'claude' && !apiKey.trim()) {
      setItems((prev) => [...prev, { kind: 'error', id: nextId(), text: 'Enter a Claude API key (or switch provider).' }]);
      return;
    }

    setInput('');
    setBusy(true);
    setItems((prev) => [...prev, { kind: 'user', id: nextId(), text }]);

    const controller = new AbortController();
    abortRef.current = controller;

    const onEvent = (e: AgentEvent) => {
      setItems((prev) => {
        if (e.type === 'assistant_text') {
          const last = prev[prev.length - 1];
          if (last && last.kind === 'assistant') {
            return [...prev.slice(0, -1), { ...last, text: last.text + e.text }];
          }
          return [...prev, { kind: 'assistant', id: nextId(), text: e.text }];
        }
        if (e.type === 'tool_call') {
          return [...prev, { kind: 'action', id: nextId(), callId: e.id, name: e.name, input: summarize(e.input), status: 'running', startedAt: performance.now() }];
        }
        if (e.type === 'tool_result') {
          return prev.map((it) =>
            it.kind === 'action' && it.callId === e.id && it.status === 'running'
              ? { ...it, status: e.ok ? 'ok' : 'fail', detail: summarize(e.result) }
              : it
          );
        }
        if (e.type === 'error') {
          return [...prev, { kind: 'error', id: nextId(), text: e.message }];
        }
        return prev;
      });
    };

    try {
      // Inject a fresh, compact scene snapshot each turn so the AI builds with
      // real coordinates instead of guessing.
      const system = `${SYSTEM_PROMPT}\n\n${sceneContext(core.store.getState())}`;
      const { history } = await runAgentTurn(
        { provider: makeProvider(), bridge, history: historyRef.current, userText: text, system, signal: controller.signal },
        onEvent
      );
      historyRef.current = history;
    } catch (err) {
      setItems((prev) => [...prev, { kind: 'error', id: nextId(), text: err instanceof Error ? err.message : String(err) }]);
    } finally {
      setBusy(false);
      abortRef.current = null;
    }
  }, [input, busy, providerId, apiKey, makeProvider, bridge, core]);

  const stop = useCallback(() => abortRef.current?.abort(), []);

  // Pipeline controls — also runnable by the AI (undo/redo are MCP meta-tools;
  // view.reset / view.save_defaults are commands). These buttons give the human
  // the same control directly from the chat.
  const undo = useCallback(() => void core.dispatcher.undo(), [core]);
  const redo = useCallback(() => void core.dispatcher.redo(), [core]);
  const runControl = useCallback(
    (commandId: string) => void core.dispatcher.dispatch({ commandId, params: {}, source: 'human' }),
    [core]
  );

  // Upload a CAD file (STEP/IGES/BREP/STL) straight from the chat → OCC import →
  // the viewport updates. Shows a running action chip with elapsed time.
  const fileRef = useRef<HTMLInputElement>(null);
  const importFile = useCallback(
    async (path: string, label: string) => {
      const aid = nextId();
      const callId = `import-${aid}`;
      setItems((prev) => [...prev, { kind: 'action', id: aid, callId, name: 'gmsh.import', input: label, status: 'running', startedAt: performance.now() }]);
      const finish = (status: 'ok' | 'fail', detail: string) =>
        setItems((prev) => prev.map((it) => (it.kind === 'action' && it.callId === callId ? { ...it, status, detail } : it)));
      try {
        const outcome = await core.dispatcher.dispatch({ commandId: 'gmsh.import', params: { path }, source: 'human' });
        if (outcome.ok) finish('ok', summarize(outcome.result));
        else finish('fail', outcome.error?.message ?? 'import failed');
      } catch (err) {
        finish('fail', err instanceof Error ? err.message : String(err));
      }
    },
    [core]
  );

  // Re-render every 0.5s while something is running so elapsed-time tickers update.
  const hasRunning = items.some((it) => it.kind === 'action' && it.status === 'running');
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!hasRunning && !busy) return;
    const h = setInterval(() => setTick((t) => t + 1), 500);
    return () => clearInterval(h);
  }, [hasRunning, busy]);
  const now = performance.now();

  return (
    <div style={S.panel}>
      <div style={S.header}>
        <span style={{ color: '#4096ff', fontWeight: 600 }}>AI</span>
        <span style={{ color: '#888', fontSize: 11 }}>talk to operate the workbench</span>
        <div style={{ flex: 1 }} />
        <select
          value={providerId}
          onChange={(e) => setProviderId(e.target.value as ProviderId)}
          style={S.select}
        >
          <option value="claude">Claude</option>
          <option value="ollama">Ollama (local)</option>
          <option value="mock">Mock (offline)</option>
        </select>
      </div>

      <div style={S.controls}>
        <button onClick={undo} style={S.ctlBtn} title="Undo the last command (or just ask the AI)">↶ Undo</button>
        <button onClick={redo} style={S.ctlBtn} title="Redo">↷ Redo</button>
        <span style={{ width: 1, alignSelf: 'stretch', background: '#2a2a2a', margin: '0 2px' }} />
        <button onClick={() => runControl('view.reset')} style={S.ctlBtn} title="Reset camera / render / viz to saved defaults">⤺ Reset view</button>
        <button onClick={() => runControl('view.save_defaults')} style={S.ctlBtn} title="Save the current view as the defaults">★ Save defaults</button>
        <div style={{ flex: 1 }} />
        <button onClick={() => fileRef.current?.click()} style={S.ctlBtn} title="Import a CAD file (STEP/IGES/BREP/STL)">📎 Import CAD</button>
        <input
          ref={fileRef}
          type="file"
          accept=".step,.stp,.iges,.igs,.brep,.stl"
          style={{ display: 'none' }}
          onChange={(e) => {
            const f = e.target.files?.[0];
            // Electron 32+ removed File.path → use webUtils via the preload bridge,
            // with a fallback to the legacy property for older runtimes.
            const path = f
              ? (typeof window !== 'undefined' && window.gfdAPI?.getPathForFile?.(f)) || (f as File & { path?: string }).path || ''
              : '';
            if (f && path) void importFile(path, f.name);
            else if (f) setItems((prev) => [...prev, { kind: 'error', id: nextId(), text: 'Could not read the file path (run the desktop app, not a browser).' }]);
            e.target.value = '';
          }}
        />
      </div>

      {providerId === 'claude' && (
        <input
          type="password"
          value={apiKey}
          onChange={(e) => setApiKey(e.target.value)}
          placeholder="Claude API key (sk-ant-…)"
          style={S.keyInput}
        />
      )}

      <div style={S.log}>
        {items.length === 0 && (
          <div style={{ color: '#777', fontSize: 12 }}>
            <p style={{ marginTop: 0 }}>Tell the assistant what to build or compute. It drives every command for you.</p>
            {SUGGESTIONS.map((s) => (
              <button key={s} onClick={() => setInput(s)} style={S.suggest}>{s}</button>
            ))}
          </div>
        )}
        {items.map((it) => (
          <ChatRow key={it.id} item={it} now={now} />
        ))}
      </div>

      <div style={S.inputRow}>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          placeholder={busy ? 'Working…' : 'Ask the assistant to do something (Enter to send)'}
          rows={2}
          style={S.textarea}
          disabled={busy}
        />
        {busy ? (
          <button onClick={stop} style={{ ...S.sendBtn, background: '#5a2a2a' }}>Stop</button>
        ) : (
          <button onClick={() => void send()} style={S.sendBtn}>Send</button>
        )}
      </div>
    </div>
  );
}

function ChatRow({ item, now }: { item: ChatItem; now: number }) {
  if (item.kind === 'user') {
    return <div style={{ ...S.bubble, ...S.user }}>{item.text}</div>;
  }
  if (item.kind === 'assistant') {
    return <div style={{ ...S.bubble, ...S.assistant }}>{item.text}</div>;
  }
  if (item.kind === 'error') {
    return <div style={{ ...S.bubble, ...S.errorBubble }}>⚠ {item.text}</div>;
  }
  const color = item.status === 'ok' ? '#3fb950' : item.status === 'fail' ? '#f85149' : '#d29922';
  const icon = item.status === 'ok' ? '✓' : item.status === 'fail' ? '✗' : '⋯';
  const elapsed = item.status === 'running' && typeof item.startedAt === 'number'
    ? ` ${((now - item.startedAt) / 1000).toFixed(1)}s`
    : '';
  return (
    <div style={S.action}>
      <div style={{ display: 'flex', gap: 6, alignItems: 'center' }}>
        <span style={{ color }}>{icon}</span>
        <code style={{ color: '#9cdcfe', fontSize: 11 }}>{item.name}</code>
        <span style={{ color: '#666', fontSize: 11 }}>{item.input}</span>
        {elapsed && <span style={{ color: '#d29922', fontSize: 11, marginLeft: 'auto' }}>{elapsed}</span>}
      </div>
      {item.detail && <div style={{ color: '#888', fontSize: 11, marginLeft: 18, marginTop: 2 }}>{item.detail}</div>}
    </div>
  );
}

const S: Record<string, React.CSSProperties> = {
  panel: { display: 'flex', flexDirection: 'column', height: '100%', background: '#0d0f13', color: '#ddd' },
  header: { display: 'flex', alignItems: 'center', gap: 8, padding: '6px 10px', borderBottom: '1px solid #2a2a2a' },
  controls: { display: 'flex', alignItems: 'center', gap: 4, padding: '4px 8px', borderBottom: '1px solid #2a2a2a', background: '#0b0d11' },
  ctlBtn: { padding: '2px 8px', background: '#16181d', color: '#cbd3dc', border: '1px solid #2a2f38', borderRadius: 5, cursor: 'pointer', fontSize: 11 },
  select: { background: '#1a1a1a', color: '#ddd', border: '1px solid #333', borderRadius: 4, fontSize: 12, padding: '2px 4px' },
  keyInput: { margin: '6px 10px 0', padding: '4px 8px', background: '#1a1a1a', color: '#ddd', border: '1px solid #333', borderRadius: 4, fontSize: 12 },
  log: { flex: 1, overflowY: 'auto', padding: 10, display: 'flex', flexDirection: 'column', gap: 8 },
  bubble: { padding: '6px 10px', borderRadius: 8, fontSize: 13, lineHeight: 1.45, whiteSpace: 'pre-wrap', maxWidth: '92%' },
  user: { alignSelf: 'flex-end', background: '#1f3a5f', color: '#e8f0ff' },
  assistant: { alignSelf: 'flex-start', background: '#1c1f26' },
  errorBubble: { alignSelf: 'flex-start', background: '#3a1d1d', color: '#ffd7d7' },
  action: { alignSelf: 'flex-start', background: '#15171c', border: '1px solid #262a31', borderRadius: 6, padding: '4px 8px', maxWidth: '92%' },
  inputRow: { display: 'flex', gap: 6, padding: 8, borderTop: '1px solid #2a2a2a' },
  textarea: { flex: 1, resize: 'none', background: '#1a1a1a', color: '#ddd', border: '1px solid #333', borderRadius: 6, padding: '6px 8px', fontSize: 13, fontFamily: 'inherit' },
  sendBtn: { padding: '0 16px', background: '#2563eb', color: '#fff', border: 'none', borderRadius: 6, cursor: 'pointer', fontSize: 13 },
  suggest: { display: 'block', width: '100%', textAlign: 'left', margin: '4px 0', padding: '6px 8px', background: '#16181d', color: '#bbb', border: '1px solid #2a2a2a', borderRadius: 6, cursor: 'pointer', fontSize: 12 },
};
