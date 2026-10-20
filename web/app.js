// MAGMA Piet spatial front end.
// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque
//
// The grid is the program. Local compilation runs in WebAssembly
// (the Rust library compiled to magma.wasm); the WebSocket channel
// shares the grid and its compiled result with every other open
// page. Conversions (grid <-> Brainfuck) always come from the
// library — via WASM locally, via the server when connected — so
// the snake-path semantics live in exactly one place.

const EXAMPLE_BF = '+++>+++++>--.<.<.';
const state = {
  width: 24,
  height: 12,
  cells: [], // rows of chars, ' ' = void
  selected: '+',
  painting: false,
  seq: -1,
  palette: [],
  wasm: null,
  ws: null,
  wsOpen: false,
  syncTimer: null,
};

const $ = (id) => document.getElementById(id);
const gridEl = $('grid'), bfEl = $('bf'), statsEl = $('stats'),
  verilogEl = $('verilog'), lanesEl = $('lanes'), paletteEl = $('palette');

function blankGrid(w, h) {
  return Array.from({ length: h }, () => Array(w).fill(' '));
}
state.cells = blankGrid(state.width, state.height);

function gridToText() {
  return state.cells.map((row) => row.join('').trimEnd()).join('\n');
}
function gridFromText(text) {
  const rows = text.split('\n');
  const w = Math.max(1, ...rows.map((r) => r.length));
  state.width = w;
  state.height = Math.max(1, rows.length);
  state.cells = rows.map((r) => (r + ' '.repeat(w)).slice(0, w).split(''));
  renderGrid();
}

// ---------- rendering ----------
function colorOf(cmd) {
  const e = state.palette.find((p) => p.command === cmd);
  return e ? e.hex : null;
}
function renderGrid() {
  gridEl.style.gridTemplateColumns = `repeat(${state.width}, 1fr)`;
  gridEl.replaceChildren();
  state.cells.forEach((row, y) => {
    row.forEach((cmd, x) => {
      const d = document.createElement('div');
      d.className = 'cell';
      d.dataset.x = x;
      d.dataset.y = y;
      const hex = cmd !== ' ' ? colorOf(cmd) : null;
      if (hex) d.style.background = hex;
      gridEl.appendChild(d);
    });
  });
}
function renderPalette() {
  paletteEl.replaceChildren();
  state.palette.forEach((entry, i) => {
    const b = document.createElement('button');
    b.className = 'swatch' + (state.selected === entry.command ? ' selected' : '');
    b.style.background = entry.hex;
    b.textContent = `${entry.command} ${entry.name}`;
    b.title = `key ${i + 1}`;
    b.onclick = () => { state.selected = entry.command; renderPalette(); };
    paletteEl.appendChild(b);
  });
  const er = document.createElement('button');
  er.className = 'swatch eraser' + (state.selected === ' ' ? ' selected' : '');
  er.textContent = 'erase';
  er.title = 'key E';
  er.onclick = () => { state.selected = ' '; renderPalette(); };
  paletteEl.appendChild(er);
}
function renderReport(report, bf) {
  bfEl.textContent = bf || '(empty)';
  if (!report) return;
  if (!report.ok) {
    statsEl.innerHTML = `<span class="err">compile error: ${escapeHtml(report.error)}</span>`;
    verilogEl.textContent = '// no netlist';
    lanesEl.replaceChildren();
    return;
  }
  statsEl.textContent =
    `${report.cells} cells · ${report.writes} writes · ${report.taps} output taps · no instruction pointer`;
  verilogEl.textContent = report.verilog;
  renderLanes(report.verilog);
}
function renderLanes(verilog) {
  lanesEl.replaceChildren();
  const cellRe = /assign cell_out_(\d+) = cell_in_\d+ \+ 16'd(\d+);/g;
  const tapRe = /assign print_(\d+) = cell_in_(\d+) \+ 16'd(\d+);/g;
  let m;
  const lanes = [];
  while ((m = cellRe.exec(verilog))) lanes.push({ i: +m[1], raw: +m[2], tap: false });
  while ((m = tapRe.exec(verilog))) lanes.push({ i: +m[2], raw: +m[3], tap: true, n: +m[1] });
  lanes.sort((a, b) => a.i - b.i || (a.tap - b.tap));
  for (const lane of lanes) {
    const signed = lane.raw >= 32768 ? lane.raw - 65536 : lane.raw;
    const row = document.createElement('div');
    row.className = 'lane';
    const pct = Math.min(100, Math.abs(signed) / 32768 * 100);
    row.innerHTML =
      `<span>${lane.tap ? `<span class="tap">print_${lane.n}</span>` : `cell ${lane.i}`}</span>` +
      `<span class="bar"><span class="fill${signed < 0 ? ' neg' : ''}" style="width:${pct}%"></span></span>` +
      `<span>${signed >= 0 ? '+' : ''}${signed}</span>`;
    lanesEl.appendChild(row);
  }
}
function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]));
}

// ---------- painting ----------
function paint(target) {
  const x = +target.dataset.x, y = +target.dataset.y;
  if (state.cells[y][x] === state.selected) return;
  state.cells[y][x] = state.selected;
  const hex = state.selected !== ' ' ? colorOf(state.selected) : null;
  target.style.background = hex || '';
  scheduleSync();
}
gridEl.addEventListener('pointerdown', (e) => {
  if (e.target.classList.contains('cell')) { state.painting = true; paint(e.target); }
});
gridEl.addEventListener('pointerover', (e) => {
  if (state.painting && e.target.classList.contains('cell')) paint(e.target);
});
window.addEventListener('pointerup', () => { state.painting = false; });
window.addEventListener('keydown', (e) => {
  const idx = ['1', '2', '3', '4', '5'].indexOf(e.key);
  if (idx >= 0 && state.palette[idx]) { state.selected = state.palette[idx].command; renderPalette(); }
  else if (e.key === 'e' || e.key === 'E') { state.selected = ' '; renderPalette(); }
  else if (['+', '-', '>', '<', '.'].includes(e.key)) { state.selected = e.key; renderPalette(); }
});

// ---------- engines ----------
function scheduleSync() {
  clearTimeout(state.syncTimer);
  state.syncTimer = setTimeout(syncNow, 120);
}
function syncNow() {
  const text = gridToText();
  if (state.wasm) {
    const bf = state.wasm.gridToBf(text);
    if (bf.ok) renderReport(state.wasm.compile(bf.bf), bf.bf);
  }
  if (state.wsOpen) {
    state.ws.send(JSON.stringify({ type: 'grid', grid: text }));
  }
}

async function initWasm() {
  try {
    const bytes = await (await fetch('magma.wasm')).arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    const ex = instance.exports;
    const mem = () => new Uint8Array(ex.memory.buffer);
    const enc = new TextEncoder(), dec = new TextDecoder();
    const call = (fn, text, extra) => {
      const input = enc.encode(text);
      const ptr = ex.magma_alloc(input.length);
      mem().set(input, ptr);
      const packed = extra === undefined ? fn(ptr, input.length) : fn(ptr, input.length, extra);
      const outPtr = Number(packed >> 32n), outLen = Number(packed & 0xffffffffn);
      const out = dec.decode(mem().subarray(outPtr, outPtr + outLen));
      ex.magma_free(ptr, input.length);
      return JSON.parse(out);
    };
    const palettePacked = ex.magma_palette();
    const palPtr = Number(palettePacked >> 32n), palLen = Number(palettePacked & 0xffffffffn);
    const palette = JSON.parse(dec.decode(mem().subarray(palPtr, palPtr + palLen))).palette;
    state.wasm = {
      palette,
      compile: (bf) => call(ex.magma_compile, bf),
      gridToBf: (text) => call(ex.magma_grid_to_bf, text),
      bfToGrid: (bf, w) => call(ex.magma_bf_to_grid, bf, w),
    };
    $('st-wasm').textContent = 'wasm on';
    $('st-wasm').classList.remove('off');
    if (state.palette.length === 0) { state.palette = palette; renderPalette(); }
    if (!state.wsOpen) loadExampleLocal();
  } catch (err) {
    $('st-wasm').textContent = 'wasm unavailable';
  }
}

function loadExampleLocal() {
  if (!state.wasm) return;
  const g = state.wasm.bfToGrid(EXAMPLE_BF, state.width);
  if (g.ok) {
    gridFromText(g.grid);
    const bf = state.wasm.gridToBf(g.grid);
    if (bf.ok) renderReport(state.wasm.compile(bf.bf), bf.bf);
  }
}

function initWs() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  let ws;
  try {
    ws = new WebSocket(`${proto}//${location.host}/ws`);
  } catch { return; }
  state.ws = ws;
  ws.onopen = () => {
    state.wsOpen = true;
    $('st-ws').textContent = 'websocket live';
    $('st-ws').classList.remove('off');
  };
  ws.onclose = () => {
    state.wsOpen = false;
    $('st-ws').textContent = 'websocket off (wasm only)';
  };
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.type === 'hello') {
      if (state.palette.length === 0) { state.palette = msg.palette; renderPalette(); }
      applyRemote(msg.state);
    } else if (msg.type === 'state') {
      applyRemote(msg.state);
    }
  };
}
function applyRemote(remote) {
  if (!remote || remote.seq <= state.seq) return;
  state.seq = remote.seq;
  if (!state.painting) gridFromText(remote.grid);
  renderReport(remote.report, remote.bf);
}

// ---------- controls ----------
$('btn-clear').onclick = () => {
  state.cells = blankGrid(state.width, state.height);
  renderGrid();
  scheduleSync();
};
$('btn-example').onclick = () => {
  if (state.wsOpen) {
    state.ws.send(JSON.stringify({ type: 'layout', bf: EXAMPLE_BF, width: state.width }));
  } else {
    loadExampleLocal();
  }
};
$('width').onchange = (e) => {
  const w = +e.target.value;
  const bf = bfEl.textContent === '(empty)' ? '' : bfEl.textContent;
  state.width = w;
  if (state.wsOpen) {
    state.ws.send(JSON.stringify({ type: 'layout', bf, width: w }));
  } else if (state.wasm) {
    const g = state.wasm.bfToGrid(bf, w);
    if (g.ok) { gridFromText(g.grid); syncNow(); }
  }
};

renderPalette();
renderGrid();
initWasm();
initWs();
