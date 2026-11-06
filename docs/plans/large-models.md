# Większe modele — aktywny plan

Przeczytaj gdy: rozwijasz duże uploady i prywatne podglądy.
Opis: aktualne granice, dowody i następne kroki; szczegóły w [raporcie](../corpus-report.md).

## Cel i decyzje
- Pełna geometria STL/STEP/3MF dla slicera; ograniczony, poprawny podgląd. Nie liczymy413/422 jako obsługi modelu ani błędu konwersji jako udanej ręcznej wyceny.
- Zachować wygląd Piotra, własność, kwoty dysku, izolację, holdy zamówień i dokładne kwoty płatności. Cały cel stron/auth/email/wycen pozostaje w [corpus-quality.md](corpus-quality.md).
- Po decyzji o rozszerzeniu obsługi profil `large` działa na lokalnym serwerze od 6 listopada; wymaga `AUTH_MODE=localhost`. Ustawiono go tylko w środowisku procesu, bez zmiany `.env`. Domyślny profil programu pozostaje standard; produkcji nie wdrażano.
- 41 projektów wystarcza; bez nowych pobrań. STEP z fixtures. Dane/zrzuty prywatne i ignorowane.

## Budżety
| Limit | standard | large — tylko opt-in |
| --- | ---: | ---: |
| API/klient upload |32MiB|96MiB|
| ZIP łącznie/na wpis; liczba |96MiB;512|512MiB;512|
| Kanoniczny STL / unikalne vertices |50MiB/1M|200MiB/4M|
| Trójkąty z budżetu STL |1 048 574|4 194 302|
| GLB / renderer |25MiB/2Mtri|25MiB/2Mtri|
| Proces geometrii |2GiB/25CPU/30wall|bez zmian|
| Bambu na partię |4GiB/60CPU/65wall|4GiB/120CPU/125wall|

Całe zadanie150s/lease180s; dwie duże partie mogą nie zmieścić się w deadline. Kwoty20modeli/256MiB na właściciela i1000/8GiB globalnie obejmują oryginał/STL/GLB. Lokalny STL nadal25MiB/1Mtri; błąd metadata API daje awaryjne25MiB. Wierzchołki renderera mogą zająć144MB +72MB pozycji tymczasowych +GLB/kopia trim/GPU; viewport390px nie dowodzi działania fizycznego telefonu.

## Etapy
1. **Diagnoza — wykonana:** dziewięć dużych przypadków zmierzonych w izolacji, źródłaSHA.14/21/37 mają potwierdzoną wadę topologii,32 niepoprawny pakiet; nie obchodzimy walidatora.41 wcześniej kończył się przy 25 CPU; niezależny licznik wykazał przekroczenie budżetu zespołu, obecnie odrzucane przed drogą walidacją (szczegóły poniżej). Nie potwierdzono pełnej poprawności semantycznej.
2. **Konwersja — częściowo:** kompaktowe u16 i backendowy profil;29 odzyskany przestrzenną kolejnością samych trójkątów, bez zmiany f32/winding/liczności, bez mutacji STL. Dotychczasowy pasujący zapis pozostaje bajtowo identyczny. Sort tylko po porażce obu dotychczasowych zapisów; minimalny rozmiar referencjonowanych pozycji+indeksów pomija niemożliwe próby. Nadal64bloki/64KiBJSON/25MiBGLB.
3. **API/UI — częściowo:**8/20/29/40 mają rzeczywisty upload/privateGLB/render/zoom w1440/390 i obu motywach.20/29 również rzeczywisty slicer/koszyk/cenę UI na jawnych stawkach fixture; nie jest to dowód produkcyjnej rentowności. Dla29 dokładnie porównano każdy bit wszystkich1 993 410 zorientowanych trójkątów z kanonicznym STL, także powtórzenia.
4. **Usuwanie — wykonane w sprawdzonym zakresie:**10E2E17,5s/22DB+API/3zwykłe scenariusze20,9s. Chronione DELETE:204 po fizycznym usunięciu,404 cudzy/brak,409 runninglease/livehold; dokładnyOrigin/CSRF, trwały mark/lockkolejki, retry po błędzie. Sprzątane spóźnione zaakceptowane odpowiedzi i błędy advanced po201. Token gościa przy koncie dotyczy tylko nieprzejętego rekordu, bez niejawnego claimu. Nieznana odpowiedź/zamknięta strona nadal24hretencji; retry ID w pamięci strony.
5. **Audyt/integracja — otwarte:** Solxhigh w10kategoriach po zmianie, konkretne uwagi naprawiane; bez wymuszania10/10. Brak pełnego nowego korpusu/core/providerów po każdej zmianie. Nie sumować osobnych przebiegów w jedenPASS.

## Bieżące dowody modelu29
- `large-model-probe-spatial-minimum/results.json`:29 GLB24 171 984B zamiast29 224 268B;13,975swall/~449MiBRSS, te same996139vertices/1993410tri/wymiary.19 nadalOutputTooLarge11,497s; sam minimalny pełny zapis przekracza25MiB. Program diagnostyczny nie generuje STL ani transformu metrów/Y-up.
- `spatial29-exact-api-price.json`:1PASS46,1s — pełny multizbiór bitów zorientowanych faces/STL i niezależny rachunek fixture; przed dodaniem wyłącznie dolnej granicy rozmiaru. `spatial29-ui-price.json`:1PASS72,2s po tej kontroli;1993410 widocznych tri/zeroArea0,4ujęcia obejrzane, UI4178gr =koszyk serwera. Wcześniejszy osobny pomiar4180gr nie jest stałą oczekiwaną ceną.
- `spatial-default-models.json`:8PASS7,9s; geometry42PASS/1opcjonalnyignored; app/example build, Clippygeometry/APIall-targets, fmt/diff PASS. `spatial29-final-exact-api.json`: 1 PASS, 46,1 s na końcowym kodzie — pełne porównanie geometrii i wycena 4178 gr.

## Następne kroki
1. Model 19: bezpieczny podgląd uproszczony wdrożony; szczegóły i weryfikacja poniżej. Pełny model pozostaje źródłem produkcji, wymiarów i oceny ręcznej.
2. Model 41: niezależnie potwierdzono przekroczenie budżetu zespołu; kontrola przed SDK wdrożona, końcowe wyniki poniżej. Nie oznacza to obsługi modelu ani potwierdzenia pełnej poprawności pakietu.
3. Dokończyć dowody nowych cen/ilości/zasobów i audyt całego celu. Weryfikować wybrane zmiany, bez powtarzania zielonych zestawów bez powodu.

Historyczny szczegółowy plan i poprzednie przebiegi: `cf060a3:docs/plans/large-models.md`; surowych modeli/raportów nie usunięto. Komendy:
```sh
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=spatial29-ui-price python frontend/tests/run-integration.py --large-models corpus-ui.spec.ts --grep 'corpus GUI 29:'
E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=spatial29-final-exact-api python frontend/tests/run-integration.py --large-models large-profile.spec.ts --grep 'corpus 29:'
```

Review Sol xhigh: brak konkretnego blokera tej zmiany. Oceny /10: poprawność 9, bezpieczeństwo 9, kwoty 9, zasoby 8, błędy 9, współbieżność 9, dowody 9, prostota 8, utrzymanie 8, zgodność wizualna 9. Review bez ponownego uruchamiania testów; nie jest oceną całego celu.

## Model 19 — podgląd uproszczony

Pełny model ma 3 861 000 trójkątów. Sam zapis bezstratny przekracza limit GLB, a renderer przyjmuje do 2 mln trójkątów. Dodano `meshopt 0.6.2`: tylko podgląd, po nieudanej próbie bezstratnej albo przekroczeniu limitu renderera. Profile i limity zasobów pozostają bez zmian.

- Slicer dostaje pełny kanoniczny STL; algorytm nie modyfikuje źródłowej siatki.
- Cel: do 1,7 mln trójkątów (dla mniejszych wejść 75% liczby źródłowej); dopuszczony wynik do 2 mln i 25 MiB. Nie podnosimy tolerancji, żeby wymusić sukces.
- Metryka błędu biblioteki: najwyżej 0,05 mm. To **nie jest gwarancja maksymalnej odległości powierzchni**. Granice otwartych siatek są blokowane; kotwice części i ekstremów oraz kontrola spójności/braku znikających lub połączonych części chronią wynik.
- GLB oznacza uproszczenie, liczbę źródłowych/wyświetlanych trójkątów i zmierzony błąd. UI pokazuje uproszczenie i obie liczby w dotychczasowym podpisie, bez zmiany stylu.
- Legacy analiza nie oblicza masy z uproszczonego GLB: zwraca brak masy. Model 19 przekracza 500 mm, więc cena wymaga oceny ręcznej; nie udajemy automatycznej wyceny.

Dowody: `approx19-api-final.json` — 1 PASS / 31,4 s: rzeczywisty upload, prywatny GLB, pełny STL 193 050 084 B / 3 861 000 trójkątów, oryginał SHA, wymiary, właściciel, brak masy z uproszczenia i 422 `manual_quote_required/size_limit`. Wynik ma 1 812 626 trójkątów, metryka 0,049854316 mm. Pierwszy test API zatrzymało stare oczekiwanie węzła bez metadanych; poprawiono test i dodano kontrolę metadanych. `approx19-ui-confirmed.json`: 1 PASS / 44,6 s — rzeczywisty upload, niezależny odczyt GLB kontra liczba renderowanych trójkątów, piksele/zoom, 1440/390 px, oba motywy oraz ręczna wycena po wyborze wszystkich opcji. Cztery obrazy zapisano; desktop ciemny i mobile jasny obejrzano. To emulowane szerokości, nie pomiar telefonu.

Prywatny eksperyment przed integracją: 15,6 s / 468 MiB RAM, 81 części zachowanych; nie zastępuje pomiaru pełnego procesu API. Nowy test mniejszej siatki łapie wcześniej pomijany fallback poniżej 1,7 mln trójkątów. Pozostaje model 41 i pełny audyt celu.

Końcowe kontrole: `approx-glb-parser.json` 25 PASS / 1,6 s; `approx19-standard-models.json` 8 PASS / 7,7 s. Rust/frontend build i końcowy Clippy PASS; 4 testy modułu preview po poprawce PASS. Początkowe błędy testów API/UI są zachowane i opisane w raporcie. Cache kompilatora po ENOSPC: usunięto tylko stare zamknięte katalogi incremental pod blokadą; prywatny zapis w cache-cleanup.json, bez plików użytkownika lub sekretów.

Końcowy review Sol xhigh: bez kolejnego konkretnego blokera. Oceny /10: poprawność 8, bezpieczeństwo 9, kwoty 8, zasoby 8, błędy 8, współbieżność 8, dowody 9, prostota 7, utrzymanie 7, zgodność wizualna 9. To zakres modelu 19, bez deklaracji pełnego 10/10 lub testów obciążeniowych. Reviewer nie powtarzał testów.

## Model 41 — wcześniejsze sprawdzenie budżetu zespołu

Niezależny licznik XML i grafu build/components, z kontrolą SHA, wykazał 16 instancji siatek: 6 257 906 trójkątów, STL 312 895 384 B. Wszystkie 14 wpisów build ma printable=1. Największa pojedyncza siatka ma tylko 955 064 trójkąty — limit przekracza cały zespół. Archiwum rozpakowuje się do 522 098 543 B. Licznik nie potwierdza pełnej poprawności semantycznej/topologicznej. Prywatny pomiar: 8,605 s / 8,602 CPU / 27 908 KiB, bez sieci/sekretów, w osobnym profilu diagnostycznym; nie jest pomiarem produktu.

`geometry/three_mf_budget.rs` tworzy ograniczoną wskazówkę z istniejącego, sprawdzonego małego root XML i już odczytywanych części. Liczy tylko osiągalne siatki, z wielokrotnością instancji; pomija nieużywane obiekty. Rozpoznaje przestrzenie nazw i używa wspólnego resolvera ścieżek. Limit metadanych 4096 obiektów/referencji, głębokość 64, obliczenia nasycane. Niejasny graf/nazwy/cykl/DTD wyłączają wskazówkę. Każdy kandydat mieszczący się w limicie nadal przechodzi cały dotychczasowy walidator. Jedynie udowodnione przekroczenie daje wcześniej TooManyTriangles; błąd budżetu może poprzedzać wykrycie innej wady pakietu.

Pierwszy rzeczywisty test wykazał nadal 503 po 25,2 s: poprawne referencje tekstowe &amp; w metadanych wyłączały wskazówkę. Dodano obsługę pięciu predefiniowanych i poprawnych liczbowych referencji tekstowych oraz test regresji; DTD/niestandardowe encje nadal wracają do walidatora. Oczekiwania testu pozostawiono: jawne 422 i działający kolejny upload, bez uznawania 503 za sukces.

Kontrole po poprawce: 50 testów geometrii PASS / 1 wcześniej opcjonalny ignored, Clippy geometry all-targets, format i build aplikacji PASS. `budget41-api-fixed.json`: 3 PASS / 103,9 s — model 41 daje 422 model_resource_limit i następny poprawny upload działa (13,4 s razem); model 19 zachowuje pełny STL/podgląd/ochronę masy/ścieżkę ręczną (37,1 s), model 20 przechodzi slicer i niezależny rachunek ceny (52,7 s). `budget41-ui.json`: 1 PASS / 27,6 s — rzeczywisty formularz, ten sam jawny błąd, brak udawanego podglądu lub ceny, działające kontrolki; test potwierdza też odrzucenie osobnym zapytaniem HTTP. Limity, zależności, .env i wygląd pozostają bez zmian; model 41 nadal wykracza poza profil large.

Review Sol xhigh po poprawce encji: brak konkretnego blokera. Oceny /10: poprawność 8, bezpieczeństwo 9, kwoty N/A, zasoby 8, błędy 9, współbieżność 9, dowody 9, prostota 7, utrzymanie 7, wygląd N/A. Dodatkowy skan XML ma koszt: dwa przyjęte duże przypadki mieszczą się w limicie, bez gwarancji dla dowolnego pliku/sprzętu. Reviewer nie uruchamiał testów ponownie; nie jest to ocena całego celu.

Regresja zwykłego profilu po kontroli zespołu: `budget41-standard-models.json` 8 PASS / 7,6 s — wszystkie formaty, odrzucenia, retencja, izolacja klientów/admission i upload ponad dawnym limitem 25 MiB.


## Lokalny serwer — 6 listopada

`http://localhost:18430/wycena` obsługuje worktree `codex/corpus-quality`, API 18432 z profilem large. Przed aktualizacją zachowano poprzednią binarkę, pg_dump i pliki; strukturę dumpa sprawdzono pg_restore --list (nie wykonano odtworzenia). Migracje 22–23 zastosowane, dane głównego checkoutu i `.env` bez edycji. Pierwszy check kopii nie przekazał stdin do kontenera i uruchomił rollback; następny check z -i i aktualizacja przeszły.

Rzeczywisty formularz: model 8, 78 387 984 B → 201, prywatny GLB 19 466 312 B / 1 567 758 trójkątów, wymiary 100×190×58 mm, geometria widoczna na obejrzanym zrzucie; usunięcie własnego testowego uploadu 204. Prywatny zapis smoke-fixed.json. Pierwszy smoke przy 201 stracił body odpowiedzi w cache inspektora; poprawiono harness na odczyt ID ze szkicu i metadata GET. Jego poprzedni upload pozostał bez cookie do naturalnej retencji 24 h. Nie uznajemy tego pierwszego testu za PASS.

Nie powtarzano tu slicera/Stripe/email; ich izolowane dowody opisano osobno. Wspólny backend/target jest symlinkiem do głównego checkoutu: to współdzielona binarka, nie izolowany katalog kompilacji; jej SHA zapisano w prywatnych metadata. Kolejny start przez launcher wymaga jawnego `MODEL_PROFILE=large python backend/run.py`, inaczej wróci standard. Backup i logi są prywatne/ignorowane.

Scoped Sol xhigh bez blokera dla upload/podgląd/usunięcie: poprawność 8, bezpieczeństwo 8, kwoty 7, zasoby 7, błędy 7, współbieżność 7, dowody 8, prostota 7, utrzymanie 7, wygląd 8. Bez ponawiania testów przez reviewera i bez oceny całego produktu.
