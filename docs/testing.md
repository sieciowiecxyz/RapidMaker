# Szybka weryfikacja

Testy mają pomagać dostarczać kod. Sprawdzaj zmieniony zakres najkrótszą sensowną metodą. Bez obowiązkowej Astry, testujących subagentów, wielokrotnych review i pełnych zestawów przy każdej iteracji. Zielonego wyniku nie powtarzaj bez nowej zmiany, błędu lub konkretnej wątpliwości.

- UI: build i krótki przebieg zmienionego ekranu. Przy zmianie układu obejrzyj komputer i telefon; screenshoty tylko, gdy pomagają ocenić efekt.
- Algorytm: kilka istotnych przypadków granicznych. Dla pakowania: różne części, 1/1000 sztuk, masa, gabaryt i brak danych.
- API: jedna poprawna operacja i najważniejszy błąd. Dla płatności sprawdź serwerową kwotę, ponowienie i brak fałszywego potwierdzenia; dla danych klienta — uprawnienia.
- Docs: przejrzyj zmienione linki i zgodność opisu. Audyt narzędziem tylko, gdy jest potrzebny.

Nie kasuj działających testów bez powodu. Zmieniony kontrakt wymaga dopasowania testów, a wykryty błąd — małej regresji, jeśli jest przydatna. Pełny E2E, fuzzing, pentest, skany i odtwarzanie awarii są osobnymi zadaniami według potrzeb lub przed wydaniem; nie blokują codziennego prototypowania.

## Komendy według potrzeby

- `npm --prefix frontend run build` — Svelte/TypeScript i build.
- `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npm --prefix frontend run test:e2e -- --grep 'nazwa testu'` — wybrany scenariusz.
- `cargo test --locked --manifest-path backend/Cargo.toml -p rapidmaker-app -p rapidmaker-packing -p rapidmaker-pricing` — zmienione reguły API, pakowania i wyceny.
- `cargo build --locked --manifest-path backend/Cargo.toml`, `python backend/run.py` — lokalne Rust API na porcie 18432.
- `cargo fmt --manifest-path backend/Cargo.toml --all -- --check` — formatowanie Rust.
- `cargo test --locked --manifest-path backend/Cargo.toml --workspace` — API i parser, gdy wymagają tego zmiany.

## Granice obecnego demo

Frontend: `frontend/`, port 18430; preview E2E: 18431. Vite przekazuje `/api` do `backend/` na localhost:18432. API uruchom przed testami integracji. Szczegóły konfiguracji: [README frontendu](../frontend/README.md).

Testy sandboxów wymagają lokalnych sekretów: Stripe test oraz `.env` z `FURGONETKA_SANDBOX_TOKEN` i `FURGONETKA_SENDER_JSON`, uprawnienia 0600, poza Gitem. Taryfa druku jest testowa, geometria odczytywana przez parser, masa szacowana. Panele, login i prośby o wycenę pozostają mockami. Brak tokena oznacza brak dowodu integracji, a nie poprawny darmowy przewóz. Nie twórz prawdziwych obciążeń ani nadań.

W `PLAN.md` zapisz krótko, co sprawdzono i czego nie. Nie wklejaj sekretów, modeli klientów ani dużych logów. Testy bezpieczeństwa kieruj tylko do własnego localhosta; dostawców sprawdzaj zwykłymi wywołaniami sandbox API. Publiczny deployment pozostaje poza zakresem.
