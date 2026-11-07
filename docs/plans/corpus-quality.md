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

| Zakres | Ostatni dowód i granica |
| --- | --- |
| Korpus API, `69bec69` | 41 PASS: 24 ceny slicera z rachunkiem, 12 ręcznych, 4 wadliwe, 1 limit; 36 GLB. Odmowa to poprawny wynik testu, nie obsługa modelu. |
| Nowy korpus API,6217076 | 40 PASS/1 FAIL: model37 upload503 po25s, pozostałe24native/12manual/36GLB jak wcześniej. Scoped37 później422/exit67; przyczyna pierwotnego503 nieustalona. Dodano bezpieczny log exit/signal, bez zmiany oczekiwań. |
| Model37 — poprawka | Ograniczona wskazówka topologii potwierdzonego roota: exit67 około1,9s zamiast20,9s, te same limity.18unit/4nativeAPI PASS; reviewerowa regresja secondary usunięta. Pełny korpus acbd999 sprawdzony poniżej. |
| Bieżący korpus API,acbd999 | **41 PASS/0 FAIL/0 SKIP/0 flaky**,594,9s:24native/12manual/4wadliwe/1limit i36GLB. Model37typed422 w2,212s.486hashy i oczekiwania bez zmian; osobny przebieg, nie nowy dowód UI. |
| Korpus UI, `40e8a5d` | 41 PASS, 36 renderów, 144 unikalne obejrzane ujęcia; ceny UI tylko 9/20/29. To osobny przebieg na innym HEAD. |
| Bieżący korpus UI,d560c97 | **41 PASS/0 FAIL/0 SKIP/0 flaky**,662,9s:36renderów,144unikalne obejrzane ujęcia1440/390/light/dark.486hashy zgodne. CenyUI tylko9/20/29; orientacja29 i formalny kontrast niepotwierdzone. |
| Macierze3MF — nowsza poprawka | Trzy kolumny konstruktora poprawione: rotation/shear/nested3MF miały3FAIL, teraz22unit/2nativeAPIUI PASS. Model29upright/basebelow zgodnie ze źródłem. PełnyAPI/UI41 po tej zmianie sprawdzono poniżej. |
| Bieżące API/UI po macierzach,0e53ffb | Osobno41PASSAPI i41PASSUI;24ceny/12manual/36GLB i144obejrzane ujęcia. Model25poprawnieprinter_fit zamiastsize_limit; staryUI40/1 zachowany, ścisłytestpowodupoprawiony.486hashy bez zmian w obu przebiegach. |
| Ciemny viewer — nowsza zmiana | Rim0,18dark/0,11light: przed/po osobno3nativePASS;16ujęć/etap, jasne8pikselowoidentyczne. OsobnoWebGLrestore2PASS. Małe rozproszone części nadal małe; bez zmiany kolorów/style i bez nowegopełnego41. |
| Większe modele | Pełny STL, kompaktowy/spatial GLB i jawnie uproszczony tylko podgląd; [budżety](large-models.md). Model 41 nadal ponad limitem; 14/21/32/37 poprawnie odrzucone jako wadliwe. |
| Zbiorczy `baad9d1` | **179 PASS / 0 FAIL / 1 opt-in SKIP**, 0 flaky; osiem grup exit0, wrapper2 wyłącznie przez skip.441hashy bez zmian. |
| Large na tym samym HEAD | Pominięty przypadek uruchomiony osobno: **1 PASS/0SKIP**,16,5s, te same441hashy. Nie zmienia wyniku wrappera. |
| Historia `6addae4` | 170PASS/3FAIL/1SKIP zachowane w raporcie; nowszy zbiorczy przebieg powyżej dotyczy innego kodu. |
| Poprawka `a03f362` — celowane | Privacy 11 PASS; panele 1 PASS i journey 1 PASS osobno po naprawie fokusu iframe/synchronizacji. Nie sumować w nowy wynik pełnego zestawu. |
| Nowsze zmiany | `5222964`: prywatny cache/access 1 PASS; `63e5387`: Worker 2 PASS (marker, bez osobnego JSON); `3aed38f`: podgląd 3 PASS (mock HTTP, rzeczywiste Worker/parser/GPU). |
| Opcje wyceny — celowane | Native standard/draft/fine, skrajne ściany/wypełnienie: 1 PASS, trzy rachunki dziesięciu pól i snapshoty; 9 błędnych konfiguracji bez zadań. Jeden model/PLA, nie cała macierz. |
| Anulowanie slicera | Natywna regresja1 PASS: produkcyjny sandbox/zaufany fork, oba procesy kończą i Workspace znika. Bez Bambu/crash hosta; łączne limity potomków i inodów nadal otwarte; bajty `/out` ograniczone poniżej. |
| Wyjście slicera | `/out` tmpfs512 MiB + hostJSON maks.128 MiB; cztery native płyty i regresje błędów/cleanup PASS. Bez pełnego korpusu; metadane/inody i aggregate procesów nadal otwarte. |
| API nowego `/out` | Builda3d97ca: native formaty/opcje/partie3 PASS. Osobno rozszerzona partia26=25+1:1 PASS,2płyty i rachunek. Źródła/binary stabilne, normalny API bez restartu. |
| Ceny i zasoby | Minimum koszyka: 2 PASS, rzeczywisty slicer na stawkach testowych. Osiem klientów: 1 PASS, próbkowane RSS drzewa 187,2 MiB; nie limit RAM ani przepustowość. |
| Prostota | Usunięcie Badge: 71 linii w 3 plikach, w tym 31 linii komponentu, i 3 bezpośrednie devDependencies; nie sumować jako całego usuniętego kodu. Szczegóły/audyt w raporcie. |
| Launcher — celowane | Przypięcie binarki: baseline upload201→503 po podmianie, fix201→201 z tym samym PID/inode/SHA; 8 unit, 4 native integration i osobny queue-restart1 PASS. Bez restartu zwykłego API. |
| Runtime | Ostatni opisany lokalny smoke: `7fb2a24`, upload/GLB/cena/delete PASS, rachunek 37,05 zł. To zapis historyczny, nie potwierdzenie obecnego procesu lub wdrożenia. |

## Dalsza praca i granice

Goal pozostaje otwarty. Konsultować dalsze działania z reviewerem według polecenia właściciela; wybierać konkretny wykonalny problem i potrzebną regresję. Same oceny nie są technicznym blokerem. Nie powtarzać pełnych zestawów tylko dla wyższej liczby.

Granice: model 41 i wadliwe źródła, rzeczywiste koszty/rentowność firmy, pełna macierz opcji, fizyczny telefon/passkey, formalny kontrast, szerokie obciążenie/pentest i ogólna prostota kodu. Testy lease/restartu nie dowodzą odporności na awarię hosta/power loss; headless nie dowodzi natywnego BFCache lub przełączania okien. Scoped review nie potwierdza 10/10 produktu.

## Uruchamianie według potrzeby

Wyłącznie headless; zakres i konfiguracja w [testing](../testing.md). Poniższe komendy nie oznaczają nowego wykonania:

```sh
python frontend/tests/run-all.py --group core --no-build
E2E_REPORT_NAME=eight-clients-current python frontend/tests/run-integration.py concurrency.spec.ts
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=corpus41-large-current python frontend/tests/run-integration.py --large-models corpus.spec.ts
```

`core` uruchamia też `privacy`; `all` zawiera privacy i grupy providerów. `--no-build` wymaga aktualnego buildu. Nadawaj nowe nazwy i zachowuj poprzednie raporty (wrapper używa stałych nazw — archiwizuj je przed ponowieniem). Pełne/providerowe zestawy tylko celowo; sekrety poza raportami. Starsze checkpointy i FAIL harnessu pozostają w raporcie oraz Git.
