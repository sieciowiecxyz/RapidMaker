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
E2E_REPORT_NAME=corpus-ui-all41 \
python frontend/tests/run-integration.py corpus-ui.spec.ts
```

Requires the41 accepted unique cases, including5/9/22/25, with matching paths/hashes. Four deep tests retain pricing/manual/privacy checks. Every other file exercises actual form rendering or explicit size/resource rejection. Rendered models get four normal scene captures and pixel/interactivity checks (rotation in4 deep cases, zoom in21 others). A rejected file is not counted as a successful quote; the API pipeline report remains separate. Oversized input must show the25 MiB form limit without an upload POST. Resource responses are checked against a repeated identical owner HTTP request when Chromium cannot retain the browser response body, then matched to the actual settled UI error and disabled checkout after print settings are completed. It is opt-in; normal `core` does not silently read third-party models. Pixel probes are not golden image baselines. The runner fixes the owned browser locale to UTF-8 so Unicode paths reach Chromium's FileList.
