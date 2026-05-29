import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import { AppV2 } from './react/AppV2';

// The legacy App remains the default. Opt into the new command-core workbench
// (Phases 3/4) with ?v2 until it reaches feature parity.
const useV2 = typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('v2');

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>{useV2 ? <AppV2 /> : <App />}</React.StrictMode>
);
