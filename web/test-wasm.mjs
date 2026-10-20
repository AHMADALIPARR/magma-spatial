// MAGMA WASM check: instantiate web/magma.wasm in Node and verify
// its compile output against the natively compiled golden file.
// Usage: node web/test-wasm.mjs [path-to-golden-parallel.v]
import { readFileSync } from 'node:fs';

const wasmPath = new URL('./magma.wasm', import.meta.url);
const goldenPath = process.argv[2];
const bytes = readFileSync(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, {});
const ex = instance.exports;
const mem = () => new Uint8Array(ex.memory.buffer);
const enc = new TextEncoder();
const dec = new TextDecoder();

function callString(fn, text, extra) {
  const input = enc.encode(text);
  const ptr = ex.magma_alloc(input.length);
  mem().set(input, ptr);
  const packed = extra === undefined ? fn(ptr, input.length) : fn(ptr, input.length, extra);
  const outPtr = Number(packed >> 32n);
  const outLen = Number(packed & 0xffffffffn);
  const out = dec.decode(mem().subarray(outPtr, outPtr + outLen));
  ex.magma_free(ptr, input.length);
  return JSON.parse(out);
}

let failures = 0;
function check(name, cond, detail) {
  if (cond) {
    console.log(`ok: ${name}`);
  } else {
    failures += 1;
    console.error(`FAIL: ${name}${detail ? ` — ${detail}` : ''}`);
  }
}

// 1. Compile the shipped example; compare with the native golden.
const report = callString(ex.magma_compile, '+++>+++++>--.<.<.');
check('compile ok', report.ok === true, JSON.stringify(report));
check('stats', report.cells === 3 && report.writes === 10 && report.taps === 3,
  `cells=${report.cells} writes=${report.writes} taps=${report.taps}`);
if (goldenPath) {
  const golden = readFileSync(goldenPath, 'utf8');
  check('verilog matches native golden byte-for-byte', report.verilog === golden);
}

// 2. Compile errors surface as JSON, not traps.
const bad = callString(ex.magma_compile, '++[>+<-]');
check('loop rejected in wasm', bad.ok === false && /dynamic loop at character 2/.test(bad.error), JSON.stringify(bad));

// 3. Grid -> BF over the snake path.
const bf = callString(ex.magma_grid_to_bf, '++>\n<- ');
check('grid_to_bf', bf.ok === true && bf.bf === '++>-<', JSON.stringify(bf));

// 4. BF -> grid layout roundtrips.
const grid = callString(ex.magma_bf_to_grid, '+++>+++++>--.<.<.', 6);
check('bf_to_grid', grid.ok === true, JSON.stringify(grid));
const round = callString(ex.magma_grid_to_bf, grid.grid);
check('grid roundtrip', round.bf === '+++>+++++>--.<.<.', JSON.stringify(round));

// 5. Palette is served from the library table.
const palPacked = ex.magma_palette();
const palPtr = Number(palPacked >> 32n);
const palLen = Number(palPacked & 0xffffffffn);
const pal = JSON.parse(dec.decode(mem().subarray(palPtr, palPtr + palLen)));
check('palette has 5 entries', Array.isArray(pal.palette) && pal.palette.length === 5);
check('palette maps + to red', pal.palette[0].command === '+' && pal.palette[0].hex === '#E53935');

if (failures > 0) {
  console.error(`${failures} check(s) failed`);
  process.exit(1);
}
console.log('PASS: wasm compile, grid conversion, palette');
