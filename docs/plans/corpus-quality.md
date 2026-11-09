# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: wznawiasz goal lub oceniasz jego zamknięcie.
Opis: wymagania i checkpoint; szczegółowe dowody w [mapie E2E](../e2e-coverage.md), historia w [raporcie](../corpus-report.md).

## Wymagania

- 41 prywatnych projektów MakerWorld z pochodzeniem/SHA: 34×3MF, 7×STL; pobieranie zakończone. STEP z fixtures.
- Rzeczywisty upload, prywatny podgląd, slicer i niezależny rachunek ceny. Ręczna oferta ani odmowa nie oznacza automatycznej wyceny/obsługi geometrii.
- Zachować obrazy, opcje i interakcje Piotra: konfigurator, zakup/refundy, dostawa, klient/admin, MFA, chat i rzeczywisty email; bez redesignu. [Kontrakt UI](../frontend-contract.md).
- Minimalny Rust, proste API/KISS według [architektury](../ARCHITECTURE.md); zachować kwoty, własność, izolację i limity. Nie zmieniać oczekiwań dla zielonego raportu.
- Po etapie Sol xhigh: poprawność, bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody, prostota, utrzymanie, wygląd. Naprawiać konkretne problemy; nie wymuszać 10/10.

## Checkpoint — 2026-11-08

Goal otwarty; różne wersje/przebiegi nie tworzą nowego PASS całości. FAIL/hashe/szczegóły: [raport korpusu](../corpus-report.md), [mapa E2E](../e2e-coverage.md), [security](../security.md) i [testing](../testing.md). Poprzedni checkpoint: Git `0a2a360:docs/plans/corpus-quality.md`.

| Nowszy zakres | Dowód i granica |
| --- | --- |
| Dwa warianty po zapłacie | **1 PASS**: wspólny upload PLA ×1/PETG ×2, ręczna oferta 25 zł, rzeczywista płatność Stripe TEST i niezależna kontrola kwoty/korelacji. Panel admina zapisuje [1,0], klient zachowuje obie pozycje; dokładnie 3 SMTP sent tylko void. Bez IMAP/automatycznej wyceny. Zachowany błąd końcowego bind harnessu i brak zapisu PID prywatnego API; cleanup potwierdzony osobno. [Dowody](../e2e-coverage.md#płatność-dwóch-wariantów--2026-11-08). |
| Nowe stany konfiguratora | Obejrzano 8 rzeczywistych viewportów (1440/390, jasny/ciemny): identyczny wariant i ręczna wycena, tekst/CTA bez ucięcia lub overflow. Kontrolowane HTTP, bez CSS patcha; pierwszy błędny sposób zapisu sticky/fixed zachowany. To nie pełne WCAG/UI lub backend. [Zakres](../e2e-coverage.md#stany-konfiguratora--2026-11-08). |
| Manual razem, `9d7793a` | **4/4 PASS**, headless/prywatna DB/API, 0 SKIP/flaky/retry: publikacja, terminalny slicer → manual UI, dwa warianty/admin unpaid gate, fizyczny GC. Bez providerów/doręczenia emaila; nie full core. |
| Live journey, `f257f53` | **1 PASS**: oferta → Stripe TEST 25 zł → zwrot 5 zł → email/chat dedup. 18 obserwacji jednej pozycji potwierdza line_id=quote_id/model/ilość/opcje; niezależne GET Stripe: kwoty/korelacja/testmode. 5 SMTP sent tylko void; IMAP oferta/umowa/zwrot/chat +2 reply, payment_received bez osobnego IMAP. Nie płatność wielu wariantów. [Pełny zakres](../testing.md). |
| Warianty/manual/retencja | Jeden upload/różne konfiguracje: osobne line_id. Backend odrzuca dokładne duplikaty; UI blokuje identyczny klon, pozwala zmienić ilość/ustawienia/preferencje kolorów. PG syntetycznie paid: [1,0]→[2,0]; native unpaid admin: blokada UI/422 przy ważnym auth/CSRF, bez udanej edycji paid w UI. Ostatni hold zachowuje źródło/STL/GLB; oba wygasłe → GC/410, dwa snapshoty zostają. Daty we własnym fixture; paid/completed niebadane. [Dowody i FAIL fixture](../security.md#2026-11-08--warianty-jednego-uploadu-w-ręcznej-wycenie). |
| Opcje modelu 29 | **4/5 auto w osobnych przebiegach**: PLA standard ×1/draft ×2, PETG/TPU standard ×1, rachunki według stawek testowych. Fine ×3: historyczne exit137 bez diagnozy; nowa pojedyncza próba wait4: SIGKILL przy 120,021 s CPU/limicie 120 s, około 744 MiB RSS, bez timeout/OOM delta — mocny dowód limitu CPU w tej próbie. Manual API bez ceny/płatności pozostaje. Cienki STL: pełny manual UI po trzech exit156; bez diagnozy 156/137. Zachowano 2 FAIL macierzy/4 FAIL harnessu. [Rachunki](../corpus-report.md#2026-11-08--opcje-na-złożonym-modelu-i-bezpieczny-fallback), [polityka/dowody](../security.md#2026-11-08--przerwane-cięcie-bez-kosztownego-retry). |

## Dowody historyczne — nie obecny pełny przebieg

| Zakres | Zachowana interpretacja |
| --- | --- |
| Korpus `0e53ffb` | Osobno **41 PASS API / 41 PASS UI**: API 24 auto/12 manual/4 invalid/1 limit/36 GLB; UI 36 renderów/144 ujęcia desktop/mobile/light/dark, ceny tylko 9/20/29. Odmowa nie oznacza obsługi geometrii. Po dokładnej masie `5173459`: osobno 41 PASS API/24 rachunki/36GLB, bez nowych odmów. Oracle publiczne f64; surowe tokeny testowane w Rust, snapshoty/stare ceny do wygaśnięcia bez zmian. [Mapa](../e2e-coverage.md#najnowsze-dowody--2026-11-08). |
| Integracja `baad9d1` | **179 PASS / 0 FAIL / 1 opt-in SKIP**, 0 flaky, osiem grup exit0/wrapper2; large osobno 1 PASS przy tych samych hashach. Nie sumować do pełnego wyniku. [Mapa](../e2e-coverage.md). |
| Geometria/viewer | Pełny STL, kompaktowy/spatial GLB; tylko podgląd upraszczany. Model 41 ponad limitem; modele 14/21/32/37 wadliwe; 37 typed422, stary503 niewyjaśniony. Światło i WebGL loss/restore mają odrębne dowody, bez nowego41 UI. [Historia](../corpus-report.md), [budżety](large-models.md). |
| Ceny/współbieżność | Kostka/opcje, partie/minimum/maksimum, ośmiu klientów i kontrolowana kolizja refundów mają odrębne dowody/negatywne kontrole; próbkowane RSS nie dowodzi limitu/przepustowości. [Raport](../corpus-report.md), [mapa](../e2e-coverage.md). |
| Security/regresje | Email preflight zachowuje wygasły claim do odzyskania; bez exactly-once SMTP. Klient/admin/faktury, overlap/cache/Worker, pobieranie (producent/deadline300 s), Vite, rodziny sesji i checkout: zakresy/negatywne kontrole w [security](../security.md) i [mapie](../e2e-coverage.md). Mock HTTP nie jest natywnym backendem; wirtualny passkey nie jest fizycznym kluczem. Dawne uproszczenia/smoke nie dowodzą prostoty całości/aktualnego procesu. |

## Dalsza praca i granice

- **Sekrety 2026-11-09**: 11 aktualnych wartości, 404 śledzone zwykłe pliki i 3412 blobów osiągalnych z lokalnych refs (112 MB) bez jawnych kopii; sprawdzone wzorce sk_/whsec_ również bez trafień. Dwa odbiorniki SOPS zgodne z kluczami właściciela/znajomego, cache głównego hooka zgodny z trzema hashami. `sec-secret-literals-20261109/`; bez odszyfrowania/MAC, skanu nieosiągalnych obiektów, wszystkich dawnych wartości lub kodowań. Brak nowej potwierdzonej luki i zmian produktu.

- **Macierz operatora 2026-11-09**: rzeczywiste 33 chronione operacje (13 odczytów/20 zapisów): 99 odmów dla gościa/klienta/admina przed 2FA oraz 40 dla pełnego admina bez CSRF/z obcym Origin. Dodatnie kontrole i hash 17 tabel PASS; 1 headless test 2,5 s. `sec-admin-boundaries-20261108/result.json`; automatyczne odkrywanie tras/body z OpenAPI, grupa `privacy`. Początkowe 422 od niepełnego body jest zachowane jako błąd fixture. Brak nowej potwierdzonej luki; kod produktu/normalny API/DB bez zmian.

- **Frontend — ostatni krok 2026-11-08**: spóźniony prywatny GLB po logout odrzucany przed wyświetleniem; ponowienie błędnego uploadu faktycznie wysyła plik. Dowód GLB przed FAIL/po PASS i kontrola lokalnego STL, łącznie cztery headless PASS; netto +3 linie produktu dla granicy sesji, bez zależności. `sec-preview-body-20261108/result.json`, test należy do `privacy`. Lokalny Vite używa poprawki, API/DB bez restartu/zmian; wcześniej zapisane lokalne kopie siatek pozostają w szkicu.

- Priorytet właściciela: szukanie i małe poprawki security, bez puchnięcia kodu. Najmniejsze naprawy konkretnych problemów; bez kodu/zależności/testów dla oceny. Konsultować poprawki z reviewerem według polecenia właściciela.
- **Normalny localhost zaktualizowany 2026-11-09**: ten sam backend `77f5cb2`, PID 3645174, przypięta binarka SHA `82e00f94f4e45d81a57242bd03d10d24e9ce693e14166adf4c65c5e6d7fd5146`, teraz w `rapidmaker-api.service`. 25 migracji/checksumów, profil large, konfiguracja, finanse/manual_review/hold i pliki zgodne; frontend/PG bez restartu. Przerwa API 8,85 s. Dowody i niezależny postcheck: `sec-service-envelope-normal-update-20261109/`; poprzednia binarka tej samej schemy zachowana. Szyfrowany backup sprzed migracji pozostaje nietknięty; odtworzenie tamtej wersji wymaga DB i plików, pełnego restore nie wykonywano. Starsze checkpointy są w historii Git. GC wycen nie ustanawia globalnej kwoty DB.
- **Zasoby**: normalny API i natywni potomkowie mają łącznie 8 GiB RAM, swap0, 256 zadań i CPU400%; `OOMPolicy=kill`/`KillMode=control-group`, restart ręczny. Własny unit: upload/GLB/rzeczywista wycena PASS; osobne małe próby OOM64MiB i TasksMax8 potwierdzają zabicie całej grupy oraz EAGAIN. Limity innych aplikacji bez zmian; bwrap/prlimit zachowane. CPUQuota ogranicza tempo, nie sumę CPU-time; nie ma osobnej kwoty inodów ani dowodu maksymalnego obciążenia wszystkich dużych/równoległych modeli. Jeden OOM dotyka całego API. [Polecenie i dowody](../backend/README.md#lokalny-serwer-z-limitem-zasobów).
- **Prywatność/utrzymanie**: stare wymieszane uwagi bez migracji, brak pełnej serializacji kart; [checkout](../security.md#2026-11-08--zmiana-konta-w-otwartym-checkout). Cargo 0 podatności, npm źródłowe LOW deepmerge(dev) bez poprawki; pin-utils ostrzeżenie utrzymania(compiled IMAP), bez wyciszania/zmiany locków. [Audyt](../security.md#2026-11-08--aktualny-audyt-zależności).
- **Nadal niepotwierdzone**: intencja każdego złożenia, małe/rozproszone części, pełna macierz geometrii/opcji/materiałów, rzeczywiste koszty/rentowność, fizyczny telefon/passkey, formalny kontrast/WCAG, natywny BFCache/przełączanie okien, szerokie obciążenie/pentest i ogólna prostota. Zachować ograniczenia i FAIL, bez udawania ukończenia.

## Uruchamianie według potrzeby

Wyłącznie headless; zakres i konfiguracja w [testing](../testing.md). Poniższe komendy nie oznaczają nowego wykonania:

```sh
python frontend/tests/run-all.py --group core --no-build
E2E_REPORT_NAME=eight-clients-current python frontend/tests/run-integration.py concurrency.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
```

`core` zawiera `privacy`, `all` także providerów. `--no-build` wymaga aktualnego buildu. Prywatne DB/root/API, zamrożone źródła/dist/binarka, cleanup; sekrety poza raportami. Unikalne nazwy, zachować raporty/FAIL (wrapper ma stałe nazwy — archiwizować przed ponowieniem). Pełne/providerowe zestawy celowo; nie powtarzać dla wieku dowodu.
