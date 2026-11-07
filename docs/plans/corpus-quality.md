# Cel: rzeczywiste modele i wiarygodne E2E

Przeczytaj gdy: wznawiasz goal lub oceniasz jego zamknięcie.
Opis: wymagania i checkpoint; szczegółowe dowody w [mapie E2E](../e2e-coverage.md), historia w [raporcie](../corpus-report.md).

## Wymagania

- 41 prywatnych projektów MakerWorld z pochodzeniem/SHA: 34×3MF, 7×STL; pobieranie zakończone. STEP z fixtures.
- Rzeczywisty upload, prywatny podgląd, slicer i niezależny rachunek ceny. Ręczna oferta ani odmowa nie oznacza automatycznej wyceny/obsługi geometrii.
- Zachować obrazy, opcje i interakcje Piotra: konfigurator, zakup/refundy, dostawa, klient/admin, MFA, chat i rzeczywisty email; bez redesignu. [Kontrakt UI](../frontend-contract.md).
- Minimalny Rust, proste API/KISS według [architektury](../ARCHITECTURE.md); zachować kwoty, własność, izolację i limity. Nie zmieniać oczekiwań dla zielonego raportu.
- Po etapie Sol xhigh: poprawność, bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody, prostota, utrzymanie, wygląd. Naprawiać konkretne problemy; nie wymuszać 10/10.

## Checkpoint — 2026-11-07

Wyniki dotyczą wskazanych wersji i osobnych przebiegów. Nowszy dowód celowany uzupełnia wcześniejszy pełny zestaw; nie tworzy nowego wyniku całego produktu. Szczegóły i historyczne FAIL pozostają w [raporcie](../corpus-report.md).

| Zakres | Ostatni dowód i granica |
| --- | --- |
| Pełny korpus po macierzach 3MF, `0e53ffb` | Osobno **41 PASS API** i **41 PASS UI**, bez FAIL/SKIP/flaky. API: 24 ceny z rachunkiem, 12 ręcznych, 4 wadliwe, 1 limit, 36 GLB. UI: 36 renderów i 144 obejrzane ujęcia (1440/390, jasny/ciemny), ceny tylko 9/20/29. Model 25: `printer_fit`; model 29: upright/base below. 486 hashy bez zmian. Odmowa nie oznacza obsługi geometrii; nie potwierdzono intencji każdego złożenia. |
| Nowsze oświetlenie viewera | Rim 0,18 dark/0,11 light: przed i po osobno **3 native PASS**, po 16 ujęć; 8 jasnych canvasów identycznych pikselowo. Osobno WebGL loss/restore **2 PASS**. Małe rozproszone części nadal małe. To celowana weryfikacja nowego światła, bez nowego pełnego korpusu 41. |
| Większe modele | Pełny STL, kompaktowy/spatial GLB, uproszczenie jawnie tylko dla podglądu; [budżety](large-models.md). Model 41 nadal ponad limitem; 14/21/32/37 wadliwe. Model 37 na `acbd999` po poprawce topologii: typed 422 w 2,212 s; przyczyna wcześniejszego upload 503 nieustalona. |
| Pełny zestaw integracji, `baad9d1` | **179 PASS / 0 FAIL / 1 opt-in SKIP**, 0 flaky; osiem grup exit 0, wrapper 2 przez skip, 441 hashy bez zmian. Obejmuje auth, dostawców i email. Pominięty large osobno **1 PASS**, ten sam HEAD/hashe; nie sumować do nowego wyniku wrappera. Nie powtarzać niezmienionych przepływów tylko z powodu wieku dowodu. |
| Celowane regresje UI/prywatności | `a03f362`: privacy 11 PASS, panele i journey osobno po 1 PASS. Nowsze: cache/access 1 PASS (`5222964`), Worker 2 PASS (`63e5387`, marker bez osobnego JSON), podgląd 3 PASS (`3aed38f`, mock HTTP, rzeczywisty Worker/parser/GPU). |
| Opcje, ceny i współbieżność | Standard/draft/fine, skrajne ściany/wypełnienie: 1 native PASS, pięć rachunków dziesięciu pól (3 PLA + PETG/TPU z różnymi kosztami), 9 błędnych konfiguracji oraz cofnięte dopuszczenie PETG bez zadań; jeden model. Partia 26 = 25+1: osobno 1 PASS. Minimum koszyka 2 PASS. Ośmiu klientów 1 PASS, próbkowane RSS drzewa 187,2 MiB; nie pełna macierz, limit RAM, przepustowość ani rentowność. |
| Izolacja i trwałość | `/out` tmpfs 512 MiB, host JSON maks. 128 MiB; regresje błędów/cleanup i native formaty/opcje/partie PASS. Anulowanie: 1 native PASS z zaufanym forkiem, bez Bambu. Łączne budżety procesów/potomków i metadanych/inodów otwarte. Lease/restart i przypięcie binarki mają osobne dowody w [mapie](../e2e-coverage.md); nie dowodzą awarii hosta/power loss. |
| Prostota i runtime | Usunięcie Badge i trzech bezpośrednich devDependencies opisano w raporcie; nie dowodzi to prostoty całości. Smoke `7fb2a24`: upload/GLB/cena/delete PASS, 37,05 zł. To historia localhost, bez potwierdzenia obecnego procesu lub wdrożenia. |

## Dalsza praca i granice

Goal pozostaje otwarty. Probe inodów i64pustychplików wykonany; obecny Bubblewrap nie udostępnia kwoty `nr_inodes`. Wykrytą możliwość tworzenia kolejnych user namespaces zablokowano w obu workerach (natywne probes i API2PASS). Twarda kwota inodów oraz sumy RAM/CPU/PID potomków nadal wymagają rozwiązania mount/deployment; nie zastępować ich pollingiem ani RLIMIT_NOFILE. Zachować obecne limity i fail-closed, bez udawania ukończenia. Szczegóły i warianty: [bezpieczeństwo](../security.md#zagnieżdżone-przestrzenie-użytkownika).

Dalej: pełna macierz opcji ponad jedną kostkę i wybrane materiały; fizyczny telefon/passkey i formalny kontrast; rzeczywiste koszty/rentowność firmy. Otwarte pozostają szerokie obciążenie/pentest i ogólna prostota kodu. Headless nie dowodzi natywnego BFCache ani przełączania okien. Konsultować konkretne poprawki z reviewerem według polecenia właściciela; nie wymuszać 10/10, nie traktować ocen jako blokera ani powtarzać pełnych zestawów dla wyższej liczby.

## Uruchamianie według potrzeby

Wyłącznie headless; zakres i konfiguracja w [testing](../testing.md). Poniższe komendy nie oznaczają nowego wykonania:

```sh
python frontend/tests/run-all.py --group core --no-build
E2E_REPORT_NAME=eight-clients-current python frontend/tests/run-integration.py concurrency.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
```

`core` uruchamia też `privacy`; `all` zawiera privacy i grupy providerów. `--no-build` wymaga aktualnego buildu. Nadawaj nowe nazwy i zachowuj poprzednie raporty (wrapper używa stałych nazw — archiwizuj je przed ponowieniem). Pełne/providerowe zestawy tylko celowo; sekrety poza raportami. Starsze checkpointy i FAIL harnessu pozostają w raporcie oraz Git.
