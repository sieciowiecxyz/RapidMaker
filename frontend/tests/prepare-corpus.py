#!/usr/bin/env python3
"""Prepare private browser downloads for corpus.spec.ts; no network or license grants."""
import argparse
import hashlib
import io
import json
import re
import stat
import struct
import zipfile
from pathlib import Path, PurePosixPath
from urllib.parse import urlsplit

MAX_BYTES = 128 * 1024 * 1024
MAX_ENTRIES = 4096
NAMES = [
    'część próbna', 'model 🧩', '零件', 'part #1 + 50%',
    'nazwa (wersja 2) [final]', 'model..final', 'CON', 'cafe\u0301',
    'bardzo długa nazwa ' + 'element-' * 20, 'model "cytat"', 'część; test', 'مسند',
]
FORMATS = {'.stl', '.step', '.stp', '.3mf'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def inside(root, relative):
    path = PurePosixPath(relative)
    require(not path.is_absolute() and path.parts and all(p not in ('.', '..') for p in path.parts)
            and '\\' not in relative and '\x00' not in relative, 'Unsafe relative path')
    target = root.joinpath(*path.parts)
    require(target.resolve().is_relative_to(root), 'Path escapes corpus')
    require(not any(p.is_symlink() for p in [target, *target.parents] if p != root and p.is_relative_to(root)),
            'Symlink inside corpus is not allowed')
    return target


def read_file(path, limit=MAX_BYTES):
    require(path.is_file() and not path.is_symlink(), 'Expected regular file: ' + str(path))
    require(0 < path.stat().st_size <= limit, 'File size limit: ' + str(path))
    with path.open('rb') as source:
        data = source.read(limit + 1)
    require(len(data) <= limit, 'File grew beyond limit')
    return data


def entries(archive):
    items = archive.infolist()
    require(len(items) <= MAX_ENTRIES, 'Too many ZIP entries')
    require(len({x.filename for x in items}) == len(items), 'Duplicate ZIP member')
    for item in items:
        name = PurePosixPath(item.filename)
        require(not name.is_absolute() and '..' not in name.parts and '\\' not in item.filename
                and '\x00' not in item.filename and not stat.S_ISLNK(item.external_attr >> 16),
                'Unsafe ZIP member')
        require(not item.flag_bits & 1, 'Encrypted ZIP is unsupported')
    return items


def model(source):
    raw = read_file(source)
    member = None
    ext = source.suffix.lower()
    payload = raw
    if ext == '.zip':
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            choices = [x for x in entries(archive) if not x.is_dir()
                       and Path(x.filename).suffix.lower() in FORMATS
                       and 0 < x.file_size <= MAX_BYTES
                       and x.file_size / max(x.compress_size, 1) < 1000]
            require(choices, 'No bounded model in ZIP')
            chosen = max(choices, key=lambda x: x.file_size)
            member, ext = chosen.filename, Path(chosen.filename).suffix.lower()
            with archive.open(chosen) as stream:
                payload = stream.read(MAX_BYTES + 1)
            require(len(payload) == chosen.file_size <= MAX_BYTES, 'ZIP member size mismatch')
    require(ext in FORMATS, 'Unsupported source format')
    if ext == '.3mf':
        with zipfile.ZipFile(io.BytesIO(payload)) as archive:
            names = [x.filename for x in entries(archive)]
            valid = '[Content_Types].xml' in names and any(n.lower().endswith('.model') for n in names)
    elif ext == '.stl':
        valid = (len(payload) >= 84 and len(payload) == 84 + 50 * struct.unpack_from('<I', payload, 80)[0]) or (
            payload.lstrip().lower().startswith(b'solid') and b'facet' in payload[:20000].lower())
    else:
        valid = b'ISO-10303-21' in payload[:1000]
    require(valid, 'Invalid model signature')
    return payload, ext, member


def records(root):
    result, ids = [], set()
    for path in sorted(root.glob('download-records-*.json')):
        rows = json.loads(read_file(path, 4 * 1024 * 1024))
        require(isinstance(rows, list), 'Download records must be a list')
        for row in rows:
            require(isinstance(row, list) and len(row) == 4 and all(isinstance(x, str) for x in row),
                    'Record must contain URL, filename, author and observed license')
            url, filename, _, _ = row
            parsed = urlsplit(url)
            match = re.fullmatch(r'/[a-z]{2}/models/([0-9]+)(?:-[^/]+)?', parsed.path)
            require(parsed.scheme == 'https' and parsed.netloc == 'makerworld.com' and match,
                    'Expected public MakerWorld model URL')
            ident = match[1]
            require(ident not in ids, 'Duplicate design ID: ' + ident)
            require(filename not in ('', '.', '..') and '/' not in filename and '\\' not in filename
                    and '\x00' not in filename, 'Unsafe download filename')
            ids.add(ident)
            result.append((ident, row))
            require(len(result) <= 1000, 'Too many corpus records')
    require(result, 'No download records found')
    return result


def prepare(root, downloads=None, check=False):
    root = root.resolve(strict=True)
    manifest_path = inside(root, 'manifest.json')
    existing = json.loads(read_file(manifest_path, 4 * 1024 * 1024)) if manifest_path.exists() else []
    require(isinstance(existing, list), 'Manifest must be a list')
    rows = records(root)
    require(len(existing) <= len(rows), 'Manifest has more cases than records')
    if check:
        require(len(existing) == len(rows), 'Manifest needs preparation for new records')
    output, hashes = [], set()
    for index, (ident, row) in enumerate(rows, 1):
        url, original, author, license_text = row
        source = inside(root, f'raw/{ident}/{original}')
        if not source.exists():
            require(not check and downloads is not None, 'Raw source missing: ' + str(source))
            imported = downloads.resolve(strict=True) / original
            # Validate before importing. Never extract archive paths to the filesystem.
            model(imported)
            source.parent.mkdir(parents=True, exist_ok=True)
            raw = read_file(imported)
            with source.open('xb') as target:
                target.write(raw)
        payload, ext, member = model(source)
        filename = NAMES[(index - 1) % len(NAMES)] + (ext.upper() if index % 3 == 0 else ext)
        relative = f'cases/{index:03d}/{filename}'
        target = inside(root, relative)
        digest = hashlib.sha256(payload).hexdigest()
        require(digest not in hashes, 'Duplicate model payload: ' + ident)
        hashes.add(digest)
        entry = dict(case=index, design_id=ident, source_url=url, author=author,
                     license_observed=license_text, license_fully_verified=False,
                     raw=str(source.relative_to(root)), archive_member=member, file=relative,
                     format=ext[1:], size_bytes=len(payload), sha256=digest,
                     exceeds_25_mib=len(payload) > 25 * 1024 * 1024)
        if index <= len(existing):
            require(existing[index - 1] == entry, 'Existing manifest entry changed: ' + str(index))
        if target.exists():
            require(read_file(target) == payload, 'Prepared file differs: ' + str(index))
        else:
            require(not check, 'Prepared file missing: ' + str(index))
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open('xb') as stream:
                stream.write(payload)
        output.append(entry)
    if not check:
        temporary = inside(root, 'manifest.json.tmp')
        with temporary.open('x', encoding='utf-8') as stream:
            json.dump(output, stream, ensure_ascii=False, indent=2)
            stream.write('\n')
        temporary.replace(manifest_path)
    return dict(cases=len(output), unique_payloads=len(hashes), checked=check,
                formats={ext: sum(x['format'] == ext for x in output) for ext in sorted({x['format'] for x in output})})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path, help='Explicit private corpus directory')
    parser.add_argument('--downloads', type=Path, help='Optional local browser download directory')
    parser.add_argument('--check', action='store_true', help='Verify records, raw and cases without writes')
    args = parser.parse_args()
    try:
        print(json.dumps(prepare(args.root, args.downloads, args.check), ensure_ascii=False))
    except (ValueError, OSError, zipfile.BadZipFile, KeyError, RuntimeError, NotImplementedError) as error:
        parser.exit(1, f'Corpus error: {error}\n')
