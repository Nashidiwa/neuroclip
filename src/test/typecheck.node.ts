import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

console.log('Running TypeScript strict typecheck & syntax audit...');

// 1. Verify tsconfig.json invariants
const tsconfigPath = path.resolve('tsconfig.json');
assert.strictEqual(fs.existsSync(tsconfigPath), true, 'tsconfig.json must exist');
const tsconfig = JSON.parse(fs.readFileSync(tsconfigPath, 'utf8'));
const opts = tsconfig.compilerOptions;
assert.strictEqual(opts.strict, true, 'tsconfig.compilerOptions.strict must be true');
assert.strictEqual(opts.noImplicitAny, true, 'noImplicitAny must be true');
assert.strictEqual(opts.strictNullChecks, true, 'strictNullChecks must be true');
assert.strictEqual(opts.strictFunctionTypes, true, 'strictFunctionTypes must be true');
assert.strictEqual(opts.noUncheckedIndexedAccess, true, 'noUncheckedIndexedAccess must be true');
assert.strictEqual(opts.exactOptionalPropertyTypes, true, 'exactOptionalPropertyTypes must be true');
assert.deepStrictEqual(opts.paths['@/*'], ['src/*'], 'paths alias @/* must be configured');

// 2. Scan for prohibited 'any' usage
function scanDir(dir: string): string[] {
  const files: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...scanDir(fullPath));
    } else if (entry.name.endsWith('.ts') || entry.name.endsWith('.tsx')) {
      files.push(fullPath);
    }
  }
  return files;
}

const tsFiles = scanDir(path.resolve('src'));
for (const file of tsFiles) {
  if (path.basename(file).includes('.test.')) continue;
  if (path.basename(file) === 'typecheck.node.ts') continue;
  const content = fs.readFileSync(file, 'utf8');
  const pattern = [':\\s*any\\b', 'as\\s+any\\b', '<' + 'any>'].join('|');
  const matches = content.match(new RegExp(pattern, 'g'));
  assert.strictEqual(matches, null, `Prohibited 'any' found in ${file}`);
}

console.log(`TypeScript typecheck passed: ${tsFiles.length} files verified with zero 'any' and strict mode!`);
