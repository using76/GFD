const { app, BrowserWindow, ipcMain } = require('electron');
const path = require('path');
const { spawn } = require('child_process');

let mainWindow = null;
let gfdProcess = null;

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

  // In development, load from Vite dev server; in production, load built files
  if (isDev) {
    mainWindow.loadURL('http://localhost:5173').catch(() => {
      // Fallback to built files if dev server is not running
      mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));
    });
  } else {
    mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

function spawnGfdBackend() {
  // Try gfd-server binary first, then gfd with server subcommand
  const serverBinary = path.join(__dirname, '..', '..', 'target', 'release', 'gfd-server');
  const gfdBinary = path.join(__dirname, '..', '..', 'target', 'release', 'gfd');

  // Check which binary exists
  const fs = require('fs');
  let binary, args;
  if (fs.existsSync(serverBinary + '.exe') || fs.existsSync(serverBinary)) {
    binary = serverBinary;
    args = [];
  } else {
    // GUI works without backend (browser simulation mode)
    console.log('[GFD] No backend binary found. GUI running in simulation mode.');
    console.log('[GFD] Build with: cargo build --release --bin gfd-server');
    return;
  }

  try {
    gfdProcess = spawn(binary, args, {
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    gfdProcess.stdout.on('data', (data) => {
      const lines = data.toString().split('\n').filter((l) => l.trim());
      for (const line of lines) {
        try {
          const msg = JSON.parse(line);
          if (mainWindow && !mainWindow.isDestroyed()) {
            mainWindow.webContents.send('gfd:event', msg);
          }
        } catch {
          // Non-JSON output (log lines, etc.)
          console.log('[GFD]', line);
        }
      }
    });

    gfdProcess.stderr.on('data', (data) => {
      console.error('[GFD stderr]', data.toString());
    });

    gfdProcess.on('close', (code) => {
      console.log(`[GFD] process exited with code ${code}`);
      gfdProcess = null;
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
    const id = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
    const request = JSON.stringify({ jsonrpc: '2.0', id, method, params });

    const onData = (data) => {
      const lines = data.toString().split('\n').filter((l) => l.trim());
      for (const line of lines) {
        try {
          const msg = JSON.parse(line);
          if (msg.id === id) {
            gfdProcess.stdout.off('data', onData);
            resolve(msg.result ?? { error: msg.error });
          }
        } catch {
          // ignore non-JSON
        }
      }
    };

    gfdProcess.stdout.on('data', onData);
    gfdProcess.stdin.write(request + '\n');

    // Timeout after 30 seconds
    setTimeout(() => {
      gfdProcess.stdout.off('data', onData);
      resolve({ error: 'Request timed out' });
    }, 30000);
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
