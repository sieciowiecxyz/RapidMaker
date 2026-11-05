# Private model corpus

The browser downloads models manually into a local directory. This tool makes no network requests and grants no license rights. Keep the corpus ignored/private.

`download-records-*.json` contains ordered arrays `[source_url, original_filename, author, observed_license]`. Add new records after existing records; do not reorder or rename previous downloads. One distinct MakerWorld design and model payload per case.

```sh
# Verify existing records, source files, selected models and hashes; no writes.
python frontend/tests/prepare-corpus.py backend/test-models/makerworld --check

# After more browser downloads/records: import missing raw files and append cases.
python frontend/tests/prepare-corpus.py backend/test-models/makerworld --downloads /home/sieciowiec/Pobrane

# Offline regression tests using temporary synthetic files only.
python -m unittest discover -s frontend/tests -p test_prepare_corpus.py -v
```

Preparation preserves existing manifest entries, case names and payloads. It selects the largest bounded model member of each ZIP, retains the real extension and assigns deterministic unusual filenames. It refuses changed cases, duplicate IDs/payloads, unsafe paths, symlinks, invalid signatures and oversized/high-ratio ZIP members. It never extracts archive paths or overwrites the corpus README/results. Signature checks are not geometry validation; run `integration/corpus.spec.ts` for the actual API/slicer pipeline.

## Real upload and WebGL views

From the repository root, with the ignored local corpus prepared:

```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" \
E2E_REPORT_NAME=corpus-ui-final \
python frontend/tests/run-integration.py corpus-ui.spec.ts showcase.spec.ts
```

Requires cases5,9,22,25 exactly once, with matching paths/hashes and original files within the25 MiB UI limit. Uses the actual form, private GLB, large rendered meshes/rotation, two widths/themes, server-price display and explicit size-based manual handling. It is opt-in; normal `core` does not silently read third-party models. Pixel probes are not golden image baselines. The runner fixes the owned browser locale to UTF-8 so Unicode paths reach Chromium's FileList.
