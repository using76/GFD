import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { AppV2 } from './react/AppV2';
import { AssistantShell } from './react/AssistantShell';

// Entry routing (opt-in via query string):
//   ?ai  → AssistantShell — the AI-only workbench (talk to the AI; it drives the
//          commands; the viewport shows only the result + process). The target
//          direction for the GUI.
//   ?v2  → AppV2 — data-driven ribbon workbench on the command-core.
//   else → the legacy App (default until the AI shell reaches parity).
const params = typeof window !== 'undefined' ? new URLSearchParams(window.location.search) : new URLSearchParams();
const root = ReactDOM.createRoot(document.getElementById('root')!);

function pickApp() {
  if (params.has('ai')) return <AssistantShell />;
  if (params.has('v2')) return <AppV2 />;
  return <App />;
}

root.render(<React.StrictMode>{pickApp()}</React.StrictMode>);
