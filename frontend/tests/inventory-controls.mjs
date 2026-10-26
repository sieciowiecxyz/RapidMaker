// Template inventory, not a claim that the controls have passed a browser test.
import { parse } from 'svelte/compiler';
import { readdirSync, readFileSync } from 'node:fs';
import { join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../src/', import.meta.url));
function files(path) {
  return readdirSync(path, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? files(join(path, entry.name))
      : entry.name.endsWith('.svelte')
        ? [join(path, entry.name)]
        : [],
  );
}
const rows = [];
for (const file of files(root).sort()) {
  const source = readFileSync(file, 'utf8');
  const ast = parse(source, { modern: true });
  const controls = [];
  function walk(node) {
    if (!node || typeof node !== 'object') return;
    const attrs = node.attributes ?? [];
    if (
      typeof node.name === 'string' &&
      (['button', 'a', 'input', 'select', 'textarea', 'summary', 'Button'].includes(node.name) ||
        attrs.some((a) => /^on(click|change|submit|keydown)$/.test(a.name ?? '')))
    ) {
      const line = source.slice(0, node.start).split('\n').length;
      const opening = source
        .slice(node.start, source.indexOf('>', node.start) + 1)
        .replace(/\s+/g, ' ');
      const text = (node.fragment?.nodes ?? [])
        .map((n) => (n.type === 'Text' ? n.data : ''))
        .join(' ')
        .trim();
      controls.push({
        line,
        kind: node.name,
        description: (text || opening).slice(0, 180).replaceAll('|', '\\|').replaceAll('`', "'"),
      });
    }
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(walk);
      else if (value && typeof value === 'object') walk(value);
    }
  }
  walk(ast.fragment);
  if (controls.length) rows.push({ file: 'frontend/src/' + relative(root, file), controls });
}
console.log('# Inwentaryzacja kontrolek E2E\n');
console.log(
  'Wygenerowano: `node frontend/tests/inventory-controls.mjs`. To mapa źródeł, nie wynik testów. Jedna kontrolka szablonu może tworzyć wiele opcji/wierszy; wartości i pokrycie opisują pliki e2e-*-inventory.md oraz raport. Nie klikamy automatycznie wszystkiego bez sprawdzenia skutków.\n',
);
console.log(
  `Kontrolki szablonów: **${rows.reduce((n, row) => n + row.controls.length, 0)}**, pliki: **${rows.length}**.\n`,
);
for (const { file, controls } of rows) {
  if (file !== rows[0].file) console.log('');
  console.log(
    `## ${file}\n\n| Linia | Typ | Etykieta / początek deklaracji |\n| --- | --- | --- |`,
  );
  for (const c of controls) console.log(`| ${c.line} | ${c.kind} | ${c.description} |`);
}
