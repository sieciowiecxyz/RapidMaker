# Rapidmaker — rapidmaker.pl

POC zamawiania wydruków FDM. Frontend: Svelte 5 / TypeScript / Vite. Backend: Rust / Axum, analiza modeli, pakowanie oraz sandboxy Stripe i Furgonetki. Konta i panel admina używają mocków; nie jest to gotowy sklep.

## Start lokalny

Odszyfruj jeden `.env` według [instrukcji SOPS](docs/secrets.md). Wymagania: Node.js, npm, Rust/Cargo, Python 3; analiza modeli wymaga Linux, bubblewrap i prlimit.

```sh
npm --prefix frontend ci
cargo build --locked --manifest-path backend/Cargo.toml
python backend/run.py
```

W drugim terminalu:

```sh
npm --prefix frontend run dev
```

Strona: http://127.0.0.1:18430. API: 127.0.0.1:18432. Oba procesy są lokalne; Vite przekazuje `/api` do backendu.

## Struktura

- `frontend/` — interfejs, mocki i testy UI.
- `backend/` — API Rust, `geometry/` — parser, `tests/fixtures/` — modele testowe.
- `docs/` — [indeks](docs/INDEX.md), [plan](docs/PLAN.md), [milestones](docs/MILESTONES.md) i [architektura](docs/ARCHITECTURE.md).
- `secrets/dev.enc.env` — wspólna zaszyfrowana konfiguracja. Jawny `.env` jest poza Gitem.

Krótka weryfikacja: `npm --prefix frontend run build` i `cargo build --locked --manifest-path backend/Cargo.toml`. Więcej: [testing](docs/testing.md), [frontend](frontend/README.md).

Stary kod, Dockerfile i Taskfile usunięto; są dostępne w historii Git.
