import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';

function createPng(width, height, r, g, b, a) {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const crcTable = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) c = ((c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1));
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
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  const rawLineLen = 1 + width * 4;
  const raw = Buffer.alloc(height * rawLineLen);
  for (let y = 0; y < height; y++) {
    const rowOffset = y * rawLineLen;
    raw[rowOffset] = 0;
    for (let x = 0; x < width; x++) {
      const pxOffset = rowOffset + 1 + x * 4;
      raw[pxOffset] = r; raw[pxOffset + 1] = g; raw[pxOffset + 2] = b; raw[pxOffset + 3] = a;
    }
  }
  return Buffer.concat([
    sig,
    makeChunk('IHDR', ihdr),
    makeChunk('IDAT', zlib.deflateSync(raw)),
    makeChunk('IEND', Buffer.alloc(0))
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
  return Buffer.concat([header, ...dirEntries, ...pngEntries.map(p => p.buf)]);
}

function createIcns(png128) {
  const tag = Buffer.from('ic07', 'ascii');
  const elemLen = Buffer.alloc(4);
  elemLen.writeUInt32BE(8 + png128.length, 0);
  const elem = Buffer.concat([tag, elemLen, png128]);

  const fileLen = Buffer.alloc(4);
  fileLen.writeUInt32BE(8 + elem.length, 0);
  return Buffer.concat([Buffer.from('icns', 'ascii'), fileLen, elem]);
}

const iconsDir = path.resolve('src-tauri/icons');
if (!fs.existsSync(iconsDir)) fs.mkdirSync(iconsDir, { recursive: true });

// Neuro-sama Pink (#FFB6C1: R=255, G=182, B=193, A=255)
const p32 = createPng(32, 32, 255, 182, 193, 255);
const p128 = createPng(128, 128, 255, 182, 193, 255);
const p256 = createPng(256, 256, 255, 182, 193, 255);

fs.writeFileSync(path.join(iconsDir, '32x32.png'), p32);
fs.writeFileSync(path.join(iconsDir, '128x128.png'), p128);
fs.writeFileSync(path.join(iconsDir, '128x128@2x.png'), p256);
fs.writeFileSync(path.join(iconsDir, 'icon.ico'), createIco([
  { width: 32, height: 32, buf: p32 },
  { width: 256, height: 256, buf: p256 }
]));
fs.writeFileSync(path.join(iconsDir, 'icon.icns'), createIcns(p128));

console.log('Successfully generated all 5 Tauri placeholder icons in src-tauri/icons/');
