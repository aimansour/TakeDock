import { execFileSync } from 'node:child_process';
import {
  readFileSync,
  writeFileSync,
  readdirSync,
  existsSync,
  mkdirSync,
  realpathSync,
} from 'node:fs';
import { dirname, join } from 'node:path';
const metadata = JSON.parse(
  execFileSync(
    'cargo',
    [
      'metadata',
      '--locked',
      '--format-version',
      '1',
      '--features',
      'desktop',
      '--filter-platform',
      'x86_64-pc-windows-msvc',
    ],
    { encoding: 'utf8', maxBuffer: 20 * 1024 * 1024 },
  ),
);
const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
const included = new Set();
function include(id) {
  if (included.has(id)) return;
  included.add(id);
  for (const dep of nodes.get(id)?.deps ?? [])
    if (dep.dep_kinds.some((kind) => kind.kind !== 'dev')) include(dep.pkg);
}
for (const p of metadata.packages)
  if (['takedock', 'takedock-files'].includes(p.name)) include(p.id);
const supplemental = existsSync('third-party/supplemental.json')
  ? JSON.parse(readFileSync('third-party/supplemental.json', 'utf8'))
  : {};
const entries = [];
const texts = [];
function licenses(folder, depth = 0) {
  const found = [];
  for (const file of readdirSync(folder, { withFileTypes: true })) {
    const path = join(folder, file.name);
    if (
      file.isFile() &&
      /^(licen[cs]e|copying|notice|copyright)([._-]|$)/i.test(file.name)
    )
      found.push(path);
    else if (file.isDirectory() && depth < 1 && /^licenses?$/i.test(file.name))
      found.push(...licenses(path, depth + 1));
  }
  return found.sort();
}
function record(name, version, license, source, folder) {
  if (!license) throw Error(`Missing license: ${name}`);
  entries.push({ name, version, license, source });
  const files = licenses(folder);
  const extra = supplemental[`${name}@${version}`];
  if (!files.length && !extra?.text)
    throw Error(`Missing license text: ${name}@${version}`);
  texts.push(
    `\n${name} ${version}\nLicense: ${license}\nSource: ${source}\n` +
      (files.length
        ? files.map((path) => readFileSync(path, 'utf8').trim()).join('\n\n')
        : `Upstream license sources: ${extra.sources.join(', ')}\n${extra.text}`),
  );
}
for (const p of metadata.packages.sort((a, b) =>
  `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`),
)) {
  if (included.has(p.id) && p.source?.startsWith('registry+'))
    record(
      p.name,
      p.version,
      p.license,
      `https://crates.io/api/v1/crates/${p.name}/${p.version}/download`,
      dirname(p.manifest_path),
    );
}
const project = JSON.parse(readFileSync('package.json', 'utf8'));
const visited = new Set();
function npmPackage(name, parent = process.cwd()) {
  let base = parent,
    folder;
  while (true) {
    folder = join(base, 'node_modules', name);
    if (existsSync(join(folder, 'package.json'))) break;
    const next = dirname(base);
    if (next === base) throw Error(`Missing installed package ${name}`);
    base = next;
  }
  const canonical = realpathSync(folder);
  if (visited.has(canonical)) return;
  visited.add(canonical);
  const p = JSON.parse(readFileSync(join(folder, 'package.json'), 'utf8'));
  record(
    p.name,
    p.version,
    p.license,
    `https://registry.npmjs.org/${p.name}/-/${p.name.split('/').at(-1)}-${p.version}.tgz`,
    folder,
  );
  for (const dependency of Object.keys(p.dependencies ?? {}).sort())
    npmPackage(dependency, folder);
}
for (const name of Object.keys(project.dependencies).sort()) npmPackage(name);
texts.push(readFileSync('third-party/rust-standard-library.txt', 'utf8'));
mkdirSync('third-party', { recursive: true });
writeFileSync(
  'third-party/inventory.json',
  JSON.stringify(entries, null, 2) + '\n',
);
writeFileSync(
  'third-party/licenses.txt',
  'TakeDock third-party notices\n\nLocked Windows Rust runtime/build dependencies and Android file-helper roots, plus production JavaScript dependencies. Build-only entries may not be shipped. MPL source archives linked below are unmodified and remain under MPL-2.0.\n' +
    texts.join('\n') +
    '\n',
);
console.log(
  `Generated ${entries.length} dependency notices; all declared licenses and license texts present.`,
);
