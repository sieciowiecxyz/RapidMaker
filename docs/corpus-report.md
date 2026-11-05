# Modele i E2E — checkpoint 2026-11-05

## SVG — geometria, rysowanie i brakujące grafiki

`svg-public-presence-final.json`: **6 PASS/55,2s**,0 skip/flaky.56 kombinacji trasy/motywu/szerokości,246 powtarzanych kontroli rysowania małych SVG (nie246 unikalnych ikon),4 kontrole samej geometrii dużego tła. Testy publicznych i zalogowanych tras zachowują dotychczasowe sprawdzanie obrazów/fontów/układu. Puste SVG, wszystkie SVG ukryte i brak SVG są celowo odrzucane; minus z poprawną kreską przechodzi mimo zerowej wysokości bbox samej linii. Minimum widocznych rysowanych SVG: storefront20, admin11 desktop/10 mobile, pozostałe trasy1.

Helper porównuje ten sam SVG przed/po ukryciu jego potomków na nieprzezroczystym diagnostycznym tle, co wyklucza ruch innej grafiki pod przezroczystym elementem. Przywraca tymczasowy atrybut i styl. To dowód wkładu pikseli w tych warunkach, nie kontrastu na rzeczywistym tle, dokładnej tożsamości każdej ikony ani poprawności wszystkich ścieżek wewnątrz SVG. Powyżej600 px w którejkolwiek osi sprawdza tylko geometrię. Limity160 korzeni/2000 kształtów na korzeń. Bez zmian produktu/wyglądu/zależności.

Pierwsze dwa przebiegi6 PASS47,1/47,4s poprzedzały finalne minimum obecności; Sol wskazał możliwy fałszywy PASS po ukryciu wszystkich SVG. Poprawiono minimum i dodano negatywne kontrole przed końcowym przebiegiem. Stricttsc/Prettier/diff-check PASS. Sol xhigh finalnie bez blokera, oceny8–9/N/A; nie jest to zamknięcie całego celu.

## Dwie sesje panelu i konflikt ceny

`manual-publication-concurrent.json`: **1 PASS/29,5s**,0 skip/flaky. Rozszerzenie istniejącego scenariusza loguje ten sam admin-account w dwóch niezależnych kontekstach przez prawdziwe MFA. Oba panele mają wersję3; równoległe kliknięcia wysyłają35/40 zł. HTTP200/409, klient widzi wersję4 ze zwycięską ceną, oba panele ją odczytują, przegrany pokazuje konflikt; outbox: stare wersje skipped, jedna aktualna pending. Bez zmiany produktu, płatności i SMTP. Czas obejmuje oczekiwanie na następny krok TOTP, bez obchodzenia ochrony replay.

To dowód konfliktu nieaktualnej wersji przez dwie sesje jednego konta, nie dwóch różnych użytkowników ani wymuszonego nałożenia transakcji PostgreSQL. Sol xhigh: brak blokera, oceny8–9 w większości kategorii; wykonanie potwierdzone przez głównego agenta po review. Prettier/diff-check PASS.

## Publikacja oferty po zerwaniu połączenia

Nowy rzeczywisty test odtworzył utratę komunikatu i zatwierdzonej próby po błędzie sieci. `OrderDetail` odświeża teraz tę samą ofertę bez odmontowania formularza; inicjalne ładowanie nadal czyści dane, a401/403/404 usuwa prywatny widok. Nowsza wersja oferty rozstrzyga wynik publikacji i usuwa nieaktualny komunikat błędu. Bez zmian CSS.

`manual-publication-final.json`: **2 PASS/29,3s**,0 skip/flaky —nowa regresja publikacji i istniejąca macierz wizualna w jednej bazie. Przed zapisem test przerywa POST bez wysłania go do API: wersja1/bez emaila, zachowane pola i komunikat; retry wysyła identyczną zatwierdzoną treść bez nowego dialogu→wersja2/25 zł/jeden wpis outbox. Następnie API rzeczywiście zapisuje30 zł, ale test gubi samą odpowiedź: odczyt rozpoznaje wersję3 bez kolejnego POST; stara wiadomość skipped, nowa pending, bez duplikatu. Prawdziwy logout admina i refresh usuwają ofertę/zakres z UI; API zwraca401. SQL tylko odczytuje własną walidowaną bazę/UUID; SMTP i płatności wyłączone. To dowód kolejki, nie doręczenia emaila ani równoległej publikacji przez dwóch adminów. Test należy do core.

Początkowy FAIL21,3s potwierdził błąd produktu. Po poprawce nowa regresja przeszła, ale wspólny przebieg znalazł zbyt szeroki selektor starego testu przy dwóch zamówieniach z tym samym plikiem; zawężono go do właściwego numeru zamówienia. Końcowe dwa testy przeszły razem. Svelte-check0/0, Vite build, Prettier i diff-check PASS. Sol xhigh: brak blokera, oceny8–10 w zakresie zmiany; bez deklaracji10/10 całego celu.

## Oferta indywidualna — błędy i stany wizualne

Nowa regresja ujawniła znikający komunikat błędnej ceny w `/admin/zamowienia/{id}`: lokalna walidacja odświeżała zamówienie i odmontowywała formularz. Jedna zmiana warunku w `ManualOffer.svelte` zachowuje komunikat i wpisane pola przed publikacją; po próbie API odświeżenie pozostaje. Bez zmian CSS/wyglądu.

`manual-offer-visual-final.json`: **1 PASS/25,5s**,0 skip/flaky;72 PNG, w tym28 nowych kart i manifest `manual-offer-matrix.json`. Macierz1440/390 × light/dark: klient oczekujący i admin z ceną0/dostawą większą od ceny (12 kart); rzeczywiście opublikowana oferta25 zł/dostawa0 (8); wygasła oferta (8). Sprawdza zachowanie pól, stan API, kwoty/zakres, przyciski i granice kontrolek. Wygaśnięcie wymuszone tylko dla własnego UUID w walidowanej bazie `rapidmaker_test_browser_*`. Nie inicjuje płatności, nie dowodzi HTTP odmowy płatności po wygaśnięciu ani odbioru emaila. Dostawcy wyłączeni; SMTP/Stripe mają osobne dowody.

Pierwszy przebieg FAIL30,9s potwierdził błąd produktu. Kolejny FAIL21,3s wykazał błędne założenie testu o endpointcie GET admin/orders/{id}; poprawiono na istniejącą listę z kontrolą200/UUID. Po usunięciu zbędnych ujęć skorygowano licznik32→28 przed końcowym przebiegiem. Nie zmieniano API dla testu. Wszystkie28 kart obejrzano w4 zestawieniach, trzy mobilne także w rozmiarze natywnym: tekst/kwoty/przyciski czytelne, brak zaobserwowanego obcięcia poziomego. Pole tekstowe ma własne przewijanie. Zestawienia w prywatnym podkatalogu `overview/`.

Svelte-check0 błędów/ostrzeżeń, Vite build, Prettier i diff-check PASS. Sol xhigh po poprawkach: bez konkretnego problemu; oceny6–9/10, nie jest to10/10 całego celu ani test współbieżnej publikacji.

## Tożsamość pakietu slicera i aktualność ceny

Hash obejmuje teraz całe stałe drzewo `/opt/bambustudio-bin`, w tym zasoby, względne ścieżki, typy, uprawnienia i zawartość. Zmiana zasobu przy tym samym launcherze/ELF zmienia tożsamość po restarcie; wykryta zmiana w działającym procesie blokuje kolejne użycia do restartu. Wymagane pliki muszą mieć bit wykonywania. Systemowe zależności poza pakietem nadal nie są hashowane; to nie jest atestacja ani ochrona przed celową podmianą w trakcie wykonania. Limity i złożoność: [slicer-queue.md](slicer-queue.md).

5 testów jednostkowych PASS (zasoby, kolejność, restore, mode, limity, dowiązania/pliki specjalne); opcjonalny test rzeczywistej instalacji uruchomiono osobno:1 PASS, zimny odczyt6,905s,10 ciepłych kontroli101,117ms. Hashowanie/skanowanie przeniesiono do `spawn_blocking`; pierwsze równoległe użycia mogą oczekiwać na inicjalizację. Test nie modyfikował `/opt`.

`bundle-runtime-pricing.json`: **4 PASS/24,5s**,0 skip/flaky —realny konfigurator/slicer,1/10/1000 sztuk i niezależna arytmetyka na jawnych stawkach testowych, wielomodelowy checkout, za duży model→ręczna wycena. Istniejąca regresja zmienia hash wyłącznie w izolowanej bazie: brak/inny hash→409, przywrócony→200. Nie jest to benchmark rentowności rzeczywistych kosztów firmy. Build, Clippy API/slicer all-targets -D warnings oraz fmt/diff-check PASS; wcześniejszy warning wygenerowanego CXX nadal obecny.

Sol xhigh wskazał brak mode w hashu i wczesnej kontroli wykonywalności —oba poprawione i przetestowane; finalnie bez konkretnego blokera, oceny7–9/N/A (dowody E2E sprawdzone przez agenta głównego po review). Frontendu/style nie zmieniano, korpusu i dostawców nie powtarzano.

## Dodatkowe rzeczywiste testy DB/API

`database-with-api-final.json`: **20 PASS/0 FAIL/0 skipped/0 flaky** —18 testów storage, katalog API oraz HTTP: klient/gość/admin nie mogą przekraczać granic cudzych modeli, zamówień i operacji. Każdy test ma osobną tymczasową bazę, access także własny katalog modeli; sprzątanie jest częścią wyniku. Runner jest w grupie `storage` i `test:all`, bez kluczy dostawców. Z25 pomijanych testów workspace wykonano osobno20; pozostałe5 nie są zaliczone jako wykonane.

Pierwszy przebieg19 PASS/1 FAIL ujawnił nieaktualną asercję katalogu: PA-CF ma jawny czarny kolor mimo pustej listy upstream. Test teraz sprawdza dokładny obiekt tego fallbacku i pełne listy pozostałych kolorów, nadal kontrolując cenę/dopuszczenie operatora i niezależność od magazynu. Kod produktu nie zmieniony. Sol xhigh: brak konkretnego problemu, oceny8–9/N/A; fmt/diff-check PASS. Nie powtarzano korpusu, UI ani dostawców po zmianie wyłącznie testów/runnera.

Pobieranie zamknięte decyzją użytkownika 2026-11-05: **41 modeli wystarczy**. Pobrano41 różnych projektów MakerWorld: 34 × 3MF, 7 × STL; bez STEP z MakerWorld. Oryginały, warianty nazw i manifest SHA-256 pozostają lokalnie w ignorowanym `backend/test-models/makerworld/`. Licencje nie są w pełni zweryfikowane; nie publikujemy plików.

## Aktualny korpus i testy bazodanowe — HEAD8bd6fcc

`corpus41-trimmed.json`: **36 PASS/5 FAIL/424,3s**,0 skipped/flaky —20 rzeczywistych cen,11 ręcznych,5 oczekiwanych413. FAIL14/19/20/40: kontrolowane limity;32: błędne metadane. To pełny aktualny przebieg po ostatnich poprawkach parsera, nie suma wcześniejszych regresji. Decyzja o osobnym profilu dużych modeli albo pozostaniu przy STL50 MiB jest pytaniem do użytkownika; w tej zmianie budżetu nie rozszerzano.

`cargo test --offline --locked --workspace`: **139 PASS/25 ignored**. Nowy `python frontend/tests/run-all.py --group storage --no-build` uruchamia18 pomijanych testów storage pojedynczo, w18 nowych tymczasowych lokalnych DB. `storage.json`: **18 PASS/0 FAIL/0 skipped** —MFA/sesje/hasła/passkey, kolejka, własność/retencja, rozmowy/outbox, checkout, zwroty i logistyka. Nie zaliczamy pozostałych7 ignored z workspace jako wykonanych. Runner jest częścią `test:all`; standalone nie wymaga frontendu/Chromium. Nie przekazuje sekretów dostawców, tylko URL własnej bazy.

Oddzielny kontrolowany SIGTERM (symulowany proces Cargo + prawdziwa lokalna DB) potwierdził zatrzymanie childa, usunięcie własnej DB i prawidłowy FAIL przerwanego przebiegu. Nie jest dodatkowym PASS logiki biznesowej. Logi0700/0600; nieudany DROP zachowuje nazwę własnej bazy w prywatnym raporcie, bez URL/sekretów na stdout. Sol xhigh: brak blokera po poprawce diagnostyki cleanup, oceny8–9/N/A, dowody9 po wykonaniu. Python syntax/diff-check PASS.

Świeży audyt: Cargo0 podatności/1 `pin-utils` unmaintained przez async-imap0.12.0; npm3 low (jeden problem deepmerge, bez poprawki). Nie wyciszano zgłoszeń ani nie dodano forka: [security.md](security.md). Podczas zapisu zabrakło miejsca na dysku; odzyskano raport z Git i usunięto wyłącznie671501112 B zakończonych własnych binariów testowych. Wyniki/modele zachowane, raport odzyskany przed commitem.

## Bieżąca regresja całości — wcześniejszy HEAD97c10dd

`core.json`: **31 PASS/237,4s**,0 FAIL/skipped/flaky — realne API/DB/parser/slicer, wyceny i koszyk, współbieżność, własność/retencja, konfigurator, motywy/zasoby/przykłady, panele z danymi i ustawienia bezpieczeństwa. `contracts.json`: **21 PASS/5,7s**,0 FAIL/skipped/flaky — osobny przebieg kontrolowanych kontraktów UI/narzędzi. Nie jest to jeden wspólny52-testowy run ani nowe potwierdzenie Stripe/Furgonetki/iCloud; aktualne regresje dostawców mają własne wpisy niżej. Oba przebiegi wykonano po zmianach zwrotów/emaili/lokalnych potwierdzeń i usunięciu martwej makiety.

Uaktualniono checkpoint i sprzeczne historyczne opisy w email/chat/checkout: interfejsy i odpowiedzi email nie są już makietami, sandbox obejmuje zwroty i logistykę. Sol xhigh zweryfikował źródła liczb i wskazał te rozbieżności; zachowano pełny zakres obrazów/przykładów i klikalnych przepływów. Roboczy audyt całego celu nadal8–9/10 (zasoby7), nie żądane10/10; siedem nieobsłużonych modeli pozostaje jawne.

## Mniejsza powierzchnia parserów

Usunięto nieudostępniane przez stronę wejściowe OBJ/PLY/glTF/GLB: **494 linie netto** w geometrii, w tym435 produkcyjnych. Pozostają STL/STEP/STP/3MF i wyjściowy GLB. Lockfile odchudzono o16 pakietów (m.in. kodeki obrazów/PLY/OBJ), bez dodanych pakietów i zmian wersji. Bezpośredni `gltf` jest testowy i nadal niezależnie sprawdza writer; pozostaje też zależnością produkcyjną lib3mf, więc nie twierdzimy, że zniknął całkowicie. Usunięto niepotrzebne features import/image.

Naprawiono ignorowanie statusu `Unsupported` w obu wejściach biblioteki: `.exe` z poprawnym STL i plik bez rozszerzenia nie są już parsowane jako STL. HTTP wcześniej miało własną prawidłową kontrolę trzech formatów; nie była to luka w publicznym uploadzie. Nowe negatywne regresje zastępują wyłącznie testy usuniętych formatów; STEP/3MF manual gate nie blokuje konwersji, uppercaseSTP zachowane.

Geometry31 PASS/1 opcjonalny ignored; workspace check i build `--offline --locked`, Clippy all-target `-D warnings`, fmt/diff-check PASS. `geometry-formats-trim.json`: **5 PASS/4,2s**,0 FAIL/skipped/flaky — rzeczywiste3 formaty→prywatnyGLB, odrzucenia/ścieżki zewnętrzne, retencja, własność/równoległość i limity. Sol xhigh bez blokera; oceny zakresu8–9/N/A. Frontend, styl i limity pozostają bez zmian. Pełny korpus i dostawcy nie były powtarzane w tej regresji.

## Pozostałe sceny i szybkie odrzucenie błędnego manifestu

Izolowana diagnostyka SHA-zweryfikowanych plików rozwinęła build/komponenty/instancje i ścieżki Production (CPU10s/AS512 MiB/wall15s, bez sieci/sekretów, bez zapisu modeli). Nie ma nieużywanych trójkątów, których usunięcie zmieści te cztery sceny w obecnym STL50 MiB:

| Przypadek | Trójkąty po instancjach | Wynikowe bajty STL |
| --- | ---: | ---: |
| 14 | 1953958 | 97697984 |
| 19 | 3861000 | 193050084 |
| 20 | 1487438 | 74371984 |
| 40 | 1677324 | 83866284 |

To liczenie referencji, nie walidacja współrzędnych/topologii ani udana konwersja.19 ma tylko1608124 surowych trójkątów, lecz instancje zwiększają wynik. Prywatne skrypty/wyniki: `/tmp/rapidmaker-reachable-DPB0a6/`.32 zapisano przez `BambuStudio-02.08.02.61`, ale jego `3DManufacturing:3mfVersion` nie ma deklaracji i nie jest poprawnym QName. Wymaganie nazwy: [3MF Core, metadata](https://github.com/3MFConsortium/spec_core/blob/master/3MF%20Core%20Specification.md). Nie naprawiamy jej przez zgadywanie lub usunięcie metadanych.

Zamiast zmieniać walidację dodano ograniczony preflight małego rzeczywistego manifestu: max64 KiB relacji/root, dokładnie jeden kanoniczny wybór OPC, istniejąca normalizacja odbić/malowania i publiczny parser strumieniowy lib3mf. Pełne sprawdzenie pakietu nadal obowiązuje. Inny/duży/zakodowany/niejednoznaczny root pozostawia poprzednią ścieżkę; test alternatywnego root z błędnym nieużywanym kanonicznym plikiem zachowuje ten sam STL.

`metadata-root-fast-reject.json`: **1 PASS/1,3s**, ciało scenariusza633ms; wcześniej ten sam32 miał22,8s. To obserwowane porównanie E2E, nie gwarantowany benchmark. GUI wymaga422 z dokładnym błędem, bez ceny i możliwości zakupu po wypełnieniu opcji. Geometry33 PASS/1 istniejący opcjonalny ignored; build/Clippy/fmt/diff-check PASS. Początkowy błąd kompilacji porównań quick-xml poprawiono przed testami. Sol xhigh bez blokera, oceny zakresu8–9/N/A; mały dodatkowy odczyt poprawnych manifestów jest ograniczony. Kod/style frontendu bez zmian; pięć plików nadal nie ma wyceny.

## Dwa kolejne modele w istniejącym budżecie STL

Limit trójkątów1000000 był niższy niż rzeczywista pojemność zachowanego STL50 MiB. Backend i prywatny viewer GLB używają teraz `(52428800−84)/50 = 1048574` (następny trójkąt przekracza plik o34 B). Nadal1 mln unikalnych punktów,96 MiB ZIP,25 MiB GLB,2 GiB RAM/25s CPU/30s wall i pozostałe walidacje; bez zmniejszania dokładności lub pomijania geometrii produkcyjnej. Frontowy parser lokalnego STL nadal25 MiB/1M; wspólny builder renderera dopuszcza pojemność STL. Zmiana produktu kilkanaście linii.

`triangle-budget-api.json`: **2 PASS/60,5s**,24 podgląd+ręczna `printer_fit`,39 podgląd+rzeczywisty slicer/cena. `triangle-budget-ui-verified.json`: **2 PASS/58,6s**,0 skip/flaky; oba wymagają201 przed jakimkolwiek fallbackiem, podgląd/piksele/zoom w1440/390 i obu motywach. Zapisano8 scen; obejrzane jako zestawienie, prywatne `triangle-budget-overview/sources.json`/`all8.png`.24 ma rozproszone części w globalnych współrzędnych; nie przestawiano ich dla wyglądu.

| Przypadek | Trójkąty kanonicznego GLB | Wyświetlone | Zerowe pola policzone niezależnie z GLB |
| --- | ---: | ---: | ---: |
| 24 | 1033695 | 1033689 | 6 |
| 39 | 1046124 | 1046109 | 15 |

Pierwszy strict UI2 FAIL wymagał błędnie równej liczby wszystkich trójkątów GLB i renderera, który od dawna pomija zerowe pola. Skorygowany oracle liczy je niezależnie i nadal wymaga ponad1M wyświetlonych; bez zmiany produktu dla testu. Geometry30 PASS/1 istniejący opcjonalny ignored, backend/frontend build0/0, Clippy/diff-check PASS. Sol xhigh sprawdził granice, dodatkową alokację typed arrays(~5,25 MB ponad stary limit) i ścieżki parserów; brak konkretnego blokera po poprawkach testu, oceny zakresu7–9/N/A. Limit procesu jest nadal egzekwowany.

Odrębna diagnostyka siedmiu plików (bwrap bez sieci/sekretów, CPU10s/AS512 MiB/wall15s) potwierdziła SHA i policzyła surowe zasoby XML.14/19/20/40 mają surowe wyjście STL~71–93 MiB i ZIP ponad96 MiB, lecz nie sprawdzono osiągalności/instancji, więc to nie dowód minimalnego rozmiaru rzeczywistego builda.32 pozostaje błędnym pakietem. Pełny przebieg41 niżej pochodzi sprzed zmiany trójkątów; nie deklarujemy nowego pełnego36/5 na podstawie dwóch regresji.

## Pełne41 — ZIP96 MiB, poprzedni limit1M trójkątów

`corpus41-96.json` na `23ecfbc` (zmiany testu wizualnego nie dotyczą tego przebiegu): **34 PASS / 7 FAIL**,419,5s,0 skipped/flaky. Bez zmiany asercji201 dla nieobsługiwanych modeli.

| Wynik | Liczba | Przypadki |
| --- | ---: | --- |
| Automatyczna cena z rzeczywistego slicera | 19 | 1,3,4,6,7,9,10,11,12,13,17,23,26,27,30,33,34,35,38 |
| GLB i ręczna wycena | 10 | 2,5,15,16,18,22,25,28,31,36 |
| Oczekiwane413: upload ponad32 MiB | 5 | 8,21,29,37,41 |
| FAIL:422 `model_resource_limit` | 6 | 14,19,20,24,39,40 |
| FAIL:422 nieprawidłowy model | 1 | 32 — błędne metadane, bez obchodzenia walidatora |

W porównaniu z bazą64 MiB dochodzą trzy ceny i jeden podgląd/ręczna wycena, brak innych zmian klasyfikacji.24 i39 mają obecnie kontrolowane422 zasobowe; historyczne exit137 dla24 pochodziło z osobnej diagnostyki, nie obecnego API. Sześciu ograniczeń zasobowych i błędnego pliku nie nazywamy działającą wyceną. Poniższe wcześniejsze przebiegi zachowują własne źródła i zakresy.

## Rozszerzenie większych modeli — 96 MiB

Decyzja użytkownika: rozszerzyć obsługę większych modeli. Limit rozpakowanego ZIP i pojedynczego wpisu64→96 MiB;512 wpisów,1 mln wierzchołków/trójkątów,2 GiB RAM,25s CPU/30s wall i limity uploadu/GLB pozostają. Dev kompiluje tylko geometry/lib3mf z opt-level2, zachowując debug assertions i kontrolę przepełnień. Granice ZIP96 MiB/+1 i48+48 MiB/+1: geometria29 PASS/1 istniejący opcjonalny ignored, fmt/Clippy/build PASS.

`expanded96-optimized.json`: rzeczywisty API/slicer,7 przypadków,165,2s, **4 PASS / 3 FAIL**, bez skips/flaky. Nowe ceny:7/13/38;31 ma podgląd i ręczną wycenę z powodu gabarytu.24/39 nadal422 `model_resource_limit`;32 mieści się w limicie bajtów, ale ma błędny prefiks metadanych `3DManufacturing:3mfVersion`, więc walidacja go odrzuca. Nie zgadujemy namespace ani nie obchodzimy walidatora. Runner nadal wymaga201: trzech odrzuceń nie zmieniono na zielone wyceny. Wyniki całych41 poniżej są historyczną bazą64 MiB, nie nowym pełnym przebiegiem.

Sol xhigh: brak blokującego defektu w zmianie; wskazał nieaktualny limit w ARCHITECTURE i brak wpisu wyników — poprawiono. Oceny zakresu7–9/10, bez deklaracji10/10 ani gotowości produkcyjnej. `expanded96-ui.json`:7 PASS/194,8s; cztery rzeczywiste podglądy i trzy jawne odrzucenia, również po uzupełnieniu opcji druku brak ceny/checkoutu dla odrzuconych. To wynik obsługi UI, nie siedem udanych wycen. Review testu wykrył możliwość zaakceptowania201 dla wadliwego przypadku32; dodano obowiązkowe422 przed rozgałęzieniem, `expanded96-invalid32.json`:1 PASS/23,5s po tej poprawce. Końcowy przegląd nie wykazał dalszego konkretnego blokera w zmienionym zakresie.

## Koszyk wielu modeli — rzeczywiste wyceny

Nowy test `real multi-model checkout combines measured costs once and rejects altered or foreign quotes`: dwa osobne uploady i pomiary slicera, ilości1/10. Niezależny rachunek jednostek kosztu z testowego cennika, jedno przygotowanie/rezerwa/marża/minimum/VAT, suma pozycji i przygotowania dokładnie równa cenie zamówienia. Odbiór osobisty bez opakowania i dostawy. Podmiana ilości/modelu/cudzych snapshotów →409, duplikat ID→400; prawidłowy koszyk nadal200. `multi-model-pricing.json`:1 PASS/4,6s. Pierwszy FAIL wynikał z braku danych odbiorcy w fixture, poprawiono bez zmiany produktu. Sol xhigh znalazł ten błąd; końcowy przegląd bez dalszego konkretnego problemu, oceny zakresu5–9/10 (współbieżność i UI mają osobne testy, tutaj nie są dowiedzione). Prettier PASS. `core.json`:31 PASS/0 FAIL/0 skipped/0 flaky,234,6s na bazie `c13dfa2` z tym nowym testem. Rzeczywiste API/DB/slicer, konfigurator, współbieżność, prywatność i retencja modeli, widoki obu motywów/rozmiarów, panele z danymi i ustawienia bezpieczeństwa (sesje/hasło/wirtualny passkey/TOTP/recovery). Stripe/Furgonetka/iCloud nie były powtarzane w tej grupie; ich wcześniejsze wyniki są oddzielne.

## Aktualny sandbox i związanie oferty z płatnością

`sandbox.json` na `f60b381`: **9 PASS/168,0s**,0 FAIL/skipped/flaky i brak `provider-blocked`. Rzeczywiste Stripe test/Furgonetka sandbox: odbiór osobisty, kurier/punkt, ponowne użycie ofert, panele/rozmowy/produkcja, częściowy+pełny refund/uprawnienia, nadanie/PDF/tracking/podjazd/anulowanie i przekazanie przewoźnikowi, wygaszanie nieopłaconych uploadów. To działania sandboxu, nie fizyczny przewóz ani publiczne wdrożenie. Ten przebieg nie powtarza iCloud.

Review Sol xhigh znalazł lukę w testach kuriera/punktu: zapłata mogła przejść mimo pomylenia wybranej oferty/kwoty. Wzmocniono istniejące dwa scenariusze bez zmiany produktu: drugi przewoźnik, jeśli dostępny; oferta z rzeczywistej odpowiedzi powiązana z zaznaczoną pozycją; dokładne quote_id/offer_id i kwota żądania/wyniku płatności; kwota i oferta zachowane w trwałym zamówieniu. Utrata odpowiedzi, ponowienie, odrzucona karta i reload nadal sprawdzane. `selected-carrier-payment.json`: **2 PASS/35,3s**,0 FAIL/skipped/flaky, po wzmocnieniu. Nie opisujemy tych osobnych przebiegów jako jednego pełnego9/9 na zmienionym teście.

Końcowy Sol xhigh bez konkretnego defektu. Oceny zmienionych testów: poprawność8, kwoty9, błędy8, dowody9, prostota8, utrzymanie7/10; uprawnienia/zasoby/współbieżność/wygląd N/A dla tej zmiany. Ograniczenia: identyfikacja radio przez kolejność/etykietę (brak wartości w istniejącym HTML), potwierdzenia przez API produktu, bez niezależnego odczytu księgi Stripe. Prettier/discovery/diff-check PASS.

## Równoległe rozpoczęcie nieopłaconej płatności

Rozszerzono istniejący scenariusz porzuconego zamówienia: cztery równoległe POST z identycznym kluczem i parametrami →200/202 i ten sam order_id; ponowienie osiąga `open`. W testowej DB dokładnie jedna próba, jedno powiązane zamówienie i jeden zapisany provider_id. Nowy klucz na zablokowanym szkicu, zmieniona wersja i nieistniejąca oferta →409. To nie dowód dla innej **prawidłowej** oferty z tym samym kluczem ani niezależna inwentaryzacja wszystkich obiektów konta Stripe. Bez obciążania karty. Zachowano sprawdzenie hold plików podczas otwartej płatności i rzeczywistego usunięcia po wygaśnięciu.

`concurrent-unpaid-payment.json`: **1 PASS/5,9s**,0 FAIL/skipped/flaky. Po tym przebiegu zmieniono wyłącznie tytuł testu, aby nie sugerował niezależnego odczytu zamknięcia sesji w Stripe. Sol xhigh bez blokującego problemu; oceny zakresu: poprawność8, bezpieczeństwo7, kwoty7, zasoby8, błędy7, współbieżność8, dowody8, prostota8, utrzymanie8/10; UI N/A. Kod produktu i wygląd bez zmian.

## Równoległe zwroty

Rozszerzono istniejący scenariusz paneli: po zwrocie500 z zapłaconych2500 groszy dwa równoległe żądania1500+1500 z różnymi kluczami → dokładnie202/409 `refund_amount`. Odpowiedź jest związana z przesłanym kluczem, retry zwraca ten sam rekord; ledger ma dokładnie dwa oczekiwane ID. Po potwierdzeniu Stripe suma zwrotów2000, UI pokazuje pozostałe5 zł; końcowy zwrot przez UI daje trzy `succeeded`, dokładnie2500. Nadal sprawdzane klient403, obcy404, overrefund409 i stan zamówienia. Jedna sesja admina wysyła równoległe żądania; nie deklarujemy testu dwóch osobnych adminów ani niezależnego odczytu księgi Stripe.

`concurrent-refund-final.json`: **1 PASS/25,7s**,0 FAIL/skipped/flaky, rzeczywiste Stripe test/API/DB. Wcześniejszy przebieg1 PASS26,1s poprzedzał dodatkową kontrolę klucza. Sol xhigh: brak blokującego problemu, drobny brak kontroli klucza poprawiono i przejrzano. Oceny zakresu8–9/10 w dziesięciu kategoriach; brak twierdzenia10/10 dla produktu. Prettier/diff-check PASS; bez zmian backendu, wyglądu i nowych zamówień poza istniejącą fixture.

## Opłacone i zwrócone zamówienie — naprawiony status klienta

Nowa kontrola prawdziwego zamówienia ujawniła błąd: po pełnym zwrocie klient nadal widział „Opłacone”, bo tylko admin pobiera ledger. Trzy autoryzowane odczyty zamówień zwracają teraz wyłącznie `refunded_grosze`: sumę potwierdzonych `succeeded` w tej samej migawce SQL. Bez nowych uprawnień, identyfikatorów ani szczegółów admina. Adapter sprawdza bezpieczną liczbę groszy ≤ kwoty; pełny zwrot wymaga potwierdzonej płatności i dokładnej równości. Zachowano fallback dla starszej odpowiedzi i odrębny stan realizacji. Review wykrył powiązane cofanie osi realizacji dla `refunded`; poprawiono jeden warunek bez zmiany stylu.

Regresja: po częściowym zwrocie klient API500, po pełnym detal/lista2500, badge „Zwrócono płatność”, zakończona oś ma cztery aktywne etapy; ledger admina dokładnie trzy potwierdzone kwoty, zero pozostałe. `paid-refunded-visual-final.json`: **1 PASS/37,6s**,0 FAIL/skipped/flaky.16 nowych zrzutów (paid/refunded × klient/admin ×1440/390 ×jasny/ciemny), fonty/obrazy/overflow, plus3 wcześniejsze zrzuty scenariusza. Wszystkie16 obejrzano w zestawieniu i reprezentatywne mobilne ujęcie osobno; prywatne `paid-refunded-overview/sources.json`/`all16.png`. To kontrola wskazanych sekcji i kompozycji, nie wszystkich stron/pikseli. Admin crop obejmuje sam ledger.

Rust build PASS; frontend build/svelte-check0 errors/0 warnings, Prettier/diff-check PASS. Sol xhigh: korekty zakresu screenshotu/ścisłych kwot oraz osi realizacji wdrożone i ponownie przejrzane; brak dalszego konkretnego blokera. Oceny produktu w zmienionym zakresie przed końcową regresją7–10/10, bez deklaracji10/10 całego celu. Starszy `paid-refunded-visual` FAIL to dowód błędu; `...-fixed`1 PASS37,2s poprzedza kontrolę osi, końcowy wynik powyżej obejmuje obie poprawki.

## Potwierdzenie zwrotu przez email

Dodano `refund_succeeded` z numerem zamówienia i dokładną potwierdzoną kwotą. Wiadomość wyjaśnia, że uznanie rachunku zależy od banku; nie wysyła się dla pending/failed. Potwierdzenie Stripe zapisuje się niezależnie od kolejki. Trwały rekord `succeeded` bez znacznika powiadomienia jest ponawiany co najmniej po60s; rezerwacja jest utrwalona przed próbą, a outbox + znacznik atomowe. Unikalny klucz dotyczy konkretnego zwrotu, nie całego zamówienia. Pełna kolejka/błędny kontakt nie cofają kwot ani nie blokują innych adresatów. Migracja0022 nie wysyła starych sukcesów wstecz. Dotychczasowe wyciszenie zablokowanych kont pozostaje. To jeden wpis outbox na zdarzenie, nie gwarancja exactly-once SMTP.

Dowody: notifications13 PASS, storage7 szybkich PASS (18 bazodanowych pominiętych w tym poleceniu); osobno na nowej usuniętej po teście DB **2 PASS/0 ignored** dla rezerwacji i nowych powiadomień. Sprawdzono zapełnienie outboxa10000, trwały sukces mimo braku miejsca, restart/60s retry, równoległy CAS/jeden event, błędny kontakt i postęp innego zamówienia, naprawę kontaktu. Build/Clippy all-targets `-D warnings`/fmt/diff-check PASS; znane ostrzeżenie generatora CXX bez zmian.

`refund-mail-journey.json`: **1 PASS/211,1s**,0 FAIL/skipped/flaky. Rzeczywiste iCloud MFA → oferta/email → Stripe test płatność → dokumenty email → częściowy zwrot500 groszy i idempotentny retry → klient API500 + odebrany nowy email z dokładnym5.00 PLN/ref → odpowiedź email widoczna raz w chat. Tylko void@sieciowiec.xyz; brak mocków dostawców i dowodu SMTP exactly-once. Sol xhigh wykrył ryzyko cofnięcia sukcesu przez pełną kolejkę w pierwszym projekcie; przeprojektowano trwałe przekazanie i ponownie sprawdzono. Końcowy review bez nowego blokera, oceny zakresu7–9/N/A, nie10/10 całego celu.

## Usunięte pozostałości makiety

`frontend/src/lib/mock.ts`: **345 linii usuniętych,0 dodanych**. Analiza importów i zależności usunęła wyłącznie nieosiągalne fikcyjne auth/zamówienia/chat/refundy/wycenę. Wszystkie zachowane instrukcje są niezmienione; lokalny koszyk, snapshoty rachunku, opcje i modele pokazowe pozostają. `mock-pruning-smoke.json`:3 PASS38,1s0skip/flaky — rzeczywisty API/slicer/Stripe odbiór, nawigacja/motywy/przykłady, pięć rodzin i koło PLA. Svelte-check0/0, build/Prettier/diff-check PASS. Sol xhigh potwierdził brak aktywnych odwołań do usuniętych eksportów; oceny zakresu9/10, prostota10, współbieżność N/A. Nie jest to ocena całego produktu ani nowy pełny przebieg core.

## Potwierdzenia zakupu bez lokalnej kopii zamówienia

Usunięto zapisywanie prawdziwego zamówienia/kontaktu do makiety w `localStorage`. Gość i konto odczytują własne zamówienie z backendu; płatność nadal wymaga weryfikacji Stripe, test-mode i zgodnej kwoty. Stare `sandboxPayment` wpisy są filtrowane i przepisywane przy dostępnym zapisie; koszyk/opcje/demo pozostają. Szkic kontaktu nadal celowo w przypisanym użytkownikowi `sessionStorage`, minimalny receipt zawiera ID/pozycje/dostawę; bez danych karty/client_secret. Cache ma walidację struktury/limitów; uszkodzony jest ignorowany. Quote fingerprint sprawdzany przed i po utworzeniu płatności; koszyk czyści tylko potwierdzona płatność zgodnej wyceny.

`server-receipt.json`:6 PASS51,5s — realne Stripe odbiór/kurier/punkt, kwoty/idempotencja, własność i fałszywy powrót; przed końcową walidacją cache. `server-receipt-final.json`:3 PASS48,8s po walidacji, w tym trzy uszkodzone cache. Końcowy `server-receipt-recovery.json`:1 PASS22,1s — ten sam realny zakup gościa, reload bez lokalnego receipt, purge starego kontaktu, ID/email z backendu, brak utraconego koszyka. Dodatkowa kontrolowana odpowiedź404 szczegółów dowodzi, że już zweryfikowana płatność nadal jest pokazana; to fault injection, nie test sztucznie postarzonego rekordu. Backend ogranicza szczegóły gościa do24h, podczas gdy potwierdzenie nadal wymaga jego właściciela. Inne błędy nie są ukrywane. Wszystkie przebiegi0skip/flaky; auto fixture wymaga braku pageerror.

Svelte-check0/0, Vite/Prettier/diff-check PASS. Brak zmian szablonów/CSS. Sol xhigh przejrzał usuwanie lokalnego stanu, walidację i przypadek404; brak konkretnego blokera, oceny zakresu8–9/N/A, bez deklaracji10/10 produktu.

## Rzeczywisty korpus — baza64 MiB

`corpus41-entry64`: 41 przypadków,262,2s, bez pominięć i flaky. Porównanie z `corpus41-total64`: przypadek9 uzyskuje automatyczną cenę, brak innych zmian wyników/klasyfikacji. Wcześniejsze poprawki uratowały przypadki5,22 i25.

| Wynik | Liczba | Co potwierdza |
| --- | ---: | --- |
| Automatyczna cena | 16 | Upload, GLB, prawdziwy slicer, dodatnie pomiary i całkowitoliczbowe koszty |
| Ręczna wycena | 9 | Podgląd działa; gabaryt przekracza politykę automatycznej wyceny/P1P |
| Za duży upload | 5 | Prawidłowe HTTP 413 powyżej 32 MiB |
| Limit geometrii | 11 | HTTP 422 `model_resource_limit`; pełna wycena nie została wykonana |

Runner zwrócił **30 PASS / 11 FAIL**, nie „41 działa”. Jedenaście testów nadal oczekuje obsługi, mimo ochronnego odrzucenia. Pozostają jawnie oznaczone `resource-limit`; nie zmieniono asercji, żeby uzyskać zielony raport. Limity: 1 mln unikalnych wierzchołków i 1 mln trójkątów, ZIP 64 MiB łącznie / 64 MiB wpis / 512 wpisów, GLB 25 MiB. Zwykły frontend dodatkowo ogranicza plik do 25 MiB. Duże poprawne projekty MakerWorld mogą być nieobsługiwane.

Przed poprawkami było 19 PASS / 22 FAIL: 13 cen, 4 ręczne, 2 prawidłowe 413, 17 limitów, 3 błędy proxy oraz 2 niewyjaśnione odrzucenia parsera. Model 2 otrzymuje **wycenę ręczną**, nie automatyczną cenę.

## Poprawki i dowody

- Limit pojedynczego wpisu32→64 MiB przy niezmienionych64 MiB łącznie. Model9 (około49,4 MiB jeden wpis/50,9 MiB łącznie,98×100×54mm) przechodzi upload, prywatny GLB i rzeczywisty slicer z rachunkiem kosztów: osobno1 PASS35,2s, potem PASS w całym korpusie. Granice ZIP:64 MiB,64 MiB+1 oraz32+32 MiB i dodatkowy1 bajt. Geometria29 PASS/1 opcjonalny ignored, fmt/Clippy/build PASS. Pozostałe limity i walidatory bez zmian; nie dopuszczono diagnostycznych192 MiB. Duży pojedynczy XML może zwiększać chwilową pracę parsera; proces nadal ma2 GiB/25s CPU/30s wall. Pełne41 jak wyżej; zmienił się tylko model9. Duże podglądy przez formularz sprawdzono osobno, patrz sekcja poniżej.
- Poprzedni etap: łączny limit rozpakowanego3MF50→64 MiB; wpis32 MiB,512 wpisów oraz siatka/wyjście/RAM/czas bez zmian. Model5 zawiera około48 MiB geometrii i dodatkowy PDF; żadnego elementu nie usunięto. Upload/GLB/ręczna wycena przechodzą w9,8s; cały projekt ma1163×864×91mm. Izolowana próba diagnostyczna:9,35s/około183 MiB peak RSS,312275 wierzchołków/637320 trójkątów; to nie pomiar RAM API ani automatyczna cena. Granice rzeczywistych ZIP:32 MiB i32 MiB+1 na wpis,64 MiB i64 MiB+1 łącznie, z unikalnymi nazwami. Geometria29 PASS/1 opcjonalny ignored, fmt/Clippy/build PASS. Tamten przebieg miał29 PASS/12 FAIL; zmienił się wyłącznie model5. Większy dopuszczony rozmiar może zwiększyć pracę parsera; limity procesu nadal obowiązują.
- Poprzedni etap: limit pojedynczego wpisu25→32 MiB przy ówczesnym limicie łącznym50 MiB uratował model25 (43,9 MiB łącznie/28,7 MiB jeden wpis). Upload/GLB/ręczna wycena przechodzą w13s. Izolowana próba:12,7s/około157 MiB peak RSS,248333 wierzchołki/497026 trójkątów,464×534×12mm. Była to ręczna wycena; poprzedni korpus miał28 PASS/13 FAIL.
- Odbicia w 3MF: przypadek22 ma434 wpisy i lustrzaną transformację odrzucaną przez lib3mf. Prywatna kopia normalizuje jej wyznacznik do walidacji, sprawdza dopasowanie i przywraca dokładną macierz; wynikowe odbicie odwraca orientację trójkątów. Oryginał niezmieniony. Limit wpisów256→512; bajty, siatka, RAM i czas bez zmian. Root przechodzi dotychczasową walidację pakietu; części podrzędne zachowują wcześniejszy parser XML. Normalizacja ma kontrole czasu i tworzy kopię XML tylko przy zmianie. Geometria**28 PASS/1 opcjonalny ignored**, Clippy/build PASS; testy pojedynczego/podwójnego odbicia, orientacji/objętości/współrzędnych, małej skali, błędnych macierzy, dopasowania i granicy512/513. Rzeczywisty upload/GLB/ręczna wycena**1 PASS10,1s**; pełne41 jak wyżej. Model obejmuje cały projekt około801×559×35mm, więc nie uzyskał automatycznej ceny. Sol xhigh: dwie uwagi poprawione, brak dalszej ustalonej regresji; oceny7–8/10, część kategorii N/A. Nie oznacza10/10 ani kompletnej walidacji wszystkich części 3MF.
- Bambu `paint_*`/`face_property`: pomijane tylko jako ustawienia trójkątów w kopii geometrycznej. Wierzchołki, transformacje, oryginał i pełna walidacja pakietu pozostają. 21 testów geometrii PASS; model 28 przechodzi do ręcznej wyceny, model 36 po indeksowaniu uzyskuje podgląd i ręczną wycenę. Znaczenie pól sprawdzono w [źródle Bambu v02.08.02.61](https://github.com/bambulab/BambuStudio/blob/v02.08.02.61/src/libslic3r/Format/bbs_3mf.cpp).
- Dokładne indeksowanie wierzchołków STL/3MF: zachowane bity współrzędnych, kolejność i orientacja każdego trójkąta, bez uproszczenia siatki. ASCII kontroluje limity podczas czytania. Geometria23 PASS, Clippy PASS; realne modele3/6 dostają cenę,36 podgląd/ręczną wycenę. Budżety czasu/RAM, trójkątów, ZIP i GLB nie zostały zwiększone. Porównanie całego41: trzy poprawione przypadki, zero nowych FAIL ani zmian klasyfikacji wcześniejszych sukcesów. `indexed-models-ui.json`1 PASS2,7s: pięć wariantów STL/STEP/3MF, prywatne GLB i UI; obejrzany desktop zachowuje styl autora. HashMap ma koszt pamięci/CPU; nie jest to dowód działania każdego dużego modelu. Sol xhigh: bez blokera w tym zakresie, oceny8–9/10.
- Model 4 wcześniej przekraczał 60 s CPU w 6,45 s czasu rzeczywistego. Slicer ograniczony do dwóch dozwolonych CPU przeszedł dla PLA/PETG i przez API. Nie podniesiono limitów; polityka `cpu2` unieważnia stare snapshoty.
- Proxy Vite zwraca 413 zamiast 500 dla odtworzonego EPIPE przy zbyt dużych uploadach. Trzy regresje PASS. Obsługa nie obejmuje EPIPE bez deklarowanego Content-Length; zwykłe błędy proxy nie są maskowane.
- Poprawiono zakres CSS etykiet dostawy i fałszywe ostrzeżenie o braku analizy mimo oszacowanych paczek. Zachowano styl autora. Test wyboru kurierów kończy przewijanie przed normalnym kliknięciem, bez force/retry.

## Pozostałe sprawdzone przepływy

| Raport prywatny w `frontend/tests/artifacts/` | Wynik i zakres |
| --- | --- |
| `email.json` (źródło `8dbdb07`) | **2 PASS/213,2s**, bez pominięć/flaky: prawdziwe iCloud MFA(hasło/email, wirtualny passkey/email), admin TOTP/uprawnienia oraz oferta ręczna→email→Stripe test→warunki→jedna odpowiedź email widoczna w chacie |
| `security-current.json` | 2 PASS: błędne dane, zmiana hasła, passkey/TOTP, sesje |
| `auth-email-current.json` | 1 PASS: prawdziwy iCloud do void, hasło/email, wirtualny passkey/email, admin TOTP i klient → admin 403 |
| `commerce-current.json` | 5 PASS: Stripe test, dostawa sandbox, panele/chat/pliki, częściowy/pełny/nadmierny zwrot, cennik |
| `checkout-final-current.json` | 3 PASS: dokładna kwota oferty/przycisku/płatności, odbiór/paczkomat/kurier; obejrzane podsumowania desktop/mobile |
| `registration-password.json` / `registration-passkey.json` | po1 PASS32,8s/28,7s: nowe konto, rzeczywisty email iCloud i kopia warunków, pierwsze hasło lub wirtualny passkey |
| `manual-email-chat-current.json` | 1 PASS: ręczna wycena w UI, publikacja admina, prywatny podgląd, email iCloud, Stripe test, warunki zakupu i odpowiedź email → chat |
| `public-account-visual-current.json` | 5 PASS38,2s: 14 wariantów tras, jasny/ciemny,1440/390px,56 zrzutów; fonty, `<img>`, błędy zasobów, szerokość i motyw; prawdziwe lokalne MFA, puste panele |
| Test `assets.spec.ts` | 2 PASS: obrazki `<img>` strony głównej i oba motywy; głównie logo, nie wszystkie grafiki serwisu |

To izolowane bazy i jawne koszty testowe. Nie dowodzi to produkcyjnej rentowności, fizycznego passkey, wszystkich przewoźników ani nadania przesyłki. Raportów zawierających dane formularzy nie publikujemy.

## Rachunek, partie i konfigurator

- Rust pricing: **6 PASS**, w tym pięć stałych, ręcznie obliczonych przykładów kosztów, zaokrągleń, marży i minimum zamówienia.
- Prawdziwy slicer: partie 1/2/10/100/1000 PASS (2 ściany, 15% wypełnienia). Testy sprawdzają sumy płyt, czasu i masy, fizyczne ograniczenie pola stołu oraz masę pełnej kostki 20 mm przy 100% wypełnienia (około 9,96 g wobec 10,08 g wynikających z objętości i gęstości profilu). Artefakt prywatny: `slicer-quantity-physical.log`.
- `batch-pricing-oracle.json`: **1 PASS**, trzy wyceny API z jawnego fixture (PLA, standard, 3 ściany, 20% wypełnienia), niezależne kontrole każdej składowej rachunku:

| Sztuki | Płyty | Filament | Brutto |
| ---: | ---: | ---: | ---: |
| 1 | 1 | 4,476 g | 29,97 zł |
| 10 | 1 | 43,906 g | 40,18 zł |
| 1000 | 40 | 4385,116 g | 1608,21 zł |

Koszty: 100 zł netto/kg, 2 zł/h maszyny, 150 W i 1,20 zł/kWh, 5 zł/płytę, 10 zł przygotowania, 5% straty, 30% marży, minimum 20 zł netto, testowy VAT 23%. Potwierdzono powiązanie kosztu z pomiarami, jednorazowe przygotowanie i malejącą cenę jednostkową. Są to **stawki testowe**, nie weryfikacja rzeczywistych zakupów i rentowności.

`configurator-current.json`: **6 PASS** (130 s): nawigacja/motywy, pięć rodzin i 178 kolorów PLA, ustawienia, rzeczywiste wyceny, sterowanie WebGL (powiększenie potwierdzone zmianą obrazu), zaawansowane wartości i powrót, błędy połączenia oraz wybór/usuwanie STL/3MF. Awarię sieci wstrzyknięto celowo; wyceny pochodziły z rzeczywistego API. Kolory sprawdzono w UI/stanie, nie jako 178 osobnych wycen/pomiarów pigmentu. To nadal nie jest pełna wizualna kontrola wszystkich ekranów.

## Równoległe wyceny i wspólny zestaw

`concurrency.spec.ts`:4 niezależne sesje gości,8 równoległych żądań →4 zadania/4 snapshoty, po jednej próbie. Każdy dostaje własny model, ilość, pomiary i kwotę. Powtórzenie gotowej wyceny zwraca ten sam wynik bez ponownego slicingu. Katalog odpowiada przed upływem2s także po zaobserwowaniu aktywnego zadania; przyjęcie żądań mieści się w5s. To próba lokalna, nie SLA ani test dużego obciążenia.

Cudze metadane/GLB/status zadania/próba wyceny dają404. Po ukończeniu wszystkich zadań checkout z **pasującym cudzym modelem, ilością i wyceną** daje409; własny200 z dokładną kwotą snapshotu. Liczniki SQL obejmują tylko modele tego testu; UUID sprawdzany przed użyciem w zapytaniu. `concurrent-quotes-current.json`:1 PASS3,4s.

Nowe testy współbieżności i wyglądu należą do grupy `core`. Pierwszy wspólny przebieg26 PASS/1 FAIL ujawnił współdzielenie konta TOTP przez testy wyglądu i negatywny test logowania. Osobne konto fixture usuwa tę zależność; ochrona przed ponownym użyciem kodu pozostaje. Pełny `core.json` po dołączeniu wypełnionych paneli: **28 PASS /0 FAIL /0 skipped /0 flaky**,245,1s. Obejmuje też końcowe asercje SQL i dopasowanego cudzego checkoutu.

Regresja limitu używa teraz1 000 002 rzeczywiście różnych wierzchołków, zamiast powtarzanych zer. HTTP422 i objaśnienie w UI:1 PASS2,8s, także PASS w core. Brak zmian w produkcyjnym auth, limitach czy stylu. Sol xhigh: bez blokera, oceny8–9/10. Ten etap nie sprawdza restartu workera, wyczerpania kolejki ani dostawców.

## Spójność motywu i widoków

Odtworzono błąd: jasny motyw wybrany na stronie głównej znikał po bezpośrednim wejściu na logowanie. Wspólny `lib/theme.ts` przywraca preferencję przy starcie aplikacji i panelu admina; oba przełączniki zapisują ten sam klucz. Brak dostępu do browser storage nie blokuje zmiany motywu. Bez zmian CSS, palety i fontów autora: Workspace używa Manrope, storefront/admin.html ma nagłówki Oxanium.

`public-visual.spec.ts`:5 PASS38,2s (56 zrzutów). Trasy publiczne: strona główna, wycena, logowanie/rejestracja, dokumenty/regulamin/prywatność. Konta: lista/ustawienia klienta oraz admin.html, starsza lista, produkcja, materiały i cennik. W obu motywach1440/390px: właściwy nagłówek, fonty i obrazy, brak błędów zasobów/poziomego overflow; admin przełącza motyw i zachowuje go po reloadzie. Obejrzano4 reprezentatywne zrzuty (login/materiały mobile, admin/konto desktop). Build frontend0 błędów/ostrzeżeń. Przegląd Sol xhigh: bez blokera; oceny8–9/10, kwoty nie dotyczą zmiany.

Ten wcześniejszy zestaw obejmuje puste panele fixture i kontrolę `<img>`/fontów/HTTP zasobów; nie obejmuje wszystkich wypełnionych zamówień, dialogów, SVG ani klatek Stripe. Nie mierzono błysku motywu przed załadowaniem JS.

## Wypełnione panele i kontrast

`populated-visual-current.json`:1 PASS10s,44 zrzuty (32 widoki,12 płócien GLB). Prawdziwe nieopłacone zapytanie, trzy sztuki, chat klient↔admin; listy, szczegóły, pliki i historia w obu motywach/1440 i390px. Obrót zmienia piksele podglądu. Kwota pozostaje null i listy pokazują „Do wyceny”; test nie udaje zapłaty ani operacji dostawcy. Klawiatura dociera do ostatniej zakładki admina, pasek przewija ją do widoku. Dodatkowo obejrzano4 reprezentatywne zrzuty, także chat poniżej pierwszego ekranu.

Naprawiono słaby kontrast etykiety „Ręczna wycena”: przed zmianą1,32:1, po zmianie6,66:1 w jasnym i13,38:1 w ciemnym motywie. Jeden token ostrzeżenia w istniejącym motywie; bez zmiany układu. Build frontend0 błędów/ostrzeżeń. Sol xhigh: bez blokera w zakresie poprawki; oceny7–9/10. Kontrola kontrastu dotyczy tej etykiety, nie całej strony. Rejestracja passkey używa wirtualnego uwierzytelniacza Chromium; fizyczny klucz pozostaje niesprawdzony.

## Czytelne nazwy materiałów i kolorów

Szczegóły klienta i operatora pokazują np. „Bambu PLA Basic · Jade White”. Nowe snapshoty zachowują nazwy mimo późniejszej edycji katalogu. ID/opcje/kwoty/fingerprint pozostają niezmienione. Starsze snapshoty pobierają bieżące etykiety katalogu; brak profilu/koloru pozostawia plik i surowe ID. Własny opis koloru pozostaje dokładnym tekstem klienta. Poprawiono też scalanie różnych historycznych nazw jednego kodu w liście admina.

`order-labels-current.json`:**3 PASS27s** — ręczny snapshot po zmianie katalogu, symulowany stary rekord i brak katalogu, prywatny GLB, własny opis koloru, automatyczne snapshoty oraz rachunek1/10/1000 sztuk. Zmiany fixture cofane w finally, wyłącznie w osobnej bazie. Test pomocnika kolorów i CSV:**2 PASS**. Obejrzano nowy widok klienta390px/jasny i admina1440px/ciemny; bez zmiany CSS/układu. Build Rust i frontend PASS (svelte-check0/0; wcześniejsze ostrzeżenie CXX pozostaje). Sol xhigh: wcześniejsza uwaga zamknięta, brak kolejnych usterek w zakresie; pieniądze10/10, pozostałe8–9/10. Pełne28 testów poprzedzają tę zmianę; po niej sprawdzono wskazany zakres. Historycznej nazwy brakującej w starym snapshotcie nie da się odtworzyć po zmianie katalogu.

## Zatrzymywanie testów

Przerwany SIGTERM przebieg28 nie został zaliczony jako sukces. Naprawiono zbyt krótki budżet nadrzędnego sprzątania, ponowne przerwanie cleanupu i pozostawianie starego summary. Tworzenie bazy ma128-bitową nazwę; cleanup obsługuje nieznany wynik CREATE i błąd kopiowania logu.

Kontrolowane ręczne próby na własnych zasobach: `cancel-check.json` — dwukrotny SIGTERM, exit143, API/baza/pliki usunięte, porty wolne; `cancel-create-check.json` — CREATE wykonany, potwierdzenie wstrzymane, przerwanie bez pozostawienia bazy/katalogu; `cancel-copy-check.json` — wstrzyknięty błąd kopii, oczekiwany exit1, API/baza/pliki usunięte. `runner-normal-current.json`:1 PASS1,3s po ostatniej korekcie cleanupu. Pełne28 testów wykonano przed tą ostatnią korektą obsługi błędów runnera; kod produktu i scenariusze nie zmieniły się. Sol xhigh: wcześniejsze usterki zamknięte, brak nowego blokera; oceny8–9/10. Zewnętrzna przyczyna pierwszego SIGTERM nieustalona. Nie gwarantujemy cleanupu po awarii systemu ani dowolnie opóźnionym CREATE kończącym się już po DROP.

## Odtwarzanie korpusu

[Skrypt przygotowania](../frontend/tests/corpus-tool.md) działa offline na jawnie wskazanym katalogu. Zachowuje kolejność, hashe i nietypowe nazwy istniejących przypadków; dopisuje nowe pobrania, nie zmienia README ani wyników testów. `--check` jest tylko odczytem. Weryfikuje ścieżki, ograniczone odczyty ZIP, sygnatury oraz unikalność projektów/payloadów. Pięć syntetycznych regresji PASS; ponowny check wszystkich41 PASS. Nie zastępuje testów geometrii ani sprawdzenia licencji. Przegląd Sol xhigh: brak blokera; zakres ocen8–9/10, pieniądze/wygląd nie dotyczą skryptu.

## Tożsamość slicera

SHA-256 launchera i programu Bambu jest częścią zadania, klucza deduplikacji i snapshotu. Worker sprawdza tożsamość, checkout odrzuca brakujący/niezgodny hash. Zmiana plików wymaga restartu API/workerów; granice mechanizmu opisuje [kolejka](slicer-queue.md).

Świeży build PASS (pozostaje wcześniejsze ostrzeżenie wygenerowanego mostka C++). `runtime-fingerprint-current.json`: **1 PASS / 8,3 s** — świeży automatyczny checkout, brakujący/zmieniony hash → 409, przywrócony → 200, rzeczywiste partie 1/10/1000 i rachunek. Zmiany snapshotu wyłącznie w izolowanej bazie testowej. Test nie podmienia instalacji Bambu. Przegląd Sol xhigh: bez blokera w tym zakresie; współbieżność 7/10, dowody 8/10, zakres wizualny bez zmian 10/10, pozostałe kategorie 8–9/10. Nie oznacza to zakończenia całego celu.

## Otwarte

- Pobieranie zamknięte na41 modelach decyzją użytkownika. Nie wymagamy kolejnych59 plików ani rozwiązywania CAPTCHA.
- Świadoma polityka obsługi dużych modeli zamiast bezwarunkowego zwiększania limitów.
- Pełna tożsamość środowiska Bambu (biblioteki i zasoby poza dwoma plikami), rzeczywiste koszty zakupu/energii, opłacone zamówienia/dialogi we wszystkich stanach, wszystkie grafiki/SVG i większe obciążenie. Kontrola rachunku i ilości dla opisanych fixture jest wykonana.
- `cargo audit`: 0 podatności/ostrzeżeń. `npm audit`: trzy wpisy low pochodzące z jednego `deepmerge` w narzędziach deweloperskich; [brak opublikowanej poprawki](https://github.com/advisories/GHSA-gcqx-w48m-w2h4). Nie jest to dowód podatności przepływu uploadu.
- Przeglądy Sol xhigh pozostają poniżej 10/10. Cel nie jest zakończony.

## Modele pokazowe — rzeczywisty obraz WebGL

`showcase-current.json`: **2 PASS/8,7s**, bez pominięć/flaky; pięć autorskich obiektów × dwa motywy × desktop1440/mobilny390px =20 widoków. Sprawdza wybór materiału, podpis, prawdziwy kontekst WebGL, niepustą i różną sylwetkę każdego modelu, obrazy/fonty, błędy skryptów/zasobów i szerokość strony. Maska powstaje ze zrzutu płótna; magentowe tło wyklucza przezroczystość/cień i napisy. Oczekiwanie na piksele zapobiega ocenie poprzedniej klatki. Osobne20 zrzutów zachowuje normalny wygląd strony. Ręcznie obejrzano wszystkie pięć kształtów na reprezentatywnych zrzutach, w tym mobilny TPU.

Test zarejestrowany w `core`; po jego dodaniu cały aktualny zestaw przeszedł30/30 (szczegóły poniżej). Nie zmieniono kodu produktu ani stylu. To kontrola renderowania i ręczna ocena kształtów, nie automatyczne porównanie ze wzorcowymi PNG; inny, lecz różny obiekt może spełnić asercję sylwetki. Sol xhigh: brak pozostałych ustalonych uwag, oceny8–9/N/A, bez deklaracji10/10.

## Wspólne sprawdzenie aktualnego kodu

Źródło `564977f`, `core.json`: **30 PASS/0 FAIL/0 skipped/0 flaky,241,3s**. Rzeczywiste lokalne API/DB/slicer, wyceny1/10/1000, równoległość i własność, STL/STEP/3MF/prywatne GLB/retencja, konfigurator, publiczne i wypełnione panele,20 widoków pokazowych, logowanie/zmiana hasła/passkey/TOTP/sesje oraz dokumenty prawne. Uwzględnia poprawki nazw, odbić3MF i nowe testy pokazowe. Nie uruchamia zewnętrznych Stripe/Furgonetki/iCloud; ich odrębne dowody i ograniczenia pozostają powyżej. Nie obejmuje opt-in korpusu41; jego późniejszy wynik jest w tabeli na początku raportu.

## Aktualny sandbox i logistyka

Źródło pierwotnego przebiegu `70c3bae`: `sandbox.json` **7 PASS/2 FAIL/136,9s**, bez pominięć/flaky. Przeszły: zamknięcie nieopłaconej płatności przed usunięciem uploadu, opłacony odbiór/paczkomat/kurier, oferta Furgonetki i jej ponowne użycie, panele/pliki/chat/produkcja/zwroty częściowe i pełne/odmowa nadmiernego zwrotu oraz edycje katalogu/cennika. Oba FAIL zatrzymały się na kliknięciu DHL podczas płynnego przewijania.

Test logistyczny kończy teraz przewijanie przez `scrollIntoView(instant)`, potem wykonuje zwykłe `.check()` i sprawdza zaznaczenie; bez force/retry/ręcznego ustawiania checked i zmian kodu produktu. Późniejszy przebieg po tej poprawce testu, `logistics-current.json`: **2 PASS/73,5s**, bez pominięć/flaky i adnotacji blokady dostawcy. DHL/Furgonetka sandbox: opłacone zamówienia, produkcja/QC, potwierdzenie paczek/kosztu, nadanie, PDF, udany podjazd i anulowanie; osobne zamówienie ma tracking i status shipped po przekazaniu. Nie dowodzi rzeczywistego odbioru/doręczenia ani działania podjazdów DPD. To wcześniejsze7 PASS + późniejsze2 PASS; **pełnych9 po poprawce NOT RUN**. Sol xhigh: brak ustalonych uwag, prostota10, pozostałe właściwe kategorie9/N/A.

## Diagnoza pozostałych dużych projektów

Na źródle `fc52df5` wykonano osobne próby12 przypadków w kopii diagnostycznej: limit ZIP192/192 MiB, nie ustawienia aplikacji. Nadal2 GiB RAM,25s CPU/30s wall,1 mln wierzchołków/trójkątów,GLB25 MiB; kolejno, bez pomijania walidacji. Wszystkie hashe zgodne z manifestem.

| Przypadki | Wynik diagnostyczny |
| --- | --- |
| 7,9,13,31,38 | Konwersja działa; czasy9,6–24,8s, RAM169–255 MiB. Część jest blisko limitu czasu; nie oznacza dopuszczenia wszystkich w API. |
| 19,20,39,40 | `TooManyTriangles`; nie zwiększono limitu siatki. |
| 14 | lib3mf odrzuca krawędź niemanifoldową (`InvalidModel/E3001`). |
| 32 | lib3mf odrzuca niezadeklarowany prefiks przestrzeni nazw metadanych (`InvalidXml/E2003`); to nie dowód wadliwych trójkątów. |
| 24 | Exit137 po25,15s; przyczyna sygnału niepotwierdzona, bez klasyfikowania jako OOM/błędna geometria. |

Próby używały debug build. Nie przeniesiono limitu192 MiB do aplikacji. Dane szczegółowe pozostają prywatne; powyższe wyniki opisują diagnostykę, nie aktualne wyniki produkcyjnej ścieżki korpusu.

## Duże3MF przez formularz i rzeczywisty WebGL

`corpus-ui-final.json`: **6 PASS/119,8s**, bez pominięć/flaky: cztery rzeczywiste modele5/9/22/25 oraz dwa testy wszystkich modeli pokazowych. Prawdziwy upload i prywatny GLB, odmowa bez cookie, ponad100 tys. trójkątów w podglądzie, niepuste piksele i obrót zmieniający obraz.16 ujęć dużych modeli +20 przykładów w obu motywach/1440 i390px. Ręcznie obejrzano cztery reprezentatywne końcowe ujęcia. Dla modelu9 kwota UI jest dokładnie równa `print_grosze` odpowiedzi serwera; to dowód przekazania kwoty, nie niezależny rachunek kosztów. Pozostałe trzy mają serwerowy wymiar ponad500mm i odpowiadający komunikat ręcznej wyceny.

Pierwszy przebieg2 PASS/2 FAIL ujawnił brak UTF-8 w ograniczonym środowisku Chromium: bare-input próba bez locale przyjmowała ASCII, ale zwracała pusty FileList dla polskich nazw; z `LANG/LC_ALL=C.UTF-8` wszystkie działały. Runner ustawia teraz te dwie wartości wyłącznie dla procesu testowej przeglądarki. Po poprawce4 PASS/109,2s; po poprawce etykiety końcowe6 PASS jak wyżej. Na telefonie długa nazwa jest skrócona wizualnie do jednej linii; pełny tekst pozostaje w ARIA, podpowiedzi i liście modeli. Bez zmiany palety, fontów i układu. Build/svelte-check0 błędów/0 ostrzeżeń. Test jest opt-in; [uruchomienie](../frontend/tests/corpus-tool.md). Pełny core po tej zmianie NOT RUN; wcześniejsze30 PASS jest opisane z własnym źródłem.

## Przegląd wszystkich zapisanych scen

Na źródle `3afb0b8` obejrzano komplet36 scen z `corpus-ui-final` w czterech zestawieniach (1440/390px × jasny/ciemny); lista źródeł i zestawienia: prywatne `frontend/tests/artifacts/visual-overview/sources.json` i zestawienia `models-*.png` w tym samym katalogu. Każde źródło występuje raz. Wcześniej cztery reprezentatywne ujęcia obejrzano osobno. Zestawienia są przeskalowane do przeglądu kompozycji, nie zastępują kontroli każdego piksela ani wszystkich stron serwisu. Nie stwierdzono nowej usterki układu w tym zakresie. Nie zmieniano oryginalnych zrzutów ani kodu produktu i nie uruchamiano ponownie testów.

Historyczną blokadę CAPTCHA zamknięto zmianą zakresu: użytkownik zaakceptował41 modeli i zrezygnował z dalszego pobierania.11 odrzuceń zasobowych i oceny poniżej10/10 pozostają jawne; zamknięcie pobierania nie zmienia wyników testów ani nie oznacza ukończenia pozostałych prac. Dalsze dopuszczanie dużych plików wymaga oceny budżetu/polityki, nie pomijania walidacji.

## Wszystkie41 plików przez formularz

`corpus-ui-all41-final.json`: **41 PASS/235,0s**, bez pominięć/flaky. To test obsługi formularza, nie41 poprawnych wycen:

| Obserwowane zachowanie UI | Liczba |
| --- | ---: |
| Rzeczywisty podgląd po uploadzie | 25 |
| Komunikat o limicie backendu | 9 |
| Odrzucenie rozmiaru przez formularz25 MiB, bez POST | 7 |

Wszystkie25 podglądów: metadane właściciela, jego GLB, ARIA/nazwa, trójkąty, niepuste piksele, obrót w4 głębokich przypadkach i zoom w21 pozostałych, oba motywy i rozmiary1440/390.100 normalnych ujęć obejrzano jako cztery przeskalowane zestawienia; lista źródeł w prywatnym `frontend/tests/artifacts/visual-all41/sources.json`. To przegląd kompozycji, nie każdego piksela. Cztery dotychczasowe głębokie przypadki zachowują kontrolę dostępu, dokładnej kwoty serwera w UI i powodu wyceny ręcznej.

Pierwszy przebieg30 PASS/11 FAIL ujawnił ograniczenia testu: Chromium usuwał odpowiedzi dużych POST z cache diagnostycznego; rozproszone części2/18 miały widoczne5/58 pikseli, poniżej arbitralnego progu60. Kod/status odrzucenia sprawdza teraz ponowne wysłanie tych samych bajtów klientem HTTP właściciela i porównanie z komunikatem widocznym w formularzu. Nie jest to bezpośredni odczyt kodu z pierwszej odpowiedzi przeglądarki. Dla rzadkich geometrii wystarczy niepusty obraz, dodatkowo potwierdzony rzeczywistym GLB, trójkątami i zmianą po zoomie; nie deklarujemy minimalnej powierzchni na ekranie.

Po ostatnim wzmocnieniu asercji `corpus-ui-rejections-settled.json`: **9 PASS/5,4s** — komplet ustawień, zakończone przeliczanie, odpowiadający błąd, brak ceny i wyłączone zamówienie. Pełne41 wykonano przed tym wzmocnieniem, a potem sprawdzono zmienione9 przypadków. Nie zmieniono kodu produktu, limitów ani oczekiwań API: osobny `corpus41-entry64` nadal ma30 PASS/11 FAIL. Oczekujemy decyzji użytkownika o dalszym traktowaniu tych ograniczeń.
