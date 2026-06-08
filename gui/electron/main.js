const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { spawn } = require('child_process');

let mainWindow = null;
let gfdProcess = null;
// Newline-delimited JSON framing for the gfd-server stdio channel. stdout 'data'
// chunks do NOT align to message boundaries — a large response (e.g. tessellation
// is several KB) spans multiple chunks — so we buffer, split on '\n', and route
// each complete line by request id (a pending resolver) or, if id-less, as an
// async event. The previous per-request listener could not reassemble a JSON
// object split across chunks, so big responses silently timed out.
let gfdStdoutBuffer = '';
const gfdPending = new Map(); // id -> { resolve, timer }
// The gfd-server expects the JSON-RPC `id` to be a u64 (number), so use an
// incrementing integer — NOT a string (a string id is rejected with a JSON
// parse error and the request then times out).
let gfdRequestId = 0;

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false,
    },
    title: 'GFD - Generalized Fluid Dynamics',
  });

  // Open DevTools in development
  const isDev = process.argv.includes('--dev') || !app.isPackaged;
  if (isDev) {
    mainWindow.webContents.openDevTools({ mode: 'bottom' });
  }

  // Prevent crash from unhandled renderer errors
  mainWindow.webContents.on('render-process-gone', (event, details) => {
    console.error('[Electron] Renderer crashed:', details.reason);
    mainWindow.reload();
  });

  mainWindow.webContents.on('crashed', () => {
    console.error('[Electron] WebContents crashed, reloading...');
    mainWindow.reload();
  });

  // Which workbench shell to open: ?ai = AI-only assistant workbench (default),
  // ?v2 = data-driven ribbon, none = legacy. Override with GFD_SHELL=v2|legacy|ai.
  const shell = process.env.GFD_SHELL || 'ai';
  const query = shell === 'legacy' ? '' : `?${shell}`;

  // In development, load from Vite dev server; in production, load built files
  if (isDev) {
    mainWindow.loadURL(`http://localhost:5173/${query}`).catch(() => {
      // Fallback to built files if dev server is not running
      mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'), query ? { search: query } : {});
    });
  } else {
    mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'), query ? { search: query } : {});
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

function spawnGfdBackend() {
  const fs = require('fs');
  // Packaged app: resources/bin/gfd-server.exe (Gmsh DLL is bundled alongside it,
  // so it loads from the exe directory). Dev: target/release/gfd-server(.exe).
  const packagedBinary = path.join(process.resourcesPath || '', 'bin', 'gfd-server.exe');
  const devBinary = path.join(__dirname, '..', '..', 'target', 'release', 'gfd-server');

  let binary;
  const args = [];
  if (app.isPackaged && fs.existsSync(packagedBinary)) {
    binary = packagedBinary;
  } else if (fs.existsSync(devBinary + '.exe') || fs.existsSync(devBinary)) {
    binary = devBinary;
  } else {
    // GUI works without backend (browser simulation mode)
    console.log('[GFD] No backend binary found. GUI running in simulation mode.');
    console.log('[GFD] Build with: cargo build --release --bin gfd-server --features gmsh');
    return;
  }

  try {
    gfdProcess = spawn(binary, args, {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    gfdStdoutBuffer = '';

    gfdProcess.stdout.on('data', (data) => {
      gfdStdoutBuffer += data.toString();
      let nl;
      while ((nl = gfdStdoutBuffer.indexOf('\n')) >= 0) {
        const line = gfdStdoutBuffer.slice(0, nl);
        gfdStdoutBuffer = gfdStdoutBuffer.slice(nl + 1);
        if (!line.trim()) continue;
        let msg;
        try {
          msg = JSON.parse(line);
        } catch {
          // Non-JSON output (log lines, etc.)
          console.log('[GFD]', line);
          continue;
        }
        if (msg && msg.id !== undefined) {
          // A response to a request — resolve its pending promise.
          const pending = gfdPending.get(msg.id);
          if (pending) {
            clearTimeout(pending.timer);
            gfdPending.delete(msg.id);
            pending.resolve(msg.result ?? { error: msg.error });
          }
          // else: stale/unmatched response (e.g. after a timeout) → drop.
        } else if (mainWindow && !mainWindow.isDestroyed()) {
          // No id → an async event (solver progress, etc.).
          mainWindow.webContents.send('gfd:event', msg);
        }
      }
    });

    gfdProcess.stderr.on('data', (data) => {
      console.error('[GFD stderr]', data.toString());
    });

    gfdProcess.on('close', (code) => {
      console.log(`[GFD] process exited with code ${code}`);
      gfdProcess = null;
      gfdStdoutBuffer = '';
      for (const [, p] of gfdPending) {
        clearTimeout(p.timer);
        p.resolve({ error: 'GFD backend stopped' });
      }
      gfdPending.clear();
    });

    gfdProcess.on('error', (err) => {
      console.error('[GFD] failed to start:', err.message);
      gfdProcess = null;
    });
  } catch (err) {
    console.error('[GFD] spawn error:', err.message);
  }
}

// IPC: send JSON-RPC request to Rust backend
ipcMain.handle('gfd:request', async (_event, { method, params }) => {
  if (!gfdProcess || !gfdProcess.stdin.writable) {
    return { error: 'GFD backend is not running' };
  }

  return new Promise((resolve) => {
    const id = ++gfdRequestId;
    const request = JSON.stringify({ jsonrpc: '2.0', id, method, params });

    // Routed by the central line-buffered stdout reader (see spawnGfdBackend).
    const timer = setTimeout(() => {
      if (gfdPending.has(id)) {
        gfdPending.delete(id);
        resolve({ error: 'Request timed out' });
      }
    }, 30000);
    gfdPending.set(id, { resolve, timer });
    gfdProcess.stdin.write(request + '\n');
  });
});

// IPC: check backend status
ipcMain.handle('gfd:status', async () => {
  return { running: gfdProcess !== null && !gfdProcess.killed };
});

app.whenReady().then(() => {
  createWindow();
  spawnGfdBackend();
});

app.on('window-all-closed', () => {
  if (gfdProcess) {
    gfdProcess.kill();
    gfdProcess = null;
  }
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
