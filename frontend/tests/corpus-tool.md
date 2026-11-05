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
