#!/usr/bin/env node
/**
 * CDP-based E2E test for RH850 simulator.
 * Usage: node cdp_test.js <cdp_port> <vite_port>
 */
const http = require('http');
const fs = require('fs');

const CDP_PORT = parseInt(process.argv[2] || '9224');
const VITE_PORT = parseInt(process.argv[3] || '4526');
const path = require('path');
const ROM_PATH = path.join(__dirname, 'test.rom');

function httpGet(url) {
  return new Promise((resolve, reject) => {
    http.get(url, (res) => {
      let data = '';
      res.on('data', d => data += d);
      res.on('end', () => resolve(data));
    }).on('error', reject);
  });
}

async function main() {
  // Wait for CDP endpoint
  let pageWs = null;
  for (let i = 0; i < 10; i++) {
    try {
      const data = await httpGet(`http://127.0.0.1:${CDP_PORT}/json`);
      const tabs = JSON.parse(data);
      const page = tabs.find(t => t.type === 'page');
      if (page) { pageWs = page.webSocketDebuggerUrl; break; }
    } catch (e) {}
    await new Promise(r => setTimeout(r, 500));
  }
  if (!pageWs) { console.error('No CDP page found'); process.exit(1); }
  console.log('Connected to:', pageWs);

  const ws = new WebSocket(pageWs);
  let id = 0;
  const pending = new Map();

  function send(method, params) {
    return new Promise((resolve, reject) => {
      const mid = ++id;
      pending.set(mid, { resolve, reject });
      ws.send(JSON.stringify({ id: mid, method, params: params || {} }));
    });
  }

  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data);
    if (msg.id && pending.has(msg.id)) {
      const p = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) p.reject(new Error(JSON.stringify(msg.error)));
      else p.resolve(msg.result);
    }
  };

  ws.onopen = async () => {
    try {
      await send('Runtime.enable', {});
      await send('DOM.enable', {});
      await new Promise(r => setTimeout(r, 2000));

      // Navigate to Vite app
      await send('Page.navigate', { url: `http://127.0.0.1:${VITE_PORT}/` });
      await new Promise(r => setTimeout(r, 3000));

      // Check sim
      const check = await send('Runtime.evaluate', {
        expression: `(() => {
          const a = document.querySelector('#app');
          if (!a || !a.__vue_app__) return 'no_vue';
          const s = a.__vue_app__._instance.setupState.sim;
          if (!s) return 'no_sim';
          const r = s.sim.value;
          if (!r) return 'no_raw';
          return 'ok_pc=' + r.getPc();
        })()`
      });
      console.log('Sim:', check.result.value);

      // Load ROM
      const doc = await send('DOM.getDocument', {});
      const fi = await send('DOM.querySelector', {
        nodeId: doc.root.nodeId,
        selector: 'input[type=file]'
      });
      if (fi && fi.nodeId) {
        await send('DOM.setFileInputFiles', {
          nodeId: fi.nodeId,
          files: [ROM_PATH]
        });
        console.log('ROM loaded');
        await new Promise(r => setTimeout(r, 500));
      } else {
        console.error('No file input');
        ws.close(); process.exit(1);
      }

      // Run simulation
      const run = await send('Runtime.evaluate', {
        expression: `(() => {
          const sim = document.querySelector('#app').__vue_app__._instance.setupState.sim.sim.value;
          const trace = [];
          for (let i = 0; i < 5000; i++) {
            const pc = sim.getPc();
            const stopped = sim.stepOnce();
            if (i < 5 || i % 1000 === 0 || stopped) trace.push({i, pc, s: stopped});
            if (stopped) break;
          }
          return JSON.stringify({
            trace,
            finalPc: sim.getPc(),
            ins: sim.getInsCount(),
            stop: sim.getStopReason(),
            hal: sim.isHalted(),
            gc: sim.getMemWord(0x1000),
            gr: sim.getMemWord(0x1004),
            gt: sim.getMemWord(0x1008),
            gg: sim.getMemWord(0x100C),
            ps: sim.getMemWord(0x1010),
          });
        })()`
      });
      console.log('Result:', run.result.value);

    } catch (err) {
      console.error('Error:', err.message);
    }
    ws.close();
    process.exit(0);
  };

  ws.onerror = (e) => {
    console.error('WS error:', e.message || e);
    process.exit(1);
  };

  setTimeout(() => {
    console.error('TIMEOUT');
    process.exit(1);
  }, 20000);
}

main().catch(e => { console.error(e); process.exit(1); });
