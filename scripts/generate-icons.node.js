import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

function clamp01(v) {
  return v < 0 ? 0 : v > 1 ? 1 : v;
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function smoothstep(edge0, edge1, x) {
  const t = clamp01((x - edge0) / (edge1 - edge0));
  return t * t * (3 - 2 * t);
}

function sdRoundedBox(px, py, bx, by, r) {
  const qx = Math.abs(px) - bx + r;
  const qy = Math.abs(py) - by + r;
  const ox = Math.max(qx, 0);
  const oy = Math.max(qy, 0);
  return Math.hypot(ox, oy) + Math.min(Math.max(qx, qy), 0) - r;
}

function sdSegment(px, py, ax, ay, bx, by) {
  const pax = px - ax;
  const pay = py - ay;
  const bax = bx - ax;
  const bay = by - ay;
  const h = clamp01((pax * bax + pay * bay) / (bax * bax + bay * bay));
  return Math.hypot(pax - bax * h, pay - bay * h);
}

function sdTriangle(px, py, p0x, p0y, p1x, p1y, p2x, p2y) {
  const e0x = p1x - p0x, e0y = p1y - p0y;
  const e1x = p2x - p1x, e1y = p2y - p1y;
  const e2x = p0x - p2x, e2y = p0y - p2y;

  const v0x = px - p0x, v0y = py - p0y;
  const v1x = px - p1x, v1y = py - p1y;
  const v2x = px - p2x, v2y = py - p2y;

  const pq0 = clamp01((v0x * e0x + v0y * e0y) / (e0x * e0x + e0y * e0y));
  const pq1 = clamp01((v1x * e1x + v1y * e1y) / (e1x * e1x + e1y * e1y));
  const pq2 = clamp01((v2x * e2x + v2y * e2y) / (e2x * e2x + e2y * e2y));

  const d0x = v0x - e0x * pq0, d0y = v0y - e0y * pq0;
  const d1x = v1x - e1x * pq1, d1y = v1y - e1y * pq1;
  const d2x = v2x - e2x * pq2, d2y = v2y - e2y * pq2;

  const s = Math.sign(e0x * e2y - e0y * e2x);
  const d0 = d0x * d0x + d0y * d0y;
  const d1 = d1x * d1x + d1y * d1y;
  const d2 = d2x * d2x + d2y * d2y;

  const s0 = s * (v0x * e0y - v0y * e0x);
  const s1 = s * (v1x * e1y - v1y * e1x);
  const s2 = s * (v2x * e2y - v2y * e2x);

  const minD = Math.min(d0, d1, d2);
  const minS = Math.min(s0, s1, s2);
  return -Math.sqrt(minD) * Math.sign(minS);
}

function blendOver(bg, fg, alpha) {
  const a = clamp01(alpha);
  return [
    lerp(bg[0], fg[0], a),
    lerp(bg[1], fg[1], a),
    lerp(bg[2], fg[2], a),
    clamp01(bg[3] + a * (1 - bg[3])),
  ];
}

function sampleNeuroClipIcon(nx, ny, aa) {
  // Base rounded squircle (#12131C obsidian with subtle top-left depth)
  const dBox = sdRoundedBox(nx, ny, 0.86, 0.86, 0.24);
  const bgAlpha = 1 - smoothstep(-aa, aa, dBox);
  if (bgAlpha <= 0) return [0, 0, 0, 0];

  const gradT = clamp01((nx + ny + 1.72) / 3.44);
  const baseR = lerp(30, 18, gradT);
  const baseG = lerp(32, 19, gradT);
  const baseB = lerp(48, 28, gradT);

  // Subtle Neuro-pink ambient glow near upper center
  const glowDist = Math.hypot(nx * 0.9, (ny + 0.1) * 1.1);
  const glow = Math.pow(clamp01(1 - glowDist / 0.95), 2.2) * 0.14;
  let col = [
    lerp(baseR, 255, glow),
    lerp(baseG, 182, glow),
    lerp(baseB, 193, glow),
    bgAlpha,
  ];

  // Crisp Neuro-pink inner border ring
  const dBorder = Math.abs(dBox + 0.045) - 0.022;
  const borderAlpha = (1 - smoothstep(-aa, aa, dBorder)) * bgAlpha * 0.92;
  col = blendOver(col, [255, 182, 193, 1], borderAlpha);

  // Top Neuro-ribbon accent wings (stylized twin-bow silhouette)
  const leftWing = sdTriangle(nx, ny, -0.34, -0.50, -0.06, -0.36, -0.26, -0.24) - 0.025;
  const rightWing = sdTriangle(nx, ny, 0.34, -0.50, 0.06, -0.36, 0.26, -0.24) - 0.025;
  const dWings = Math.min(leftWing, rightWing);
  const wingAlpha = (1 - smoothstep(-aa, aa, dWings)) * bgAlpha;
  col = blendOver(col, [255, 143, 171, 1], wingAlpha);

  // Center ribbon knot (Vedal-green core dot with crisp Neuro-pink halo)
  const dKnot = Math.hypot(nx, ny + 0.35) - 0.075;
  const knotAlpha = (1 - smoothstep(-aa, aa, dKnot)) * bgAlpha;
  col = blendOver(col, [43, 217, 153, 1], knotAlpha);

  // Primary Play / Clip triangle in Neuro Pink (#FFB6C1 -> #FFD6DE)
  const dPlay = sdTriangle(nx, ny, -0.22, -0.18, -0.22, 0.30, 0.28, 0.06) - 0.045;
  const playAlpha = (1 - smoothstep(-aa, aa, dPlay)) * bgAlpha;
  const playGrad = clamp01((ny + 0.2) / 0.52);
  const playCol = [
    lerp(255, 255, playGrad),
    lerp(208, 162, playGrad),
    lerp(217, 180, playGrad),
    1,
  ];
  col = blendOver(col, playCol, playAlpha);

  // Bottom subtitle / audio waveform bars (3 bars: Neuro Pink + Vedal Mint + Neuro Pink)
  const dBarLeft = sdSegment(nx, ny, -0.34, 0.52, -0.16, 0.52) - 0.04;
  const dBarMid = sdSegment(nx, ny, -0.04, 0.52, 0.16, 0.52) - 0.04;
  const dBarRight = sdSegment(nx, ny, 0.28, 0.52, 0.36, 0.52) - 0.04;

  col = blendOver(col, [255, 182, 193, 1], (1 - smoothstep(-aa, aa, dBarLeft)) * bgAlpha);
  col = blendOver(col, [43, 217, 153, 1], (1 - smoothstep(-aa, aa, dBarMid)) * bgAlpha);
  col = blendOver(col, [255, 182, 193, 1], (1 - smoothstep(-aa, aa, dBarRight)) * bgAlpha);

  return col;
}

function createNeuroClipPng(width, height) {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const crcTable = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) c = (c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1);
    crcTable[i] = c >>> 0;
  }
  function calcCrc(buf) {
    let c = 0xffffffff;
    for (let i = 0; i < buf.length; i++) c = (crcTable[(c ^ buf[i]) & 0xff] ^ (c >>> 8)) >>> 0;
    return (c ^ 0xffffffff) >>> 0;
  }
  function makeChunk(type, data) {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length, 0);
    const typeBuf = Buffer.from(type, 'ascii');
    const crcBuf = Buffer.alloc(4);
    const crcVal = calcCrc(Buffer.concat([typeBuf, data]));
    crcBuf.writeUInt32BE(crcVal, 0);
    return Buffer.concat([len, typeBuf, data, crcBuf]);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  const rawLineLen = 1 + width * 4;
  const raw = Buffer.alloc(height * rawLineLen);
  const ss = 4; // 4x4 supersampling for clean crisp edges at all sizes
  const aa = 2.0 / Math.max(width, height);

  for (let y = 0; y < height; y++) {
    const rowOffset = y * rawLineLen;
    raw[rowOffset] = 0; // filter type None
    for (let x = 0; x < width; x++) {
      let rAcc = 0, gAcc = 0, bAcc = 0, aAcc = 0;
      for (let sy = 0; sy < ss; sy++) {
        for (let sx = 0; sx < ss; sx++) {
          const px = x + (sx + 0.5) / ss;
          const py = y + (sy + 0.5) / ss;
          const nx = (px / width) * 2 - 1;
          const ny = (py / height) * 2 - 1;
          const [r, g, b, a] = sampleNeuroClipIcon(nx, ny, aa);
          rAcc += r * a;
          gAcc += g * a;
          bAcc += b * a;
          aAcc += a;
        }
      }
      const total = ss * ss;
      const avgA = aAcc / total;
      const pxOffset = rowOffset + 1 + x * 4;
      if (avgA > 0.001) {
        raw[pxOffset] = Math.round(clamp01(rAcc / aAcc / 255) * 255);
        raw[pxOffset + 1] = Math.round(clamp01(gAcc / aAcc / 255) * 255);
        raw[pxOffset + 2] = Math.round(clamp01(bAcc / aAcc / 255) * 255);
        raw[pxOffset + 3] = Math.round(clamp01(avgA) * 255);
      } else {
        raw[pxOffset] = 0;
        raw[pxOffset + 1] = 0;
        raw[pxOffset + 2] = 0;
        raw[pxOffset + 3] = 0;
      }
    }
  }

  return Buffer.concat([
    sig,
    makeChunk('IHDR', ihdr),
    makeChunk('IDAT', zlib.deflateSync(raw, { level: 9 })),
    makeChunk('IEND', Buffer.alloc(0)),
  ]);
}

function createIco(pngEntries) {
  const count = pngEntries.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2); // Type 1 = Icon
  header.writeUInt16LE(count, 4);

  let currentOffset = 6 + count * 16;
  const dirEntries = [];
  for (const { width, height, buf } of pngEntries) {
    const entry = Buffer.alloc(16);
    entry.writeUInt8(width >= 256 ? 0 : width, 0);
    entry.writeUInt8(height >= 256 ? 0 : height, 1);
    entry.writeUInt8(0, 2);
    entry.writeUInt8(0, 3);
    entry.writeUInt16LE(1, 4);
    entry.writeUInt16LE(32, 6);
    entry.writeUInt32LE(buf.length, 8);
    entry.writeUInt32LE(currentOffset, 12);
    dirEntries.push(entry);
    currentOffset += buf.length;
  }
  return Buffer.concat([header, ...dirEntries, ...pngEntries.map((p) => p.buf)]);
}

function createIcns(entries) {
  const chunks = entries.map(({ tag, buf }) => {
    const elemLen = Buffer.alloc(4);
    elemLen.writeUInt32BE(8 + buf.length, 0);
    return Buffer.concat([Buffer.from(tag, 'ascii'), elemLen, buf]);
  });
  const totalPayloadLen = chunks.reduce((sum, c) => sum + c.length, 0);
  const fileLen = Buffer.alloc(4);
  fileLen.writeUInt32BE(8 + totalPayloadLen, 0);
  return Buffer.concat([Buffer.from('icns', 'ascii'), fileLen, ...chunks]);
}

const iconsDir = path.resolve('src-tauri/icons');
if (!fs.existsSync(iconsDir)) fs.mkdirSync(iconsDir, { recursive: true });

const p16 = createNeuroClipPng(16, 16);
const p32 = createNeuroClipPng(32, 32);
const p48 = createNeuroClipPng(48, 48);
const p128 = createNeuroClipPng(128, 128);
const p256 = createNeuroClipPng(256, 256);
const p512 = createNeuroClipPng(512, 512);

fs.writeFileSync(path.join(iconsDir, '32x32.png'), p32);
fs.writeFileSync(path.join(iconsDir, '128x128.png'), p128);
fs.writeFileSync(path.join(iconsDir, '128x128@2x.png'), p256);
fs.writeFileSync(path.join(iconsDir, '512x512.png'), p512);
fs.writeFileSync(
  path.join(iconsDir, 'icon.ico'),
  createIco([
    { width: 16, height: 16, buf: p16 },
    { width: 32, height: 32, buf: p32 },
    { width: 48, height: 48, buf: p48 },
    { width: 128, height: 128, buf: p128 },
    { width: 256, height: 256, buf: p256 },
  ]),
);
fs.writeFileSync(
  path.join(iconsDir, 'icon.icns'),
  createIcns([
    { tag: 'ic07', buf: p128 },
    { tag: 'ic08', buf: p256 },
    { tag: 'ic09', buf: p512 },
  ]),
);

console.log('Successfully generated NeuroClip icons (32x32, 128x128, 256x256, 512x512, multi-res icon.ico, icon.icns) in src-tauri/icons/');
