# Backend Rapidmaker

Jeden Cargo workspace. Kompilowalny szkielet nie oznacza działającej funkcji. Wszystkie pakiety są prywatne (`publish = false`).

```text
backend/
├── Cargo.toml
├── Cargo.lock
├── run.py
└── crates/
    ├── app/src/
    │   ├── main.rs
    │   └── api/
    │       ├── mod.rs
    │       ├── models.rs
    │       ├── checkout.rs
    │       └── packing.rs
    ├── geometry/       # parser i tests/fixtures/
    ├── packing/        # algorytm i testy rozmieszczenia
    ├── pricing/        # obecna taryfa POC
    ├── slicer/         # szkielet
    ├── storage/        # szkielet
    ├── orders/         # szkielet
    ├── auth/           # szkielet
    ├── payments/       # szkielet
    ├── shipping/       # szkielet
    ├── notifications/  # szkielet
    └── observability/  # szkielet
```

Każdy crate ma `Cargo.toml` i `src/`; biblioteki mają `src/lib.rs`, aplikacja `src/main.rs`. `target/` jest wspólny i ignorowany przez Git.

| Crate | Stan i odpowiedzialność |
| --- | --- |
| `app` | Działa: serwer Axum, HTTP, analiza w sandboxie i orkiestracja checkoutu Stripe/Furgonetki. |
| `geometry` | Działa: parser geometrii/GLB, używany przez izolowany proces app. |
| `packing` | Działa: oszacowanie rozmieszczenia części w paczkach, bez HTTP. |
| `pricing` | Działa: zachowana taryfa POC 24 zł/szt. + 15 zł przygotowania; bez slicera i kalkulacji kosztów. |
| `slicer` | Do implementacji: profile, izolowany slicer, rzeczywisty czas i masa. |
| `storage` | Do implementacji: PostgreSQL/SQLx, migracje i trwałe prywatne pliki. |
| `orders` | Do implementacji: trwałe zamówienia, zaakceptowane ceny i statusy realizacji. |
| `auth` | Do implementacji: logowanie, sesje, role i własność danych. |
| `payments` | Do implementacji: wydzielenie Stripe i trwała obsługa webhooków. Obecny adapter jest w app. |
| `shipping` | Do implementacji: wydzielenie Furgonetki, etykiety i tracking. Obecny adapter jest w app. |
| `notifications` | Do implementacji: email, rozmowa zamówienia i preferencje powiadomień. |
| `observability` | Do implementacji: logi strukturalne, korelacja i metryki dla Grafany. |

## Uruchomienie z katalogu repo

```sh
cargo build --locked --manifest-path backend/Cargo.toml
python backend/run.py
```

Launcher czyta rootowy `.env` (0600, [SOPS](../docs/secrets.md)) i uruchamia `backend/target/debug/rapidmaker-api` na `127.0.0.1:18432`. Worker modeli nadal wymaga Linux, bubblewrap i prlimit. Osobny crate nie zastępuje izolacji procesu. Restart traci stan analiz i wycen w RAM.

Wybrany pakiet: `cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-packing`. Wszystkie pakiety: `cargo test --locked --manifest-path backend/Cargo.toml --workspace`. Budowa wszystkich, także szkieletów: `cargo build --locked --manifest-path backend/Cargo.toml --workspace`.

Dodawaj zależności do szkieletów dopiero przy implementacji. Trzymaj algorytmy niezależne od transportu HTTP. Unikaj cykli i wspólnego worka typów bez konkretnej potrzeby.
