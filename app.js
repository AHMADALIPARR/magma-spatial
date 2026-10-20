// MAGMA Piet spatial front end — the real engines, no code blocks.
// Copyright (C) 2026 Bel Esprit D'Accord Irrevocable Trust / SnapKitty Collective Limited
// Dual-licensed: AGPL-3.0-only plus SnapKitty Opaque License.
// SPDX-License-Identifier: AGPL-3.0-only AND LicenseRef-SnapKitty-Opaque
//
// Three views of one program, all computed by the actual library:
//   - the Piet grid (left): the program as a picture, painted here;
//   - the Turing machine (right, top): the library's Machine,
//     stepped through the WebAssembly ABI — tape, head, output;
//   - the MAGMA datapath (right, bottom): the combinational lanes
//     the compiler lowers the same program to, drawn as a live
//     diagram whose outputs compute from the input values.
// The WebSocket channel shares the grid/program between browsers.
// Nothing here re-implements the compiler or the machine in JS:
// conversions, compilation and stepping all happen in the WASM
// build of the Rust library.

const EXAMPLE_BF = '+++>+++++>--.<.<.';
const state = {
  width: 24,
  height: 12,
  cells: [],
  selected: '+',
  painting: false,
  seq: -1,
  palette: [],
  wasm: null,
  ws: null,
  wsOpen: false,
  syncTimer: null,
  bf: '',
  report: null,
  machine: null, // { state: <json>, playing: bool, timer: id }
  laneInputs: {}, // datapath cell_in values by cell index
};
state.cells = blankGrid(state.width, state.height);

const $ = (id) => document.getElementById(id);
const gridEl = $('grid'), statsEl = $('stats'), tapeEl = $('tape'),
  mstateEl = $('mstate'), outputEl = $('output'), diagramEl = $('diagram'),
  invalEl = $('inval'), paletteEl = $('palette');

function blankGrid(w, h) {
  return Array.from({ length: h }, () => Array(w).fill(' '));
}
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

// ---------- palette + grid ----------
function colorOf(cmd) {
  const e = state.palette.find((p) => p.command === cmd);
  return e ? e.hex : null;
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

// ---------- program pipeline ----------
function scheduleSync() {
  clearTimeout(state.syncTimer);
  state.syncTimer = setTimeout(syncNow, 120);
}
function syncNow() {
  const text = gridToText();
  if (state.wasm) {
    const bf = state.wasm.gridToBf(text);
    if (bf.ok) setProgram(bf.bf, state.wasm.compile(bf.bf));
  }
  if (state.wsOpen) state.ws.send(JSON.stringify({ type: 'grid', grid: text }));
}
// A new program (local edit, example, or remote state): refresh
// the compile view and reset the machine onto it.
function setProgram(bf, report) {
  state.bf = bf;
  state.report = report;
  renderStats();
  renderDiagram();
  resetMachine();
}
function renderStats() {
  const r = state.report;
  if (!r) { statsEl.textContent = '—'; return; }
  if (!r.ok) {
    statsEl.innerHTML =
      `<span class="err">spatial compile: ${escapeHtml(r.error)}</span>` +
      ` — the Turing machine still runs this program.`;
    return;
  }
  statsEl.textContent =
    `compiled: ${r.cells} lanes · ${r.writes} writes · ${r.taps} print taps · no instruction pointer`;
}
function escapeHtml(s) {
  return s.replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]));
}

// ---------- Turing machine ----------
function resetMachine() {
  stopRun();
  if (!state.wasm) { state.machine = null; renderMachine(); return; }
  state.machine = { state: state.wasm.turingStart(state.bf), playing: false, timer: null };
  renderMachine();
}
function machineHalted() {
  const h = state.machine?.state.halt;
  return h && h !== 'step-limit';
}
function stepMachine(chunk) {
  if (!state.machine || machineHalted()) { stopRun(); return; }
  state.machine.state = state.wasm.turingStep(state.bf, state.machine.state, chunk);
  renderMachine();
  if (machineHalted()) stopRun();
}
function stopRun() {
  if (state.machine?.timer) clearInterval(state.machine.timer);
  if (state.machine) { state.machine.playing = false; state.machine.timer = null; }
  $('btn-run').textContent = 'Run';
}
function renderMachine() {
  const m = state.machine;
  tapeEl.replaceChildren();
  if (!m) { mstateEl.textContent = 'machine needs the wasm engine'; return; }
  const s = m.state;
  s.tape.forEach((value, i) => {
    const d = document.createElement('div');
    d.className = 'tcell' + (i === s.head ? ' head' : '');
    d.innerHTML = `${value}<span class="idx">${i}</span>`;
    tapeEl.appendChild(d);
  });
  const headCell = tapeEl.children[s.head];
  if (headCell) headCell.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  mstateEl.textContent = s.halt && s.halt !== 'step-limit'
    ? `halted (${s.halt}) · ${s.steps} steps`
    : `step ${s.steps} · next: ${s.current ?? '—'} · head at cell ${s.head}`;
  outputEl.replaceChildren();
  if (s.output.length === 0) {
    outputEl.innerHTML = '<span class="hint" style="margin:0">nothing printed yet</span>';
  } else {
    for (const v of s.output) {
      const chip = document.createElement('span');
      chip.className = 'out-chip';
      chip.textContent = v >= 32 && v < 127 ? `${v} '${String.fromCharCode(v)}'` : `${v}`;
      outputEl.appendChild(chip);
    }
  }
  $('btn-step').disabled = !!machineHalted();
}
$('btn-step').onclick = () => stepMachine(1);
$('btn-reset').onclick = () => resetMachine();
$('btn-run').onclick = () => {
  if (!state.machine) return;
  if (state.machine.playing) { stopRun(); return; }
  const speeds = { slow: [1, 140], medium: [10, 60], fast: [250, 40] };
  const [chunk, ms] = speeds[$('speed').value] || speeds.medium;
  state.machine.playing = true;
  $('btn-run').textContent = 'Pause';
  state.machine.timer = setInterval(() => stepMachine(chunk), ms);
};
$('btn-input').onclick = () => {
  if (!state.machine) return;
  const v = Math.max(0, Math.min(65535, +$('input-val').value || 0));
  state.machine.state.input.push(v);
  renderMachine();
};

// ---------- MAGMA datapath diagram ----------
function parseLanes(verilog) {
  const lanes = [];
  const taps = [];
  let m;
  const cellRe = /assign cell_out_(\d+) = cell_in_(\d+) \+ 16'd(\d+);/g;
  while ((m = cellRe.exec(verilog))) lanes.push({ cell: +m[1], raw: +m[3] });
  const tapRe = /assign print_(\d+) = cell_in_(\d+) \+ 16'd(\d+);/g;
  while ((m = tapRe.exec(verilog))) taps.push({ n: +m[1], cell: +m[2], raw: +m[3] });
  return { lanes, taps };
}
const signed = (raw) => (raw >= 32768 ? raw - 65536 : raw);
function renderDiagram() {
  const svg = diagramEl;
  const NS = 'http://www.w3.org/2000/svg';
  svg.replaceChildren();
  invalEl.replaceChildren();
  const el = (tag, attrs) => {
    const e = document.createElementNS(NS, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    svg.appendChild(e);
    return e;
  };
  const text = (x, y, str, cls, size) => {
    const t = el('text', { x, y, ...(cls ? { class: cls } : {}), ...(size ? { 'font-size': size } : {}) });
    t.textContent = str;
    return t;
  };
  const r = state.report;
  if (!r || !r.ok) {
    svg.setAttribute('viewBox', '0 0 640 44');
    text(8, 26, r ? 'the spatial stage cannot compile this program yet (see left panel)' : 'no compiled program', 'lbl', 12);
    return;
  }
  const { lanes, taps } = parseLanes(r.verilog);
  const rowH = 56, tapH = 24, W = 640;
  let height = 16;
  const layout = lanes.map((lane) => {
    const myTaps = taps.filter((t) => t.cell === lane.cell);
    const y = height;
    height += rowH + myTaps.length * tapH;
    return { lane, myTaps, y };
  });
  svg.setAttribute('viewBox', `0 0 ${W} ${height + 8}`);
  for (const { lane, myTaps, y } of layout) {
    const inVal = state.laneInputs[lane.cell] ?? 0;
    const outVal = (inVal + lane.raw) & 0xffff;
    const midY = y + 22;
    // wires with animated flow overlay
    el('line', { x1: 128, y1: midY, x2: 252, y2: midY, class: 'wire' });
    el('line', { x1: 128, y1: midY, x2: 252, y2: midY, class: 'flow' });
    el('line', { x1: 348, y1: midY, x2: 472, y2: midY, class: 'wire' });
    el('line', { x1: 348, y1: midY, x2: 472, y2: midY, class: 'flow' });
    // input + output value nodes
    el('rect', { x: 16, y: midY - 15, width: 112, height: 30, rx: 5, class: 'node-in' });
    text(26, midY - 20, `cell ${lane.cell} in`, 'lbl');
    text(72, midY + 5, `${inVal}`, null, 14).setAttribute('text-anchor', 'middle');
    el('rect', { x: 472, y: midY - 15, width: 112, height: 30, rx: 5, class: 'node-out' });
    text(482, midY - 20, `cell ${lane.cell} out`, 'lbl');
    text(528, midY + 5, `${outVal}`, null, 14).setAttribute('text-anchor', 'middle');
    // adder block
    el('rect', { x: 252, y: midY - 15, width: 96, height: 30, rx: 5, class: 'adder' });
    const d = signed(lane.raw);
    text(300, midY + 5, d >= 0 ? `+ ${d}` : `− ${-d}`, null, 14).setAttribute('text-anchor', 'middle');
    // print taps branch off the input wire
    myTaps.forEach((tap, i) => {
      const ty = y + rowH + i * tapH + 8;
      el('path', { d: `M 190 ${midY} L 190 ${ty + 7} L 252 ${ty + 7}`, class: 'wire' });
      el('rect', { x: 252, y: ty - 3, width: 150, height: 21, rx: 5, class: 'tapbox' });
      const tapVal = (inVal + tap.raw) & 0xffff;
      text(262, ty + 11, `print_${tap.n} = ${tapVal}`, null, 12);
    });
    // input editor
    const label = document.createElement('label');
    label.textContent = `cell ${lane.cell} in`;
    const input = document.createElement('input');
    input.type = 'number';
    input.min = '0';
    input.max = '65535';
    input.value = inVal;
    input.onchange = () => {
      state.laneInputs[lane.cell] = Math.max(0, Math.min(65535, +input.value || 0));
      renderDiagram();
    };
    label.appendChild(input);
    invalEl.appendChild(label);
  }
}
$('btn-download').onclick = () => {
  if (!state.report?.ok) return;
  const blob = new Blob([state.report.verilog], { type: 'text/plain' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'magma_spatial.v';
  a.click();
  URL.revokeObjectURL(a.href);
};

// ---------- engines ----------
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
    const call2 = (fn, textA, textB, n) => {
      const a = enc.encode(textA), b = enc.encode(textB);
      const pa = ex.magma_alloc(a.length), pb = ex.magma_alloc(b.length);
      mem().set(a, pa); mem().set(b, pb);
      const packed = fn(pa, a.length, pb, b.length, n);
      const outPtr = Number(packed >> 32n), outLen = Number(packed & 0xffffffffn);
      const out = dec.decode(mem().subarray(outPtr, outPtr + outLen));
      ex.magma_free(pa, a.length); ex.magma_free(pb, b.length);
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
      turingStart: (bf) => call(ex.magma_turing_start, bf),
      turingStep: (bf, st, n) => call2(ex.magma_turing_step, bf, JSON.stringify(st), n),
    };
    $('st-wasm').textContent = 'wasm on';
    $('st-wasm').classList.remove('off');
    if (state.palette.length === 0) { state.palette = palette; renderPalette(); }
    if (!state.wsOpen) loadExampleLocal();
    else syncNow();
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
    if (bf.ok) setProgram(bf.bf, state.wasm.compile(bf.bf));
  }
}
function initWs() {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  let ws;
  try { ws = new WebSocket(`${proto}//${location.host}/ws`); } catch { return; }
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
  setProgram(remote.bf, remote.report);
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
  state.width = w;
  if (state.wsOpen) {
    state.ws.send(JSON.stringify({ type: 'layout', bf: state.bf, width: w }));
  } else if (state.wasm) {
    const g = state.wasm.bfToGrid(state.bf, w);
    if (g.ok) { gridFromText(g.grid); syncNow(); }
  }
};

// Demo hook: opening the page with #autorun starts the machine
// running as soon as it exists (used for screenshots and kiosks).
if (location.hash.includes('autorun')) {
  const iv = setInterval(() => {
    if (state.machine && !state.machine.playing) {
      $('btn-run').click();
      clearInterval(iv);
    }
  }, 200);
}

renderPalette();
renderGrid();
renderDiagram();
renderMachine();
initWasm();
initWs();
