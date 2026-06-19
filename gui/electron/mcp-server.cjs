#!/usr/bin/env node
/*
 * GFD MCP stdio server — lets an external MCP client (e.g. the Claude Code CLI)
 * drive the RUNNING GFD GUI's command-core, identical to the in-app chat.
 *
 *   Claude Code ──MCP(stdio JSON-RPC)──▶ this script ──ws──▶ Electron main (8765)
 *                                                              ──IPC──▶ renderer
 *                                                                      createMcpBridge(core)
 *
 * No MCP SDK dependency: MCP's stdio transport is newline-delimited JSON-RPC 2.0,
 * implemented directly here. stdout is the protocol channel — logs go to stderr.
 * Requires the GFD GUI (Electron) to be running; tools appear once it connects.
 */
'use strict';
const WebSocket = require('ws');

const WS_URL = process.env.GFD_MCP_WS || 'ws://127.0.0.1:8765';
const SERVER_INFO = { name: 'gfd-workbench', version: '0.1.0' };

let ws = null;
let wsReady = false;
let seq = 1;
const pending = new Map(); // control id -> { resolve, reject, timer }

function log(...a) { process.stderr.write('[gfd-mcp] ' + a.join(' ') + '\n'); }

function connect() {
  ws = new WebSocket(WS_URL);
  ws.on('open', () => { wsReady = true; log('connected to GUI', WS_URL); notify('notifications/tools/list_changed'); });
  ws.on('message', (data) => {
    let msg;
    try { msg = JSON.parse(data.toString()); } catch { return; }
    const p = pending.get(msg.id);
    if (p) { clearTimeout(p.timer); pending.delete(msg.id); p.resolve(msg.result); }
  });
  ws.on('close', () => { wsReady = false; setTimeout(connect, 1500); });
  ws.on('error', (e) => log('ws error:', e.message));
}

/** Send a control request to the GUI and await its reply. */
function control(op, name, args) {
  return new Promise((resolve, reject) => {
    if (!wsReady) { reject(new Error('GFD GUI not connected — open the GFD app')); return; }
    const id = seq++;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error('GUI request timed out')); }, 30000);
    pending.set(id, { resolve, reject, timer });
    ws.send(JSON.stringify({ id, op, name, args }));
  });
}

// ---- MCP stdio: newline-delimited JSON-RPC 2.0 ----
function send(msg) { process.stdout.write(JSON.stringify(msg) + '\n'); }
function reply(id, result) { send({ jsonrpc: '2.0', id, result }); }
function replyErr(id, code, message) { send({ jsonrpc: '2.0', id, error: { code, message } }); }
function notify(method, params) { send({ jsonrpc: '2.0', method, params }); }

async function handle(req) {
  const { id, method, params } = req;
  switch (method) {
    case 'initialize':
      reply(id, {
        protocolVersion: (params && params.protocolVersion) || '2024-11-05',
        capabilities: { tools: { listChanged: true } },
        serverInfo: SERVER_INFO,
      });
      return;
    case 'ping':
      reply(id, {});
      return;
    case 'tools/list': {
      try {
        const r = await control('listTools');
        const tools = (r && Array.isArray(r.tools) ? r.tools : []).map((t) => ({
          name: t.name,
          description: t.description || t.name,
          inputSchema: t.inputSchema || { type: 'object', properties: {} },
        }));
        reply(id, { tools });
      } catch {
        // GUI not up yet → empty; a tools/list_changed fires when it connects.
        reply(id, { tools: [] });
      }
      return;
    }
    case 'tools/call': {
      const name = params && params.name;
      const args = (params && params.arguments) || {};
      try {
        const r = await control('callTool', name, args);
        const isError = r && r.ok === false;
        const payload = r && Object.prototype.hasOwnProperty.call(r, 'content') ? r.content : r;
        const text = (isError ? 'ERROR: ' + ((r && r.error) || 'failed') + '\n' : '') + JSON.stringify(payload, null, 2);
        reply(id, { content: [{ type: 'text', text }], isError: !!isError });
      } catch (e) {
        reply(id, { content: [{ type: 'text', text: 'ERROR: ' + e.message }], isError: true });
      }
      return;
    }
    default:
      if (method && method.startsWith('notifications/')) return; // client notifications: ignore
      if (id !== undefined) replyErr(id, -32601, 'method not found: ' + method);
  }
}

let buf = '';
process.stdin.on('data', (chunk) => {
  buf += chunk.toString();
  let nl;
  while ((nl = buf.indexOf('\n')) >= 0) {
    const line = buf.slice(0, nl).trim();
    buf = buf.slice(nl + 1);
    if (!line) continue;
    let req;
    try { req = JSON.parse(line); } catch { continue; }
    handle(req).catch((e) => log('handle error:', e.message));
  }
});
process.stdin.on('end', () => process.exit(0));

connect();
log('started; bridging MCP stdio ↔', WS_URL);
