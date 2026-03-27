const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('gfdAPI', {
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
