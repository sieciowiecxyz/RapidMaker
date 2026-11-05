import { test, expect } from '@playwright/test';
import { csvCell, orderColors } from '../../src/admin/data';

test('order color labels preserve distinct snapshots and raw swatch codes', () => {
  expect(
    orderColors([
      { color: '10100', colorName: 'Original name' },
      { color: '10100', colorName: 'Renamed color' },
      { color: '10100', colorName: 'Original name' },
      { color: 'legacy-code' },
      { color: 'other-code', colorName: 'Original name' },
    ]),
  ).toEqual([
    { code: '10100', name: 'Original name' },
    { code: '10100', name: 'Renamed color' },
    { code: 'legacy-code', name: 'legacy-code' },
    { code: 'other-code', name: 'Original name' },
  ]);
});

test('CSV neutralizes formulas while preserving quoted customer data', () => {
  for (const value of [
    '=1+1',
    '+SUM(A1)',
    '-2+3',
    '@SUM(A1)',
    '  =1+1',
    '\ufeff=1+1',
    '\t=1+1',
    '\r=1+1',
    '\n=1+1',
    '＝1+1',
    '＋1+1',
    '－1+1',
    '＠SUM(A1)',
  ]) {
    expect(csvCell(value)).toBe('"\t' + value + '"');
  }
  expect(csvCell('Jan "Kowalski"; Firma')).toBe('"Jan ""Kowalski""; Firma"');
  expect(csvCell('Jan\nKowalski')).toBe('"\tJan\nKowalski"');
  expect(csvCell('120.50')).toBe('"120.50"');
});
