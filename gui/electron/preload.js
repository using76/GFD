const { contextBridge, ipcRenderer, webUtils } = require('electron');

contextBridge.exposeInMainWorld('gfdAPI', {
  /**
   * Resolve the absolute filesystem path of a File (from an <input>/drop).
   * Electron 32+ removed File.path; webUtils.getPathForFile is the replacement.
   * @param {File} file
   * @returns {string} absolute path ('' if unavailable)
   */
  getPathForFile: (file) => {
    try {
      return webUtils ? webUtils.getPathForFile(file) : '';
    } catch {
      return '';
    }
  },

  /**
   * Send a JSON-RPC request to the GFD Rust backend.
   * @param {string} method - RPC method name
   * @param {object} params - RPC parameters
   * @returns {Promise<any>} result or error
   */
  sendRequest: (method, params) => {
    return ipcRenderer.invoke('gfd:request', { method, params });
  },

  /**
   * Check backend status.
   * @returns {Promise<{running: boolean}>}
   */
  getStatus: () => {
    return ipcRenderer.invoke('gfd:status');
  },

  /**
   * Subscribe to events from the GFD backend (solver progress, etc.).
   * @param {function} callback - receives event data
   * @returns {function} unsubscribe function
   */
  onEvent: (callback) => {
    const handler = (_event, data) => callback(data);
    ipcRenderer.on('gfd:event', handler);
    return () => {
      ipcRenderer.removeListener('gfd:event', handler);
    };
  },
});

/**
 * MCP control channel — lets an external MCP client (e.g. Claude Code CLI),
 * relayed by the Electron main WebSocket server, drive this running GUI's
 * command-core (the same surface the in-app chat uses).
 */
contextBridge.exposeInMainWorld('gfdMcp', {
  /** Register the renderer's responder. cb receives {id, op, name?, args?}. */
  onControl: (callback) => {
    const handler = (_event, msg) => callback(msg);
    ipcRenderer.on('mcp:control', handler);
    return () => ipcRenderer.removeListener('mcp:control', handler);
  },
  /** Reply to a control request by id. */
  sendResult: (id, result) => ipcRenderer.send('mcp:result', { id, result }),
});
