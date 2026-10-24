# Bezpieczeństwo — przegląd 2026-10-24

Sprawdzono granice auth/własności, analizę plików, adapter płatności, metryki, eksport panelu i zależności. To ograniczony przegląd kodu i lokalne regresje, nie pełny pentest ani certyfikat bezpieczeństwa. Nie testowano atakami usług zewnętrznych.

## Poprawione

- Zależności geometrii: `lib3mf` 0.1.6→0.1.7, `quick-xml` 0.22/0.39.4→0.42, `zip` 8.6 usunięty (pozostaje poprawiona 9.0.5). Usunięto nieużywane features operacji boolean i VTK/analyzers; pozostaje potrzebna tessellacja. Lockfile: 471→419 pakietów, bez usuwania obsługi STL/STEP/3MF.
- Cargo audit: 9→1 zgłoszeń. Usunięte podatne ścieżki `lz4_flex`, stare `quick-xml` i `zip` obejmowały wyciek danych/dekompresję, zużycie zasobów oraz niebezpieczne rozpakowywanie. Parser nadal działa w izolowanym procesie z limitami; sama aktualizacja nie zastępuje tej ochrony.
- Cookie próby logowania: sprawdzane są wszystkie nagłówki i wystąpienia. Duplikat lub błędna wartość odrzuca próbę, zamiast wybierać pierwszy poprawny token. Funkcja ma teraz widoczność tylko w module.
- Katalog roboczy analizy od razu powstaje z trybem 0700, zamiast osobnego create/chmod.
- CSV panelu: pola są cytowane, cudzysłowy escapowane, potencjalne formuły (także po białych znakach i ich pełnoszerokie warianty) dostają tabulator wewnątrz cytowanego pola. To eksport do oglądania w arkuszu, nie bezstratny format wymiany danych. Zachowanie ponownego zapisu/importu zależy od arkusza; nie ma jednej ochrony dla wszystkich programów. [Źródło: OWASP](https://community.owasp.org/attacks/CSV_Injection).
- Martwy kod: usunięto nieużywany import ikony oraz nieużywane funkcje zależności. `cargo machete --with-metadata` nie znalazł zbędnych bezpośrednich zależności; nie dowodzi to osiągalności każdej publicznej funkcji.

## Nadal otwarte

- `rsa` 0.9.10: [RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), brak poprawionej wersji. Zapis w lockfile pochodzi z opcjonalnego SQLx/MySQL; `cargo tree -i rsa --target all` nie pokazuje aktywnej ścieżki w konfiguracji projektu. Używamy PostgreSQL. Pełny `cargo audit` nadal zwraca FAIL; nie ukryto wyjątkiem tego zgłoszenia. Zmiana features/backendu SQLx wymaga ponownego sprawdzenia.
- Ostrzeżenia upstream: `cgmath` (unmaintained/unsound) oraz `proc-macro-error` (unmaintained). Nie znaleziono wywołań zgłoszonego `swap_columns` w sprawdzonej ścieżce truck/matext; to ocena osiągalności, nie poprawka biblioteki. Parser STEP pozostaje izolowany. Wymiana stosu CAD wykracza poza małą poprawkę.
- Nowy panel `/admin.html` nadal jest wyraźnie oznaczonym prototypem z przykładowymi danymi w localStorage. Nie daje dostępu do backendu; nie wkładać tam prawdziwych danych klientów. Podłączenie serwerowych sesji/passkey/CSRF pozostaje zadaniem integracji.

## Dowody

- Geometria: 14 testów PASS; API/auth: 20 testów PASS. Clippy zmienionych pakietów/all-targets `-D warnings`, fmt i locked build PASS.
- Frontend: svelte-check/build PASS, wybrana regresja CSV PASS (test funkcji w runnerze Playwright, bez pełnego E2E), npm audit: 0 zgłoszeń.
- Działający localhost po aktualizacji: OpenAPI 200, niejednoznaczne cookie logowania 401, admin bez sesji 401, poprawny STL 200, 3MF path traversal 422. Bez nadawania paczek, płatności i wysyłki emaili w ramach testu.
- Lokalne raporty: `/tmp/rapidmaker-security-{rust-tests,clippy,build,frontend,machete}.log`, `/tmp/rapidmaker-security-smoke.json`, `/tmp/rapidmaker-dependency-audit.json`. Nie są przechowywane w Git.
