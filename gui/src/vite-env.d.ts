/// <reference types="vite/client" />

export {};

interface GfdAPI {
  sendRequest: (method: string, params?: Record<string, unknown>) => Promise<unknown>;
  getStatus: () => Promise<{ running: boolean }>;
  onEvent: (callback: (data: unknown) => void) => () => void;
}

declare global {
  interface Window {
    gfdAPI?: GfdAPI;
  }
}
