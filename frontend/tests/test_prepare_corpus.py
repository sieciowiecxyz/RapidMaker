"""Focused local-input regressions; no browser, services or real corpus mutation."""
import importlib.util
import json
import struct
import tempfile
import unittest
import zipfile
from pathlib import Path

spec = importlib.util.spec_from_file_location('prepare_corpus', Path(__file__).with_name('prepare-corpus.py'))
corpus = importlib.util.module_from_spec(spec)
spec.loader.exec_module(corpus)


class PrepareCorpus(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'corpus'
        self.root.mkdir()
        self.downloads = Path(self.temp.name) / 'downloads'
        self.downloads.mkdir()
        self.rows = []

    def record(self, ident, filename):
        self.rows.append([f'https://makerworld.com/pl/models/{ident}-fixture', filename, 'Author', 'Observed license'])
        (self.root / 'download-records-01.json').write_text(json.dumps(self.rows))

    def stl(self, name, marker=1):
        (self.downloads / name).write_bytes(bytes([marker]) * 80 + struct.pack('<I', 1) + bytes(50))

    def test_roundtrip_append_and_read_only_check(self):
        self.record('1', 'first.stl')
        self.stl('first.stl')
        corpus.prepare(self.root, self.downloads)
        initial = json.loads((self.root / 'manifest.json').read_text())
        snapshot = {p.relative_to(self.root): (p.read_bytes(), p.stat().st_mtime_ns)
                    for p in self.root.rglob('*') if p.is_file()}
        self.assertEqual(corpus.prepare(self.root, check=True)['cases'], 1)
        self.assertEqual(snapshot, {p.relative_to(self.root): (p.read_bytes(), p.stat().st_mtime_ns)
                                   for p in self.root.rglob('*') if p.is_file()})
        self.record('2', 'archive.zip')
        self.stl('other.stl', 2)
        with zipfile.ZipFile(self.downloads / 'archive.zip', 'w') as archive:
            archive.writestr('nested/other.stl', (self.downloads / 'other.stl').read_bytes())
        self.assertEqual(corpus.prepare(self.root, self.downloads)['cases'], 2)
        self.assertEqual(json.loads((self.root / 'manifest.json').read_text())[0], initial[0])
        self.assertEqual(corpus.prepare(self.root, check=True)['unique_payloads'], 2)
        target = self.root / initial[0]['file']
        target.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'Prepared file differs'):
            corpus.prepare(self.root, check=True)
        self.assertEqual(target.read_bytes(), b'changed')

    def test_direct_3mf_step_and_invalid_model_signature(self):
        self.record('1', 'part.3mf')
        with zipfile.ZipFile(self.downloads / 'part.3mf', 'w') as archive:
            archive.writestr('[Content_Types].xml', '<Types/>')
            archive.writestr('3D/3dmodel.model', '<model/>')
        self.record('2', 'cad.STEP')
        (self.downloads / 'cad.STEP').write_bytes(b'ISO-10303-21;\nEND-ISO-10303-21;')
        result = corpus.prepare(self.root, self.downloads)
        self.assertEqual(result['formats'], {'3mf': 1, 'step': 1})
        self.assertEqual(corpus.prepare(self.root, check=True)['cases'], 2)
        (self.downloads / 'invalid.stl').write_bytes(b'not a model')
        with self.assertRaisesRegex(ValueError, 'Invalid model signature'):
            corpus.model(self.downloads / 'invalid.stl')

    def test_rejects_duplicate_design_payload_and_bad_url(self):
        self.record('1', 'first.stl')
        self.stl('first.stl')
        self.record('1', 'second.stl')
        with self.assertRaisesRegex(ValueError, 'Duplicate design ID'):
            corpus.prepare(self.root, self.downloads)
        self.rows[1][0] = 'https://makerworld.com/pl/models/2-fixture'
        (self.root / 'download-records-01.json').write_text(json.dumps(self.rows))
        self.stl('second.stl')
        with self.assertRaisesRegex(ValueError, 'Duplicate model payload'):
            corpus.prepare(self.root, self.downloads)
        self.rows[0][0] = 'https://makerworld.com.evil.test/pl/models/1-fixture'
        (self.root / 'download-records-01.json').write_text(json.dumps(self.rows))
        with self.assertRaisesRegex(ValueError, 'Expected public MakerWorld'):
            corpus.prepare(self.root, self.downloads)

    def test_rejects_path_escape_symlink_zip_and_bad_signature(self):
        self.record('1', '../escape.stl')
        with self.assertRaisesRegex(ValueError, 'Unsafe download filename'):
            corpus.prepare(self.root, self.downloads)
        self.rows = []
        self.record('1', 'bad.zip')
        with zipfile.ZipFile(self.downloads / 'bad.zip', 'w') as archive:
            archive.writestr('../escape.stl', b'bad')
        with self.assertRaisesRegex(ValueError, 'Unsafe ZIP member'):
            corpus.prepare(self.root, self.downloads)
        self.stl('safe.stl')
        outside = self.root.parent / 'outside'
        outside.mkdir()
        (self.root / 'raw').symlink_to(outside)
        with self.assertRaisesRegex(ValueError, 'Path escapes corpus'):
            corpus.prepare(self.root, self.downloads)
        (self.root / 'raw').unlink()
        (self.downloads / 'bad.zip').write_bytes(b'not a zip')
        with self.assertRaises(zipfile.BadZipFile):
            corpus.prepare(self.root, self.downloads)

    def test_rejects_oversize_and_high_ratio_members_without_expansion(self):
        self.record('1', 'large.zip')
        with zipfile.ZipFile(self.downloads / 'large.zip', 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            archive.writestr('bomb.stl', bytes(1024 * 1024))
        with self.assertRaisesRegex(ValueError, 'No bounded model'):
            corpus.prepare(self.root, self.downloads)
        large = self.downloads / 'large.stl'
        with large.open('wb') as stream:
            stream.truncate(corpus.MAX_BYTES + 1)
        with self.assertRaisesRegex(ValueError, 'File size limit'):
            corpus.read_file(large)


if __name__ == '__main__':
    unittest.main()
