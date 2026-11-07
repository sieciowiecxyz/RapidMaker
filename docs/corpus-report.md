# Modele i E2E — checkpoint 2026-11-06

Przeczytaj gdy: oceniasz dowody celu lub wybierasz kolejne sprawdzenie.
Opis: bieżące wyniki, granice i decyzje; plan w [corpus-quality.md](plans/corpus-quality.md).

**Cel pozostaje otwarty.** Ostatni pełny standardowy korpus: 36 PASS/5 FAIL, w tym 20 zmierzonych cen, 11 ręcznych zgłoszeń i 5 oczekiwanych odrzuceń 413. Użytkownik wybrał rozszerzenie obsługi. Dalsze wyniki opt-in opisano poniżej; domyślny profil serwera pozostaje bez zmian.

Chronologiczny raport sprzed skrócenia pozostaje w Git: `30cd52a:docs/corpus-report.md` (`git show 30cd52a:docs/corpus-report.md`). Nie usunięto surowych dowodów, modeli ani testów. JSON/logi/obrazy są lokalne i ignorowane, głównie `frontend/tests/artifacts/`; modele i manifest SHA-256: `backend/test-models/makerworld/`. Licencje nie są w pełni sprawdzone, plików nie publikujemy. Bez zrzutów sekretów auth/MFA.

## Mapa dowodów

Wyniki dotyczą podanych przebiegów, nie późniejszego HEAD. Kontrolowane kontrakty UI i wstrzyknięte usterki nie dowodzą działania dostawców. Nie sumujemy oddzielnych przebiegów w jeden PASS.

| Artefakt / źródło | Wynik | Zakres |
| --- | --- | --- |
| `contracts.json`, 97c10dd | 21 PASS / 5,7 s | Kontrolowane kontrakty UI/narzędzi; bez dowodu dostawców |
| `core.json`, 242a935 | 33 PASS / 312,6 s | Realne lokalne API/DB/parser/slicer, koszyk, współbieżność, własność/retencja, konfigurator, panele, motywy/zasoby i bezpieczeństwo |
| `corpus41-trimmed.json`, 8bd6fcc | 36 PASS / 5 FAIL / 424,3 s | Pełne 41 po poprawkach parsera; klasyfikacja niżej |
| `database-with-api-final.json` | 20 PASS / 0 skipped | 18 storage + katalog API + granice dostępu HTTP; osobna tymczasowa baza na test |
| Rust workspace, 8bd6fcc (historyczny) | 139 PASS / 25 ignored | 20 pomijanych wykonano osobno powyższym runnerem; pozostałych 5 nie zaliczamy |
| `bundle-runtime-pricing.json`, dd24620 | 4 PASS / 24,5 s | Tożsamość całego pakietu Bambu, ceny 1/10/1000, koszyk wielu modeli, ręczna wycena i hash checkoutu |
| `manual-offer-visual-final.json`, 333b894 | 1 PASS / 25,5 s | 72 PNG, w tym 28 nowych kart: walidacja ceny/dostawy, publikacja i wygaśnięcie |
| `manual-publication-final.json`, cf9a682 | 2 PASS / 29,3 s | Zerwanie przed/po zapisie, retry, wersje/outbox/logout + wspólna regresja wizualna |
| `manual-publication-concurrent.json`, f37280b | 1 PASS / 29,5 s | Dwie niezależne sesje MFA jednego admina; konflikt dwóch cen |
| `svg-public-presence-final.json`, 30cd52a | 6 PASS / 55,2 s | 56 kombinacji, 246 powtarzanych kontroli rysowania SVG, 4 duże tła tylko geometrycznie |
| `expanded96-ui.json` | 7 PASS / 194,8 s | 4 podglądy i 3 poprawnie pokazane odrzucenia; nie 7 cen |
| `triangle-budget-api.json` / `triangle-budget-ui-verified.json` | 2 PASS / 60,5 s; 2 PASS / 58,6 s | Modele 24/39: rzeczywisty upload, prywatny GLB, UI i ponad 1 mln widocznych trójkątów |
| `sandbox.json`, baza f60b381 | 9 PASS / 168,0 s | Stripe test/Furgonetka sandbox/logistyka; bez provider-blocked, późniejsze regresje osobno |
| `selected-carrier-payment.json` | 2 PASS / 35,3 s | Wybrany przewoźnik, dokładna kwota, intent i trwałe zamówienie |
| `concurrent-unpaid-payment.json` | 1 PASS / 5,9 s | 4 równoległe POST, jedna próba/zamówienie, konflikty i retencja |
| `concurrent-refund-final.json` | 1 PASS / 25,7 s | Rzeczywisty Stripe test; współbieżne zwroty nie przekraczają wpłaty |
| `paid-refunded-visual-final.json` | 1 PASS / 37,6 s | 16 ujęć paid/refunded, klient/admin, oba motywy i rozmiary |
| `refund-mail-journey.json` | 1 PASS / 211,1 s | Rzeczywisty iCloud MFA/oferta/płatność/dokumenty/zwrot 500 gr/email/odpowiedź w chacie |
| `server-receipt-recovery.json` | 1 PASS / 22,1 s | Zakup gościa, reload bez receipt, purge starego kontaktu; kontrolowana usterka szczegółów 404 |
| `registration-password.json` / `registration-passkey.json` | po 1 PASS / 32,8 s; 28,7 s | Nowe konto, rzeczywisty email iCloud/kopia warunków; hasło lub wirtualny WebAuthn |
| `email.json` | 2 PASS | iCloud hasło/email MFA, admin TOTP/uprawnienia, oferta → Stripe test → email → chat |

Pełny core po poprawce: **33 PASS/312,6 s,0 skipped/flaky**. Poprzedni przebieg na30cd52a:32 PASS/1 FAIL/333,9 s (`core-before-runtime-warmup.json`) —zimny hash opóźnił pierwsze zlecenia6861ms przy wymaganych<5000ms. API z bazą inicjalizuje teraz hash przed obsługą HTTP; test współbieżnych wycen po poprawce przeszedł w1,5s przy niezmienionym progu. Start API wydłuża się o zimny odczyt, błąd pakietu blokuje wyceny i zapisuje ostrzeżenie, pozostałe API może działać. Build/Clippy API all-targets/fmt/diff-check PASS; wcześniejszy warning wygenerowanego CXX pozostaje. Historyczny core31 PASS/237,4s na97c10dd jest zapisany w raporcie w Git, nie jest bieżącym plikiem core.json.

## Diagnostyka większych modeli —2026-11-06

[Plan](plans/large-models.md) obejmuje9 plików8/14/19/20/21/29/37/40/41, także pięć wcześniejszych413. Diagnostyka jest odrębna od HTTP: SHA-zweryfikowane wejścia, brak sieci/sekretów/zapisu siatek,2GiB RAM/25s CPU/30s wall; parser96MiB wejścia/512MiB ZIP/128MiB GLB/4M punktów/8M trójkątów. To nie jest włączony profil produktu.

| Przypadki | Najnowszy dowód diagnostyczny |
| --- | --- |
|8/19/20/29/40 | Pełny parser+GLB,1,49–3,86M trójkątów,26,8–69,5MB GLB; wszystkie GLB przekraczają25MiB |
|14/21/37 | Po dokładnie tej samej normalizacji `geometry_package` SDK odrzuca krawędź współdzieloną przez4 trójkąty zamiast najwyżej2. Aktualny probe: `NonManifoldMesh` w9,040/22,522/20,447s |
|41 | SIGKILL9 przy~25s CPU (limity25/25); przyczyna sygnału nierozstrzygnięta, bez dowodu OOM |

Prywatne dowody: `large-model-probe/results-combined.json` —pierwotne9 przypadków z hashami źródeł/binariów; `large-model-probe-xml-opt/comparison.json` —oddzielne próbki optymalizacji; `sdk-rejection/normalized-results.json` —dokładny etap SDK po normalizacji, surowe SDK zapisane osobno; `large-model-probe-topology/results.json` —nowy typ błędu na3 rzeczywistych plikach. Tymczasowy przykład SDK usunięto po zachowaniu źródła/proweniencji prywatnie. Pierwsza infrastruktura bez `/usr/bin/time` nie uruchomiła parserów; pomiar zastąpił Python `resource.getrusage`.

Dev opt-level2 dla istniejących `zlib-rs`/`quick-xml` skrócił pojedynczą próbkę modelu20 z15,674s do10,034s (te same liczby punktów/trójkątów, wymiary i długość GLB). Bez zmiany wersji, release, walidacji/limitów; nie jest to gwarancja czasu HTTP. Wcześniejsze `MalformedInput` dla14/21/37 rozpoznano dopiero w osobnej diagnostyce SDK; nie utożsamiamy starego braku przyczyny z dowodem uszkodzenia całego pliku.

### Bezpieczny komunikat o topologii

Nowy `NonManifoldMesh` zachowuje odrzucenie SDK. Upload podaje stałe422 `model_topology_invalid` („Siatka modelu wymaga naprawy…”), bez diagnostyki/objectIDs. Dopasowanie wariantu/frazy dotyczy lib3mf0.1.7; inna fraza wraca do ogólnego odrzucenia, nie akceptacji. Legacy `/analyze` nadal ma ogólny błąd. Duże źródła w HTTP nadal najpierw podlegają dotychczasowym limitom; brak automatycznej naprawy lub nowych zaakceptowanych dużych uploadów.

`topology-models-ui-final.json`: **6 PASS/5,5s**,0 skip/flaky —syntetyczny3MF934B odrzucony, brak zaakceptowanego modelu/GLB/checkoutu, następny poprawny upload działa. Wiersz pokazuje błąd analizy zamiast prośby o ustawienia; zrzut obejrzany, bez zmian CSS/układu. Pierwszy test FAIL przez zgubione body inspektora Chromium; finalny sprawdza status/UI i identyczny HTTP w tym samym kontekście przeglądarki, bez mocków. Geometry33 PASS/1 opcjonalny ignored, klasyfikacja API1 PASS; backend/example/frontend build, Clippy geometry/API all-targets, fmt/Prettier/diff-check PASS. Wcześniejsze `xml-zlib-models.json`:5 PASS/4,1s, przed nowym typem. Nie powtarzano pełnego korpusu; wynik36/5 pozostaje historycznym dowodem z8bd6fcc.

## Korpus, geometria i ograniczenia

41 unikalnych projektów MakerWorld: 34 × 3MF i 7 × STL, bez STEP; STEP pochodzi z fixtures. Offline [corpus-tool.md](../frontend/tests/corpus-tool.md): ścieżki/sygnatury/hash/unikalność i ograniczony ZIP; `--check` bez zapisu. 5 regresji +check41 PASS, bez dowodu geometrii/licencji.

| Aktualny wynik API (8bd6fcc) | Liczba | Przypadki |
| --- | ---: | --- |
| Cena z rzeczywistego slicera | 20 | 1,3,4,6,7,9,10,11,12,13,17,23,26,27,30,33,34,35,38,39 |
| GLB + ręczne zgłoszenie | 11 | 2,5,15,16,18,22,24,25,28,31,36 |
| Oczekiwane 413, upload >32 MiB | 5 | 8,21,29,37,41 |
| FAIL 422 `model_resource_limit` | 4 | 14,19,20,40 |
| FAIL 422 nieprawidłowy pakiet | 1 | 32, niezadeklarowany prefiks metadanych |

Limity: upload API 32 MiB/UI 25 MiB; ZIP 96 MiB łącznie i na wpis, 512 wpisów; 1 mln unikalnych wierzchołków, **1 048 574 trójkąty** z budżetu kanonicznego STL 50 MiB; GLB 25 MiB; worker 2 GiB RAM/25 s CPU/30 s wall, bez sieci/sekretów. Lokalny parser STL nadal 25 MiB/1 mln. Dev optymalizuje geometry/lib3mf/zlib-rs/quick-xml, z debug assertions i kontrolą przepełnień. Większe ZIP dopuszczono decyzją użytkownika; osobnego profilu ponad STL 50 MiB nie zatwierdzono.

Diagnostyka rozwiniętych buildów/instancji wykazała brak nieużywanych trójkątów pozwalających zmieścić pozostałe sceny: 14 → 1 953 958 trójkątów/97 697 984 B STL; 19 → 3 861 000/193 050 084 B; 20 → 1 487 438/74 371 984 B; 40 → 1 677 324/83 866 284 B. To liczenie referencji, nie udana konwersja ani walidacja topologii. Nie zgadujemy namespace modelu 32 i nie obchodzimy lib3mf. Ograniczony preflight jednoznacznego małego manifestu/relacji ≤64 KiB skrócił obserwowane odrzucenie z 22,8 s do 633 ms (1 PASS/1,3 s); pełna walidacja zostaje, inne manifesty używają dotychczasowej ścieżki. Geometry 33 PASS/1 opcjonalny ignored.

Indeksowanie nie upraszcza siatki; odbicia/malowanie Bambu zmieniają tylko kopię. Dwa CPU slicera naprawiły model 4 bez zwiększania limitów. Usunięto nieudostępniane formaty (494 linie/16 pakietów), zachowano STL/STEP/STP/3MF i wyjściowy GLB. `geometry-formats-trim.json`: 5 PASS/4,2 s; geometry 31 PASS/1 opcjonalny ignored.

Modele 24/39 odzyskano po powiększeniu liczby trójkątów wyłącznie do istniejącego budżetu STL: 24 ręczna `printer_fit`, 39 rzeczywista cena. GLB ma 1 033 695/1 046 124 trójkąty, renderer 1 033 689/1 046 109; niezależnie potwierdzono 6/15 zerowych pól pomijanych przez renderer. Pierwszy strict UI FAIL wymagał błędnie równości liczb; poprawiono oracle, nie produkt. Historyczny exit137 modelu 24 był nieustaloną diagnostyką, nie bieżącym API ani dowodem OOM.

## Rachunek i tożsamość slicera

Rust pricing: 6 PASS, w tym 5 ręcznie policzonych przykładów. Partie rzeczywistego slicera 1/2/10/100/1000 PASS, sumy płyt/czasu/masy i pole stołu; pełna kostka 20 mm około 9,96 g wobec 10,08 g z objętości/gęstości. `batch-pricing-oracle.json`: 1 PASS, niezależne składowe jawnego fixture PLA/standard/3 ściany/20%:

| Sztuki | Płyty | Filament | Brutto |
| ---: | ---: | ---: | ---: |
| 1 | 1 | 4,476 g | 29,97 zł |
| 10 | 1 | 43,906 g | 40,18 zł |
| 1000 | 40 | 4385,116 g | 1608,21 zł |

Testowe 100 PLN netto/kg dowodzi arytmetyki, nie rentowności firmy. `multi-model-pricing.json`: 1 PASS/4,6 s; dwa uploady/pomiary (1/10), jedno przygotowanie/rezerwa/marża/minimum/VAT; suma dokładna, odbiór bez dostawy. Podmiana ilości/modelu/cudzej wyceny →409, duplikat ID →400. `concurrency.spec.ts`: 4 sesje, 8 żądań →4 zadania/snapshoty, po jednej próbie; gotowy wynik bez ponownego slicingu, katalog <2 s przy aktywnym zadaniu. Bez dowodu SLA/dużego obciążenia.

Od **dd24620** SHA-256 obejmuje całe stałe drzewo `/opt/bambustudio-bin`: zasoby, ścieżki, typy, uprawnienia i zawartość, z kontrolą wykonywalności. Hash jest w zadaniu/deduplikacji/snapshotcie; brak lub inny hash blokuje checkout 409, przywrócenie daje 200 w izolowanej DB. Zmiana wykryta w działającym procesie blokuje użycia do restartu. `spawn_blocking` chroni async runtime; pierwsze równoległe użycia mogą czekać. 5 jednostkowych PASS + osobno test instalacji 1 PASS: zimny odczyt 6,905 s, 10 ciepłych kontroli 101,117 ms. **Biblioteki systemowe poza pakietem nie są hashowane**; brak atestacji/ochrony przed celową podmianą w trakcie wykonania. Szczegóły: [slicer-queue.md](slicer-queue.md).

## Publikacja oferty i błędy

Naprawiono znikanie błędu ceny/dostawy: lokalna walidacja nie odmontowuje formularza. Po zerwaniu sieci `OrderDetail` odświeża tę samą ofertę z zachowaniem pól; nowsza wersja rozstrzyga publikację, 401/403/404 usuwa prywatny widok. Przed zapisem przerwany POST zachowuje wersję 1 i brak emaila; identyczny retry →wersja 2/25 zł/jeden outbox. Utrata odpowiedzi po zapisie 30 zł →odczyt wersji 3 bez ponownego POST; stara wiadomość skipped, nowa pending. Logout/refresh usuwa dane, API 401. SMTP/płatności w tym teście wyłączone: to dowód kolejki, nie doręczenia.

Dwie sesje tego samego konta MFA publikują 35/40 zł z wersji 3 →200/409, wspólna wersja 4/cena, konflikt u przegranego, jeden aktualny outbox. Nie dowodzi to dwóch różnych adminów ani wymuszonego nakładania transakcji DB.

Istotne początkowe FAIL: 30,9 s potwierdził znikający komunikat UI; kolejny 21,3 s — błędny testowy GET admin/orders/{id} (404), zastąpiony istniejącą listą z kontrolą 200/UUID. FAIL publikacji 21,3 s potwierdził utratę stanu; wspólna regresja ujawniła globalny selektor pliku przy dwóch zamówieniach, zawężony do numeru zamówienia. Bez zmiany API lub osłabienia walidacji; wyniki w tabeli.

## Zakup, konta, dostawcy i retencja

Realne lokalne sesje/MFA/własność są oddzielone od zewnętrznych dowodów. Rejestracja ma rzeczywisty email iCloud, WebAuthn używa wirtualnego uwierzytelniacza Chromium; brak dowodu fizycznego klucza. Klient →admin 403, cudze zasoby 404; granice modeli/GLB/zleceń testowane HTTP i DB. Emaile rzeczywiste wysyłano **wyłącznie do void@sieciowiec.xyz**. Nigdy nie utożsamiamy wpisu outbox z doręczeniem.

Stripe pozostaje testowy, Furgonetka sandbox. Sprawdzono zgodność kwoty oferty/przycisku/intentu/zamówienia i wybranego przewoźnika; bez dowodu fizycznego przewozu/wszystkich przewoźników. Podjazd DPD pozostaje ograniczony `carrierConnectionError`; wyniki provider-blocked są odrębne. Brak niezależnej inwentaryzacji księgi Stripe.

Cztery równoległe identyczne POST →ten sam order_id, jedna próba/zamówienie/provider_id; nowy klucz na zablokowanym szkicu, zmiana wersji/nieistniejąca oferta →409. Bez obciążania karty; hold plików utrzymany podczas otwartej płatności, usunięcie sprawdzone po wygaśnięciu. Nie dowodzi użycia tego samego klucza dla innej prawidłowej oferty. Niepewna płatność/realizacja blokuje sprzątanie, completed ma retencję 90 dni; szczegóły [slicer-queue.md](slicer-queue.md).

Zwroty: po 500 z 2500 gr konkurujące 1500+1500 →202/409 `refund_amount`; retry związany z kluczem/rekordem. Po potwierdzeniu 2000 gr UI pokazuje pozostałe 5 zł, końcowy zwrot →dokładnie 2500, trzy `succeeded`; klient 403/obcy 404/overrefund 409. Jedna sesja admina, nie dwie niezależne. Naprawiony klient widzi „Zwrócono płatność”; autoryzowane odczyty zwracają tylko sumę `refunded_grosze` z potwierdzonych sukcesów. Oś realizacji nie cofa się po zwrocie.

Email zwrotu: trwały sukces Stripe nie zależy od pojemności outbox; rezerwacja przed próbą, retry ≥60 s, outbox+znacznik atomowe, klucz per zwrot. Sprawdzono kolejkę 10000, restart, CAS, błędny kontakt/postęp innych zamówień; migracja 0022 nie wysyła starych sukcesów. Jeden event nie oznacza exactly-once SMTP. Journey potwierdza odebrany email dokładnie 5.00 PLN/ref i jedną odpowiedź w chacie.

Usunięto 345 martwych linii makiety i lokalne kopie prawdziwych zamówień/kontaktów. Backend potwierdza wpłatę; receipt bez sekretów ma walidację, uszkodzony cache jest ignorowany. Fingerprint przed/po płatności; koszyk czyści tylko zgodna potwierdzona wpłata. Szkic kontaktu zostaje w przypisanym `sessionStorage`. Szczegóły gościa: 24 h i kontrola właściciela. Kontrolowane 404 po zweryfikowanej wpłacie nie jest testem postarzonego rekordu.

## Spójność motywu i widoków

Wygląd i interakcje Piotra pozostają wzorcem. `public-account-visual-current.json`: 5 PASS/38,2 s, 14 wariantów tras ×2 motywy ×1440/390 =56 zrzutów; obrazy/fonty/zasoby/szerokość, realne lokalne MFA, puste panele. Obejrzano 4 reprezentatywne. Brak pomiaru błysku motywu przed JS. `populated-visual-current.json`: 1 PASS/10 s, 44 zrzuty (32 widoki +12 GLB), realna ręczna oferta/chat/pliki/zakładki; kwota null, bez udawanej zapłaty. Kontrast jednej etykiety poprawiono 1,32:1 →6,66:1 light/13,38:1 dark; nie audyt całej strony.

`showcase-current.json`: 2 PASS/8,7 s, 20 widoków (5 obiektów ×2 motywy ×2 szerokości), niepuste/różne sylwetki WebGL i przegląd pięciu kształtów; bez wzorcowych PNG. Historyczny `corpus-ui-all41-final.json`: 41 PASS/235 s =25 podglądów/9 limitów/7 odrzuceń UI bez POST, nie 41 cen ani bieżąca klasyfikacja API. 100 ujęć obejrzano w 4 zestawieniach. Chromium gubił duże odpowiedzi: kod odrzucenia potwierdzano powtórką HTTP właściciela + komunikatem UI. Sceny 2/18 miały 5/58 pikseli; niepusty GLB/zmiana zoomu zastąpiły arbitralne minimum 60. Po wzmocnieniu asercji: 9 PASS/5,4 s, bez ceny/checkoutu po przeliczeniu.

SVG: 56 kombinacji, **246 kontroli powtarzanych, nie unikalnych ikon**; cztery duże tła tylko geometrycznie. Diagnostyczne nieprzezroczyste tło i porównanie przed/po ukryciu potomków sprawdza wkład pikseli; przywraca styl/atrybut. Puste/ukryte/brak SVG celowo FAIL; stroked minus PASS. Minimum: storefront 20, admin 11 desktop/10 mobile, inne trasy 1. Powyżej 600 px w osi tylko geometria, limity 160 korzeni/2000 kształtów. To minimum obecności/rysowania, **bez dowodu kontrastu na realnym tle, dokładnej tożsamości każdej ikony lub wszystkich jej ścieżek**. Finalny przebieg dodaje minimum i kontrole negatywne, bez zmian wyglądu.

Ujęcia ofert i paid/refunded obejrzano w zestawieniach, wybrane mobile natywnie; bez dowodu wszystkich dialogów/pikseli/klatek Stripe. `order-labels-current.json`: 3 PASS/27 s, snapshoty nazw, fallback starego rekordu/brak katalogu, opis klienta i kwoty; brakującej historycznej nazwy nie da się odzyskać po zmianie katalogu.

## Zatrzymywanie testów

SIGTERM nie jest PASS. Ręczne `cancel-check.json`, `cancel-create-check.json`, `cancel-copy-check.json` sprawdziły podwójny SIGTERM (exit143), CREATE z wstrzymanym potwierdzeniem i błąd kopiowania logu (exit1): usunięto własne API/DB/pliki, porty wolne. `runner-normal-current.json`: 1 PASS/1,3 s po korekcie cleanupu. Nie gwarantuje sprzątania po awarii systemu ani CREATE kończącym się po DROP; zewnętrzna przyczyna pierwszego SIGTERM nieustalona. Stary summary nie jest wynikiem przerwanego przebiegu.

## Otwarte

- Odpowiedź użytkownika o profilu dużych modeli; 14/19/20/40 i błędny 32 nadal jawnie FAIL. Bez ponownego szukania plików/CAPTCHA.
- Ocena luk całego celu po33 PASS core: wszystkie renderowane grafiki/przykłady oraz klikalne przepływy z mapy kontrolek, stany/dialogi, desktop/mobile i oba motywy. Obecne macierze dowodzą podanych zakresów.
- Systemowe zależności Bambu, fizyczny passkey, większe obciążenie, rzeczywiste koszty firmy i produkcja nie są potwierdzone powyższymi testami.
- Audyt: Cargo **0 podatności/1 notice** `pin-utils` unmaintained (async-imap 0.12.0); npm **3 low** z jednego niezałatanego upstream `deepmerge`, bez wyciszeń/forka. [security.md](security.md).
- Końcowy audyt 10 kategorii pozostaje otwarty. Oceny małych zmian 7–9/8–9/N/A nie są oceną całego produktu ani wymaganym 10/10. Nie ma nowej deklaracji gotowości produkcyjnej lub zakończenia celu.

## Bezstratny GLB — etap po diagnozie topologii

`geometry/src/glb.rs` wybiera mniejszy zapis: dotychczasowy jeden blok u32 albo do64 bloków z lokalnymi indeksami u16. Bez zmiany współrzędnych f32, kolejności/liczby trójkątów, orientacji i siatki produkcyjnej; bez nowych zależności. Pełna długość z JSON/paddingiem jest sprawdzana przed alokacją wyjścia. Granice accessorów zapisujemy dokładnie jako f32→f64, bez skrótu dziesiętnego mogącego przesunąć min/max.

Parser prywatnego GLB przyjmuje oba zapisy, sprawdza sumę trójkątów przed alokacją i nadal ogranicza plik do25 MiB. Wierzchołki powtórzone pomiędzy blokami są czytane z wejściowego DataView, bez osobnej kopii pozycji; limit źródłowych unikalnych wierzchołków backendu pozostaje1 mln. Nie zmieniono HTTP/STL/CPU/RAM ani budżetu trójkątów renderera.

Pomiar diagnostyczny pięciu tych samych plików SHA, izolacja2 GiB/25s CPU/30s wall, bez sieci/sekretów/zapisu siatek. Pełna geometria, liczby i wymiary zgodne z poprzednim pomiarem. To rozmiar diagnostycznego GLB, **nie dowód zaakceptowania większego uploadu w HTTP**; probe nie dodaje produkcyjnego transformu metry/Y-up.

| Model | GLB wcześniej, B | Teraz, B | Mniej | ≤25 MiB |
| --- | ---: | ---: | ---: | --- |
| 8 | 28,220,252 | 19,466,244 | 31.0% | tak |
| 19 | 69,459,384 | 47,621,460 | 31.4% | nie |
| 20 | 26,773,996 | 20,693,156 | 22.7% | tak |
| 29 | 35,875,220 | 29,224,268 | 18.5% | nie |
| 40 | 30,188,228 | 22,325,800 | 26.0% | tak |

Wszystkie pięć konwersji zakończyło się poprawnie:0,343–13,625s wall, peakRSS148–450 MiB.19/29 nadal przekraczają budżet podglądu. Artefakt: `large-model-probe-compact/results.json` (hash programu/źródeł i metryki); bez zwiększenia limitów produkcyjnych.

Walidacja: geometry38 PASS/1 opcjonalny ignored (w tym granica64→65 bloków, dokładne min/max i zachowanie geometrii); `compact-glb-contract.json`22 PASS/0,759s; finalne `compact-glb-models-final.json`6 PASS/5,6s po korekcie min/max — rzeczywiste STL/STEP/3MF, u16 z API, prywatność/retencja/odrzucenia i odzyskanie. `compact-glb-large-ui.json`2 PASS/44,0s dla24/39: niezależne liczenie wszystkich bloków i zerowych pól, render/zoom,1440/390, oba motywy. Te dwa przebiegi UI były przed samą korektą JSON min/max; geometria binarna bez zmian, korekta ma osobny test dekodowania gltf i finalną regresję API. Obejrzano zestaw8 ujęć; bez zmian UI. Backend/example/frontend build, Clippy geometry/API all-targets, fmt/Prettier/diff-check sprawdzone. Pełnego core/korpusu nie powtarzano.

Zwolniono także zamknięte, starsze niż24h sesje incremental wyłącznie naszych pakietów/probe po uzyskaniu ich locków; nie usuwano źródeł, modeli, sekretów ani działających binariów. Wolne miejsce wzrosło z0,42 do6,08 GiB. Prywatny `cache-cleanup.json`.

## Sesja oczekująca na2FA po zmianie hasła

`pending-mfa-revocation.json`: **2 PASS/8,5s**, bez skipped/flaky, rzeczywista osobna baza/API/Chromium. Wzmocniono istniejący test ustawień: niezależne urządzenie loguje się starym hasłem i uzyskuje sesję `second_factor` (bez dostępu do zamówień). Zmiana hasła przez formularz200 odwołuje tę sesję i kopię sesji zalogowanej: próba factor/verify, auth/me i zamówienia zwracają401. POST sprawdzamy przed GET mogącym usunąć cookie. Sama błędna próba TOTP nie stanowi dowodu odwołania — potwierdza je także auth/me401 po uprzednim200/second_factor. Nowe hasło nadal wymaga2FA, stare daje401; istniejące sprawdzenia passkey/TOTP/kodów ratunkowych i odwołania sesji pozostały. Konteksty zamykane w finally. Bez zmian produkcyjnego kodu/UI, SMTP lub nowego dowodu dostawców.

W diagnostyce41 odrzucono hipotezę kompresji wewnętrznego ZIP: test Stored nie poprawił czasu (20:10,022→10,018s;41:25,074→25,077s i SIGKILL9). Zmiana oraz jej dodatkowy test zostały wycofane, nie zaliczamy ich do obecnych38 testów geometrii. Osobny tymczasowy program z punktami pomiarowymi: normalization1,247s, potem brak zakończenia pełnej walidacji SDK przed granicą25s CPU. Przyczyna sygnału nadal nierozstrzygnięta; brak dowodu OOM. Zapisano źródło/hash programu i metryki prywatnie (`large-model-probe-phases`, `large-model-probe-zip-comparison`), przywrócono i przebudowano normalny app/probe. Nie obchodzono walidacji i nie zmieniano limitów.

Review GPT-6-Sol xhigh tej zmiany: bez blokera; poprawność9, bezpieczeństwo9, zasoby9, błędy9, dowody9, prostota9, utrzymanie9; kwoty/współbieżność/wygląd N/A (brak zmiany lub nowego dowodu). Reviewer sprawdził kod `auth.current`/`auth.pending`: kontrola zamówień przed zmianą nie usuwa cookie sesji2FA, więc późniejsze auth/me401 nie jest skutkiem tej kontroli. Testów nie uruchamiał ponownie. Nie jest to10/10 ani zamknięcie całego celu.

## Większy profil backendu — nadal opt-in

`large-profile-complete.json`: **3 PASS/67,7s**,0 skipped/flaky — rzeczywiste SHA-bound8/20/40: upload201, oryginał z identycznym hashem, pełny kanoniczny STL powyżej50MiB (dokładna liczba trójkątów i wszystkie granice), GLB≤25MiB, brak dostępu obcej sesji401/404.20 dodatkowo rzeczywisty slicer:1płyta,7687gr i niezależny rachunek materiału/czasu/energii/obsługi/straty/marży/VAT na jawnych stawkach fixture. Nie jest to ocena rentowności produkcyjnej.

`MODEL_PROFILE=large` działa tylko przy jawnym `AUTH_MODE=localhost`; **nie zmieniono `.env` ani domyślnych limitów/Twojego serwera**. Backend96MiB input/512MiB ZIP/200MiB STL, GLB25MiB. Parser nadal2GiB/25CPU/30wall, Bambu large4GiB/120CPU/125wall na partię; standard pozostaje60CPU/65wall. Całe zadanie150s/lease180s i kwoty256MiB/8GiB niezmienione. W tym wcześniejszym etapie backendowym frontend nie obsługiwał jeszcze limitów/renderowania profilu; bieżący stan opisuje następna sekcja. Pełne ograniczenia/komenda: [plan](plans/large-models.md#explicit-backend-profile-not-enabled-on-the-user-server).

Zwykły profil `large-profile-standard-regression.json`:11 PASS/21,7s,3celowe pominięcia dużego profilu przez dopasowanie dawnej nazwy pliku (nowa `large-profile.spec.ts` usuwa kolizję selektora). W tym ceny1/10/1000, multi-model, własność/retencja i nowy100Continue/niekompletny body→429, po rozłączeniu200. `large-profile-storage.json`:20 PASS/0skip na osobnych bazach, migracja0023 i kwoty. Geometry39PASS/1opcjonalnyignored; API29PASS/1DBignored, slicer15PASS/3opcjonalneignored; właściwe DB przypadki wykonano osobno powyżej. Nie sumujemy tych przebiegów w pełny korpus/core.

Pierwszy test ceny20 (`large-profile-quote.json`) FAIL:3próby slicera z kodem137 przy60CPU (~33,4s wall każda), przyczyna sygnału nieustalona, bez twierdzenia OOM. Po jawnym120CPU `large-profile-quote120.json`1PASS52,8s; finalny3PASS obejmuje tę poprawkę. Pierwsze3FAIL testu uploadu dotyczyły zbyt ścisłego oczekiwania ciągu cache-control; prawidłowe `no-store` pozostawiono, test wymaga tej dyrektywy. Surowe wyniki zachowane.

Review wykrył i poprawiono: dostęp do wcześniej zapisanych dużych plików po powrocie do standardu (bezwzględne limity odczytu96/200, nadal właściciel/admin i dokładna długość); legacy analiza teraz blokuje drugie body przed alokacją i ma60s deadline całego żądania. Test prywatnych odczytów sprawdza funkcję/duży plik/admin/niezgodność długości, bez faktycznego restartu procesu. Clippy geometry/API/storage/slicer all-targets z-Dwarnings, app build, Python syntax, Rustfmt/Prettier/diff-check PASS; znane ostrzeżenie generowanego CXX pozostaje. Ten etap backendowy nie zmieniał UI ani zależności; późniejsze podłączenie frontendu opisano niżej. Cały cel pozostaje otwarty.

## Duże modele w przeglądarce

`large-profile-ui.json`:3PASS78,1s —8/20/40, rzeczywisty formularz/privateGLB i niezależnie odczytane1 567 758/1 487 438/1 677 324 widoczne trójkąty (zeroArea0);zoom/piksele/overflow w1440/390 i obu motywach,12ujęć obejrzanych. `large-profile-ui-price.json`:1PASS78,9s —20 przez wybór opcji/slicer/koszyk, kwota7686gr zgodna z UI i aktywny checkout; osobny przebieg od wcześniejszego7687gr. Nie jest to test płatności/rentowności.

31PASS6,4s (`glb-and-model-limits-contracts-final.json`):24parsera+7kontrolowanych metadata/uploadów, bez dowodu dostawców. Początkowy błąd kolekcji: zdublowana nazwa testu liczby/stringa, poprawiony. Zwykły profil:12PASS/1FAIL na starym oczekiwaniu25MB; po zmianie32MB i granicy32MB+1, osobne2PASS10,1s obejmują cały naprawiony scenariusz oraz poprawny ASCII STL25MiB+1 z whitespace→201/privateGLB12tri. To test granicy pliku, nie złożoności. Surowe raporty `large-ui-standard-regression.json`/`large-ui-standard-boundary.json`; nie sumujemy ich w pełny zielony zestaw.

GET/api/models/limits udokumentowany automatycznie, publiczny także przy zajętej analizie; UI przyjmuje tylko32/96MiB, awaryjnie25. GLB do2mln trójkątów/25MiB, lokalnySTL25MiB/1mln i normalizacja/shadery/styl bez zmian. Backend profil nadal niewłączony na serwerze użytkownika. Buildfrontend0błędów/ostrzeżeń/backend+HTTPunit+Clippy/format sprawdzone.

Na etapie integracji podglądu review wskazał brak zwalniania serwerowej kwoty po usunięciu/resecie modelu oraz obsługi spóźnionego uploadu; poprawkę opisuje M2 poniżej. To blokuje deklarację ogólnej gotowości dużego profilu, nie lokalną próbę opt-in. Następny etap: właściciel/CSRF, bezpieczne sprzątanie, ochrona kolejki/holdów i próby wyścigu; plan w large-models.md. Pamięć: do144MB vertices+72MB positions+GLB, możliwa kopia trim/GPU/cache; nie potwierdzono fizycznego telefonu. Cały cel otwarty.

Końcowy review Sol xhigh: poprawność9/bezpieczeństwo9/kwoty9/zasoby7/błędy8/współbieżność8/dowody8/prostota8/utrzymanie8/zgodność wizualna9. Pozostaje P2 zwalniania kwoty po usunięciu i spóźnionych uploadów; lokalna próba opt-in ma dowody, ogólnej gotowości nie zadeklarowano. Poprawiono dwie nieaktualne wypowiedzi raportu wskazane przez review; nie zamykamy celu.

## M2 — usuwanie i odzyskiwanie kwoty

`model-removal-complete.json`:10PASS17,5s0skip/flaky. Realne204/plik fizycznie znika/metadane404, obca sesja odmówiona, brak/złyOrigin403, konto bez/z błędnymCSRF403; po usunięciu dużego8 ten sam gość przyjmuje20 (suma przekraczałaby256MiB). Spóźniony zatwierdzony upload po lokalnym usunięciu lub próbce nie wraca; kontrolowane503 zachowuje model/pokazuje błąd, a rzeczywiste ponowienie sprząta. Cookie gościa dodane po logowaniu pozwala usunąć tylko jego nieprzejęty model bez przenoszenia pozostałych danych; to emulacja warunku wyścigu, nie pomiar logowania w trakcie konwersji. Advanced: rzeczywisty201, potem kontrolowany błąd katalogu/DELETE; przyjęty plik sprzątany teraz lub przed kolejną operacją.

`model-removal-storage-final.json`:22PASS0skip na osobnych bazach; activelease/livehold chronione, kolejka/nowyhold nie wskrzeszają usuniętego (testy); claim wyklucza oznaczone rekordy (przegląd kodu), mark nie zwalnia kwoty przed forget, guestfallback nie narusza kont. `model-removal-standard-regression.json`:3PASS20,9s — próbki/nawigacja, advanced upload/remove, wszystkie formaty. Wcześniejsze7PASS przed dodatkowymi poprawkami;9testów8PASS/1FAIL przez zgubione body inspektora Chromium, poprawiono przechwyceniem rzeczywistego ID przed przekazaniem odpowiedzi, bez dodatkowego uploadu. PierwszyClippy2ostrzeżenia test-only clone, finalnie naprawione. Builds/Svelte0/0/ClippyAPI+storage/format/diff PASS.

DELETE ma dokładnyOrigin i CSRF konta, owner-bound UUID, wspólny lockkolejki i trwały mark, fizyczne usunięcie przed metadanymi/kwotą,409 dla runningjob/holdzamówienia. Brak nowej migracji/zależności/CSS, profil large nadal niewłączony. Sol xhigh: bez konkretnego kolejnego blokera tej zmiany, oceny8/9/N/A/8/8/8/9/7/8/9 (poprawność/security/kwoty/zasoby/błędy/współbieżność/dowody/prostota/utrzymanie/wygląd), bez własnego rerunu. Retryznanych ID w pamięci strony; zamknięta strona/nieznana odpowiedź korzysta z24h retencji. Nie deklarujemy ogólnej niezawodności lub10/10; cel pozostaje otwarty.

## Model29 — przestrzenny zapis bez utraty geometrii

Nowy fallback sortuje całe trójkąty tylko po porażce pasujących dotychczas zapisów. Klucze Morton są przybliżone, **pozycje nie są kwantowane**: f32/winding/liczność zachowane, produkcyjny STL bez zmian, stare pasujące GLB bajtowo identyczne. Minimum referencjonowanych pozycji+indeksów pozwala odrzucić niemożliwy wynik przed klonem/sortowaniem. Nadal64bloki/64KiBJSON/25MiBGLB i ten sam sandbox.

SHA29: GLB29 224 268→24 171 984B,996139vertices/1993410tri/dims54,999817×55,22171×65,87768mm bez zmian. Najnowsza diagnostyka `large-model-probe-spatial-minimum`:13,975swall,~449MiBRSS,19OutputTooLarge11,497s; brak nowej obsługi19. Pierwsza udana próba29 przed minimum:13,955s/~448MiB. Program `large_model_probe /input.3mf --bounded-preview` używa25MiB, domyślnie128; diagnostyka bez STL/metre transformu/wyceny, źródła/hashes prywatnie zachowane.

API29 najpierw1PASS40,1s (4180gr), następnie1PASS46,1s z niezależnym pełnym porównaniem bitów wszystkich1 993 410 zorientowanych trójkątów GLB do STL, wraz z powtórzeniami; surowe bufory i indeksy sortowane niezależnie od produktu. Rachunek materiału/energii/obsługi/straty/marży/VAT z rzeczywistych pomiarów i stawek fixture. To drugi przebieg4178gr, bez obietnicy stałej/rentownej ceny. `spatial29-ui-price.json`:1PASS72,2s,1440/390/light/dark, wszystkie1993410tri widoczne,zeroArea0,zoom i UI4178gr zgodne z koszykiem;4ujęcia obejrzane. Podgląd zachowuje orientację pliku, nie przedstawia automatycznego ułożenia slicera.

Geometry42PASS/1opcjonalnyignored, `spatial-default-models.json`8PASS7,9s. Normal app/example build, Clippygeometry/APIall-targets, format/diff PASS; brak zmian UI/shaderów/deps/.env/profilu. `spatial29-final-exact-api.json`: 1 PASS / 46,1 s na końcowym kodzie z kontrolą minimum; pełne porównanie geometrii i pomiar ceny 4178 gr. Cały cel pozostaje otwarty, zwłaszcza19/41.

## Model 19 — pełny STL i ograniczony podgląd

Wdrożono awaryjny podgląd `meshopt`, bez zmiany źródła/kanonicznej siatki, profilu domyślnego, limitów ani stylu Piotra. Metryka do 0,05 mm nie jest gwarancją maksymalnej odległości powierzchni. Granice, zakotwiczenie i spójność części oraz limity wyniku są kontrolowane. Liczby źródłowe/wyświetlane i uproszczenie są jawne w GLB oraz podpisie UI; masa nigdy nie pochodzi z przybliżonego GLB.

`approx19-api-final.json`: 1 PASS / 31,4 s. Rzeczywiste API zapisuje pełny STL 193 050 084 B / 3 861 000 trójkątów i prywatny GLB poniżej 25 MiB / 2 mln trójkątów. Oryginał SHA, wymiary, kontrola właściciela i masa unavailable sprawdzone. Model przekracza 500 mm: API zwraca jawne 422 manual_quote_required/size_limit, bez uruchamiania pozornej wyceny.

Weryfikacja: 45 testów geometrycznych PASS / 1 wcześniej opcjonalny ignored przed uwagą review; po poprawce dodatkowe 4 testy modułu preview PASS, w tym fallback mniejszej siatki ograniczonej bajtami. Frontend check/build PASS, parser GLB 25 PASS, Rust build i Clippy geometry/API PASS; audyt na lokalnej bazie: 0 podatności / 1 wcześniejsza informacja unmaintained. Pierwszy test API miał stare oczekiwanie braku extras; pierwszy UI odczytywał liczbę z nieaktualnego podpisu. Po jego poprawce obrazy/zoom/cztery widoki przeszły, ale test próbował ocenić ręczną wycenę bez wybrania jakości/wytrzymałości; poprawiono kolejność w teście. Surowych nieudanych przebiegów nie usunięto. Finalny `approx19-ui-confirmed.json`: 1 PASS / 44,6 s, bez skipped/flaky; upload, GLB, niezależna liczba renderowanych trójkątów, piksele/zoom, 1440/390 px i oba motywy, jawna ścieżka ręczna po kompletnym wyborze opcji. Cztery zrzuty zapisane, desktop ciemny i mobile jasny obejrzane. `approx19-standard-models.json`: 8 PASS / 7,7 s. Końcowy Clippy PASS. Review Sol xhigh bez kolejnego blokera, oceny 8/9/8/8/8/8/9/7/7/9 w kolejności poprawność/bezpieczeństwo/kwoty/zasoby/błędy/współbieżność/dowody/prostota/utrzymanie/wygląd; nie jest to pełny audyt celu. Nie zamyka to całego celu; model 41 nadal otwarty.

## Model 41 — przekroczenie budżetu całego zespołu

Niezależnie odczytany graf ma 16 instancji i 6 257 906 trójkątów (STL 312 895 384 B), ponad profil large. Licznik nie potwierdza pełnej poprawności pakietu. Nowa ograniczona wskazówka `three_mf_budget.rs` odrzuca osiągalny zespół przed drogim SDK; nieużywane siatki nie podwyższają wyniku. Kandydaci mieszczący się w limicie zawsze przechodzą pełną dotychczasową walidację. Limity, zależności i profil domyślny bez zmian.

Pierwszy `budget41-api.json`: 1 FAIL / 2 PASS; poprawne &amp; w metadanych wyłączały wskazówkę, więc model 41 nadal dawał 503 po 25,2 s. Dodano obsługę predefiniowanych/numerycznych referencji tekstowych i regresję; DTD/nieznane encje nadal wyłączają wskazówkę. `budget41-api-fixed.json`: 3 PASS / 103,9 s, bez skipped/flaky. Model 41 daje jawne 422 model_resource_limit i zwalnia worker dla poprawnego STL (13,4 s razem); model 19 działa (37,1 s), model 20 zachowuje rzeczywistą wycenę (52,7 s). `budget41-ui.json`: 1 PASS / 27,6 s, błąd widoczny, brak udawanej ceny/siatki, kontrolki działają. Ten PASS oznacza poprawną obsługę odmowy, **nie obsługę geometrii modelu 41**.

50 testów geometrii PASS / 1 wcześniej opcjonalny ignored; build, format, Clippy PASS. Review Sol xhigh bez kolejnego blokera; oceny 8/9/N/A/8/9/9/9/7/7/N/A (poprawność/bezpieczeństwo/kwoty/zasoby/błędy/współbieżność/dowody/prostota/utrzymanie/wygląd), bez własnego rerunu. Dalsze dowody całego korpusu i audyt celu pozostają otwarte.

`budget41-standard-models.json`: 8 PASS / 7,6 s, bez skipped/flaky, na końcowym kodzie kontroli zespołu; zakres zwykłych modeli/odmów/retencji/izolacji/admission. Nie jest to ponowienie całego 41-elementowego korpusu.

## Pełny korpus API po rozszerzeniu modeli — 2026-11-06

`corpus41-large-current.json`: **41 scenariuszy PASS / 599,7 s**, 0 skipped/flaky. Produkt `df8760d`, profil large wyłącznie w izolowanej bazie/storage runnera, dostawcy wyłączeni. Źródła testu/runnera/manifestu oraz SHA binarki zachowane w prywatnym `corpus41-large-current-provenance/`. Profil serwera użytkownika nie został zmieniony.

| Rzeczywisty wynik | Liczba | Co udowodniono |
| --- | ---: | --- |
| Cena automatyczna | 24 | Upload, prywatny GLB, rzeczywisty Bambu i pełny niezależny rachunek kosztów/strat/marży/VAT/minimum |
| Wycena ręczna | 12 | Upload i GLB działają; odmowa automatycznej ceny ma jawny powód |
| Wadliwe źródło | 4 | 14/21/37: niepoprawna topologia; 32: wadliwy XML; nie przetworzono geometrii |
| Limit zespołu | 1 | 41: 6,26 mln trójkątów przekracza budżet; nie przetworzono geometrii |

Łącznie powstało 36 prywatnych podglądów. PASS testu odmowy nie oznacza obsługi modelu. Ceny sprawdzono na jawnej fixture 100 PLN/kg i stawkach runnera, **nie na zweryfikowanych rzeczywistych kosztach firmy**; nie jest to certyfikat rentowności. API nie dowodzi renderowania zdjęć ani wszystkich stanów UI.

Review wykrył zbyt słabą kontrolę ręcznej ścieżki. Po poprawce `corpus12-manual-geometry.json`: **12 PASS / 68,7 s**, 0 skipped/flaky, wszystkie rzeczywiste ręczne przypadki (2/5/15/16/18/19/22/24/25/28/31/36). Test niezależnie czyta surowe pozycje GLB, sprawdza wymiary i powód >500 mm lub brak ortogonalnego ułożenia P1P; mały drukowalny model nie przejdzie przez błędne manual. Późniejsza kontrola typu/długości BIN: `corpus-bin-guard-final.json` **1 PASS / 1,1 s** na przypadku 16. Nie przypisujemy nowych asercji wcześniejszemu pełnemu przebiegowi 41.

Klasyfikacja błędów w pełnym teście UI poprawiona analogicznie i zebrano 41 testów; wykonanie całego UI pozostaje oddzielnym krokiem. Nie poluzowano wymagań dla nieoczekiwanego 422/503 lub błędu slicera. Review Sol xhigh bez kolejnego blokera tego zakresu: poprawność 8, bezpieczeństwo N/A poza prywatnym odczytem, kwoty 8, zasoby 8, błędy 9, współbieżność N/A, dowody 8, prostota 8, utrzymanie 7, wygląd N/A. Reviewer nie powtarzał testów. Cały cel, aktualny core, konta/dostawcy i audyt mapy kontrolek pozostają otwarte.

## Pełny widok korpusu i poprawka testu — 2026-11-06

`corpus41-ui-current.json`: **37 PASS / 4 FAIL / 714,9 s**, bez skipped/flaky. W przypadkach 5/9/22/25 po wykonaniu kontroli obrazu test odwoływał się do `largeCase` spoza zakresu. Błąd pochodził z omyłkowo skopiowanej kontroli modelu 19, którego nie ma w tej grupie. Usunięto 9 zbędnych linii; prawdziwa kontrola 19 została. `corpus-ui-four-fixed.json`: **4 PASS / 86,2 s**, bez skipped/flaky. Surowe błędy i raporty zachowano. Hash fragmentu dowodzi, że źródło pozostałych 37 testów nie zmieniło się między przebiegami (`corpus41-ui-current-provenance/fix-scope.json`). **Nie jest to jeden czysty przebieg 41 PASS.**

Pokrycie obu przebiegów: 36 przyjętych modeli, piksele/zoom, 1440/390 px, jasny/ciemny motyw, oryginalna nazwa/title, brak poziomego overflow. Głębsze przypadki sprawdzają prywatność i ręczną wycenę; 9/20/29 również rzeczywistą cenę UI zgodną z serwerem. Pięć źródeł odrzuconych ma jawny błąd i działające kontrolki, bez udawania podglądu/ceny.

Powstało 144 unikalnych ujęć (160 plików z duplikatami czterech ponowień). Wszystkie 144 obejrzano na sześciu prywatnych zestawieniach `corpus41-ui-contact/`. Układ Piotra zachowany, brak pustych podglądów/uciętych kontrolek w sprawdzonym zakresie. Rozproszone zestawy mają małe części przy widoku całego zespołu, szczególnie subtelne w ciemnym motywie; zoom działa. To nie certyfikat kontrastu ani test telefonu. Nie zmieniano kodu strony, shaderów ani stylu w tym etapie.

Dodano `npm --prefix frontend run check:integration`: szybki TypeScript check wszystkich integration spec, który wykrywa taką nieistniejącą zmienną przed uruchomieniem przeglądarki; PASS. Review Sol xhigh bez blokera poprawki: poprawność 9, bezpieczeństwo 8, kwoty 8, zasoby 8, błędy 9, współbieżność N/A, dowody 9, prostota 9, utrzymanie 9, wygląd 8. Reviewer dodatkowo obejrzał strony 3 i 5 zestawień; nie powtarzał testów i nie oceniał całego celu. Aktualny core/auth/email/checkout/dostawcy i audyt mapy kontrolek nadal pozostają do wykonania.

## Aktualny core — 2026-11-06, fe47828

`core-current.json`: **45 PASS / 1 SKIP / 320,4 s**, bez FAIL/flaky, lokalne rzeczywiste konta/TOTP, osobny PostgreSQL/storage, dostawcy wyłączeni. Jedyny pominięty test wymaga profilu dużego: `core-large-removal-current.json` **1 PASS / 16,2 s** w osobnym przebiegu z large i manifestem. Nie opisujemy tego jako pojedynczego 46 PASS. Stan źródeł/binarki po przebiegach: prywatny core-current.sources.json; HEAD był taki sam przy uruchomieniu i po, bez zmian produktu.

Sprawdzono: rzeczywisty slicer dla 1/10/1000 sztuk, sumy partii/płyt i niezależne zaokrąglenia kosztów, wiele modeli ze wspólnym przygotowaniem, podmienione/cudze wyceny; równoległych gości i deduplikację; odrzucenia/upload/retencję; własność/Origin/CSRF/usuwanie fizyczne/zwalnianie kwoty/spóźnione odpowiedzi; zmianę hasła i odwołanie sesji, passkey/TOTP/recovery; publikację ręcznej ceny przy utracie odpowiedzi i deduplikację outboxa; dokumenty prawne, publiczne/zalogowane widoki i wszystkie pięć pokazowych modeli. To testy lokalne, nie dowód doręczenia emaili lub działania Stripe/kuriera.

Review Sol xhigh bez konkretnego blokera tego zakresu: poprawność 9, bezpieczeństwo 8, kwoty 9, zasoby 8, błędy 8, współbieżność 8, dowody 8, prostota N/A, utrzymanie N/A, wygląd 8. Reviewer obejrzał raporty i odpowiednie testy; nie powtarzał wykonania ani nie oceniał całego kodu na podstawie testów. Prawdziwy auth/email/zakup/panele/dostawcy i pełny audyt mapy kontrolek pozostają osobnym krokiem.

## Rzeczywiste konta, email i dostawcy — 2026-11-06

Osobne przebiegi na produkcie fe47828 (bez zmian backendu w tym etapie), izolowane bazy/storage, tylko Stripe test/Furgonetka sandbox i poczta testowa do void@sieciowiec.xyz:

| Raport prywatny | Wynik | Zakres |
| --- | --- | --- |
| auth-journey-current.json | 2 PASS / 209,5 s | Hasło/email, wirtualny passkey/email, admin/TOTP; ręczna oferta, płatność, legalny email, częściowy zwrot z powiadomieniem, kopia chatu i odpowiedź SMTP widoczna raz |
| sandbox-current.json | 9 PASS / 164,0 s | Zakup automatyczny, locker/kurier, adres/faktura, Furgonetka/cena/cache, panele/produkcja/chat/pliki/uprawnienia, częściowe/pełne zwroty i wyścigi, katalog/cennik, porzucone płatności, etykieta PDF/tracking/anulowanie/przekazanie |
| registration-password-current.json | 1 PASS / 30,7 s | Dobrowolne konto hasłem, prawdziwy kod email, setup i prywatność |
| registration-passkey-current.json | 1 PASS / 28,5 s | Jak wyżej, z wirtualnym passkey |
| stripe-provider-proof-current.json | 1 PASS / 37,3 s | Niezależny GET Stripe: pobrano 2500 gr PLN, dokładnie 3 udane zwroty 500/1500/500 gr, suma 2500, te same identyfikatory rejestru, brak dodatkowego zwrotu |

Wszystkie powyższe bez skipped/flaky/FAIL, każdy raport osobno. Sandbox nie ma adnotacji provider-blocked: bieżący test potwierdził **podjazd DHL**, nie ogólną dostępność każdego przewoźnika. Historyczne carrierConnectionError DPD pozostaje odrębnym dowodem/ograniczeniem. To nie fizyczne dostarczenie paczki ani produkcyjne rozliczenie.

Wzmocnienie Stripe jest tylko testowe: read-test-stripe.py + mały wrapper Node. Przyjmuje trzy identyfikatory, czyta wyłącznie klucz sk_test_, wykonuje dwa stałe GET, odrzuca redirect/proxy/paginację/niezgodną korelację, ogranicza czas i rozmiar oraz emituje tylko bezpieczne pola. Nie zwraca client_secret/danych karty/odbiorcy. Lokalny check: 5 nieprawidłowych wejść odrzuconych przed odczytem klucza, 3 nietestowe klucze przed siecią; 0 rzeczywistych wywołań w tym checku. TypeScript i format PASS. Hash kodu/runnera/binarki po przebiegu: stripe-provider-proof-current.sources.json. Pełny sandbox sprzed wzmocnienia nie miał tej nowej asercji; potwierdzono ją osobnym pojedynczym E2E.

Review Sol xhigh bez blokera nowej kontroli Stripe: poprawność 9, bezpieczeństwo 9, kwoty 9, zasoby 9, błędy 8, współbieżność 8, dowody 8, prostota 8, utrzymanie 8, wygląd N/A; bez ponawiania testów przez reviewera. Osobne review auth/journey i sandboxa potwierdziły zakres i ograniczenia: passkey wirtualny, manualny journey z odbiorem osobistym, wiadomość admina nadana API/odpowiedź widoczna UI, brak szerokiego load testu i nie jest to ocena całego produktu 10/10.

Audyt mapy wskazał nadal brak złożonego E2E opłaconego gościa → konto/claim oraz RetryPayment po opuszczeniu płatności. Backendowe zasady claim i osobne rejestracje już mają testy; brakuje spiętego przebiegu przeglądarki. Zwykły localhost:18430 nadal serwuje starszy main/API, podczas gdy powyższe dowody dotyczą worktree i oddzielnego runtime testów; nie utożsamiamy ich z aktualizacją głównego serwera.

## Zakup gościa → konto: znaleziony i poprawiony błąd kontraktu

Nowy test złożony ujawnił rzeczywisty problem: po udanej płatności i claim zamówienie miało lokalny ID koszyka w lines.model_id zamiast UUID uploadu, co rozłączało metadane i pliki w panelach. `guest-claim-current.json`: 1 FAIL/13,1 s na tym porównaniu, zachowany jako reprodukcja.

Zapis pozycji jest teraz wiązany ze zweryfikowanymi plikami/wycenami w transakcji: line_id dla unikalnej pozycji/produkcji, model_id dla uploadu, quote_id dla konfiguracji. Kwoty, publiczne klucze cen i pakowania bez zmian. Klient/admin dobierają właściwe pliki/kolory/parametry, także dla dwóch konfiguracji jednego uploadu. Starsze dane są rekonstruowane tylko odczytowo przy dokładnym zgodnym draft/snapshot; nieprzywracalne pozostają bez zmian. Przegląd wykrył też SQL NULL w ręcznych draftach bez lines; query ograniczono do tablic i dodano regresję ręcznych zamówień po przygotowaniu i po płatności.

Osobne dowody po poprawce, bez skipped/flaky: `guest-claim-fixed-final.json` **1 PASS/22,9 s** (prawdziwy Stripe gościa, widoczny link konta, login/MFA i dodatni claim, jedno zamówienie, modele/pliki/GLB, ponowienie claim=0, odmowa staremu/obcemu gościowi, świeże logowanie bez cookie gościa); `order-identity-pricing.json` **1 PASS/5,0 s**; `order-identity-manual-compat.json` **1 PASS/39,2 s** (płatność ręczna/panele/produkcja/zwroty i niezależny Stripe GET). `identity-storage-current.json`: **22 PASS**, 0 FAIL/skipped/flaky, każda baza osobna. Dodatkowe testy jednostkowe/adapterów potwierdzają dwie konfiguracje/kolory jednego modelu, unikalne klucze, ilości produkcji, metadane i sumy; nie jest to ich pełny osobny UI/provider E2E. Rust/frontend build i check:integration PASS; finalny Vite build 5,2 s. Hash źródeł/binarki po sprawdzeniu w order-identity.sources.json.

Review Sol xhigh po poprawce NULL: brak kolejnego blokera; poprawność 9, bezpieczeństwo 9, kwoty 10, zasoby 8, błędy 9, współbieżność 9, dowody 9, prostota 8, utrzymanie 8, wygląd N/A. Reviewer nie uruchamiał testów. Odczyt starych zamówień może dodać do dwóch zapytań na wiersz listy admina (limit 100); bez szerokiego testu wydajności. Nie zmieniono stylów/układu. Nowy guest-claim.spec.ts jest w automatycznej grupie sandbox; wcześniejsze 9 PASS tej grupy nie obejmowało nowego testu i zmian tożsamości.


## Wznowienie odrzuconej płatności po opuszczeniu strony

Nowy rzeczywisty E2E wykrył błąd: Stripe zwracał requires_payment_method, ale PaymentReturn pokazywał formularz ponowienia tylko dla open. `payment-resume-current.json`: 1 FAIL na brakującym formularzu; wynik zachowany. Poprawka używa trzech obsługiwanych stanów PI, uwzględnia nullable URL płatności hosted i istniejące tokeny jasnego/ciemnego motywu w Stripe, bez zmiany układu.

`payment-resume-fixed.json`: osobne 1 PASS / 18,0 s. Wzmocniony `payment-resume-provider-fixed.json`: **1 PASS / 18,1 s** — rzeczywisty upload/wycena/odbiór, karta odrzucona, wyjście i powrót do tego samego PI, poprawna karta, paid po dwóch cyklach odczytu/przeładowania; ponowienie po opłaceniu 409. Jedno utworzenie płatności, brak mocków/seedów płatności. Trzy odmowy 404 innemu prawidłowemu gościowi, claim 401 i zmieniony identyfikator 404. Dwa bezpośrednie GET Stripe przez istniejący testowy helper potwierdziły succeeded, PLN, amount i amount_received równe kwocie serwera, zero zwrotów. Bez zdjęć/trace/video danych płatniczych. TypeScript, frontend build i format PASS.

Review Sol xhigh bez blokera; poprawność 9, bezpieczeństwo 9, kwoty 9, zasoby 8, błędy 8, współbieżność 8, dowody 9, prostota 8, utrzymanie 8, wygląd 8. Reviewer nie ponawiał testów. Ograniczenie: jeden rzeczywisty scenariusz odzyskania, bez pełnej macierzy stanów/redirectów Stripe i przełączania motywu w trakcie płatności. Test dodany do grupy sandbox; stare 9 PASS tej grupy go nie obejmowało. Nie jest to aktualizacja zwykłego serwera localhost ani zakończenie całego celu.


## Jeden upload, dwie konfiguracje i płatność

`same-upload-configs.spec.ts` obejmuje rzeczywisty login TOTP klienta/admina, jeden upload, dwa wiersze lokalnego koszyka i dwie rzeczywiste wyceny: 2 czarne standardowe części (3 ściany/20%) oraz 3 białe fine (5 ścian/40%). Nie ma przycisku duplikacji uploadu; wyłącznie utworzenie drugiego wiersza korzysta z jawnego szwu localStorage. Dalszy wybór ustawień, reload, przejście do zamówienia i płatność działają przez UI/adaptory, bez mocków lub seedów płatności.

`same-upload-configs-final.json`: **1 PASS / 19,9 s**, 0 skipped/flaky/FAIL. Rzeczywiste pomiary dały koszty bazowe 676 i 869 gr; z jednym bazowym przygotowaniem 1000 gr, rezerwą, marżą i VAT rachunek daje 4698 gr brutto. Test niezależnie sprawdza także przydział kwoty do każdej pozycji, jednostkową cenę i sumę. Dwa bezpośrednie GET Stripe potwierdziły succeeded/PLN/amount_received=4698 oraz brak zwrotów. Klient i admin mają dwie odrębne pozycje z tym samym uploadem, różnymi quote ID, prawidłową ilością/kolorem/ustawieniami/kwotą; oryginały zgodne bajtowo, prywatne GLB dostępne, obie pozycje widoczne po reloadzie. Bez zdjęć/trace/video danych logowania/płatności. Build/typecheck/Prettier PASS; spec w grupie sandbox.

Cztery początkowe FAIL to błędy harnessu: brak zebranej natychmiastowej wyceny z cache, angielska etykieta koloru w API, body po nawigacji usunięte z inspektora oraz odczyt admina przez listę zamiast nieistniejącego endpointu szczegółów. Raporty current/rerun1/rerun2/rerun3 zachowano; rerun4 1 PASS / 20,9 s przed wzmocnieniem kontroli kwot. Nie zmieniano produktu, by dostosować go do testu.

Review Sol xhigh wykrył brak przypisania kwoty do konkretnego wiersza; uzupełniono asercje i powtórzono tylko scenariusz. Finalnie bez blokera. Oceny dowodu tego testu: poprawność 9, bezpieczeństwo 6, kwoty 9, zasoby 5, błędy 4, współbieżność 2, dowody 9, prostota 7, utrzymanie 6, wygląd 4. Niskie oceny poza zakresem nie są nowymi dowodami błędów produktu: ten scenariusz nie bada obciążenia, obcych kont ani wyglądu. Nie zastępuje ich oddzielnych testów i nie jest oceną całego celu.


## Mapa dowodów i wspólna przeglądarka runnerów

Aktualne wymagania/testy/raporty/ograniczenia zebrano w [e2e-coverage.md](e2e-coverage.md); odświeżono numery źródeł 447 kontrolek oraz mapy limitów i produkcji. Usunięto nieaktualne blanketowe twierdzenie, że termin/postęp/notatki właściwego panelu admina są disabled — test panels potwierdza rzeczywisty zapis, starszy widok nadal ma ograniczenia.

run-all i run-integration nie wybierają automatycznie systemowego Chromium; korzystają z tej samej przypiętej wersji co preflight, z zachowaniem jawnego override. Test statycznych ścieżek z symulacją obecnego systemowego Chromium oraz rzeczywisty start bundled PASS. `browser-default-consistency.json`: 1 PASS, bez skipped/flaky, rzeczywista izolowana aplikacja/Swagger, bez wywołań providerów. Poprzednie rzeczywiste raporty tego hosta nie zmieniają znaczenia: /usr/bin/chromium tu nie ma, więc już korzystały z bundled.

Review Sol xhigh wskazał początkowo pozostawiony fallback drugiego runnera; usunięto i sprawdzono. Finalnie bez blokera; poprawność 9, zasoby 8, błędy 8, dowody 8, prostota 8, utrzymanie 8; bezpieczeństwo/kwoty/współbieżność/wygląd N/A dla tej zmiany. Nie ponawiano całego zestawu ani testów przez reviewera.


## Końcowy zestaw na 78d5802

Komenda `python frontend/tests/run-all.py --group all --no-build` zakończyła się kodem **2**, nie 0: jeden opcjonalny test large jest pomijany przy standardzie. Wyniki, bez flaky i provider-blocked:

| Grupa | PASS | SKIP | FAIL |
| --- | ---: | ---: | ---: |
| storage | 22 | 0 | 0 |
| core | 45 | 1 | 0 |
| sandbox | 12 | 0 | 0 |
| email | 2 | 0 | 0 |
| rejestracja hasło | 1 | 0 | 0 |
| rejestracja passkey | 1 | 0 | 0 |
| contracts | 53 | 0 | 0 |

Łącznie **136 PASS, 1 SKIP, 0 FAIL**. To siedem grup z osobnymi bazami/scenariuszami. Sandbox obejmuje już nowe powiązania gościa, dwa warianty jednego uploadu i wznowienie PI; rzeczywiste Stripe/Furgonetka/iCloud sprawdzone. Contracts to osobne testy jednostkowe/kontraktowe, nie dodatkowe potwierdzenia providerów.

Pominięty scenariusz: `model-removal` opt-in large. Oddzielna komenda z --large-models --local-accounts, manifestem 41 i --grep 'opt-in large removal' dała final-large-removal-78d5802 **1 PASS/15,7 s**, 0 skip/flaky. Nie zmienia to wyniku oryginalnego wrappera; pokazuje pokrycie drugiego profilu osobno. Nie powtarzano korpusu 41 ani renderów 144 ujęć po zmianach tożsamości/PI; geometria pozostała bez zmian.

Przed zestawem zapisano czysty HEAD i 312 hashów śledzonych źródeł, testów, locków i rzeczywistej binarki; po zestawie i po large wszystkie zgodne. Prywatna niemodyfikowana kopia: final-suite-78d5802/{summary,sources-before,source-recheck,large-source-recheck}.json i raporty grup; wcześniejsze stałe nazwy zarchiwizowano przed uruchomieniem. Nie zapisano sekretów ani payloadów providerów w metadanych. Z aktualnych 153 zrzutów core obejrzano mobilne jasne szczegóły klienta i desktopowe ciemne szczegóły admina; bez przeprojektowania stylu.

Końcowy Sol xhigh nie znalazł nowej potwierdzonej regresji. Oceny lokalnego POC: poprawność 8, bezpieczeństwo 8, kwoty 8, zasoby 7, błędy 8, współbieżność 7, dowody 8, prostota 7, utrzymanie 7, wygląd 8. To oceny, nie pomiary lub dowód 10/10; reviewer nie ponawiał testów. Najmniejsza wskazana poprawka to aktualizacja starych checkpointów — wykonana w planie i mapie. Dalszy goal pozostaje otwarty, bez udawania obsługi wadliwych/ponadlimitowych źródeł, fizycznych urządzeń, szerokiego pentestu/load testu lub rentowności na kosztach fixture.


## Osiem równoczesnych klientów API

Rozszerzono istniejący concurrency.spec z 4 do 8 klientów, bez nowego frameworka lub zmian produktu. Każdy gość ma osobne cookie i model; uploady są sekwencyjne, aby oddzielić kolejkę slicera od admission parsera. Następnie 16 równoczesnych estimate (po dwa identyczne na klienta), ilości 1–8, rzeczywiste Bambu i sprawdzenie własnej wyceny/koszyka/cache. Każdy klient dostaje odmowę dla cudzego modelu, GLB, job, estimate i checkoutu. DB potwierdza dokładnie 8 zadań/8 snapshotów/max prób 1.

`eight-clients-current.json`: 1 PASS / 3,6 s, przyjęcie 88 ms, wyniki 2545 ms, 4 próbki katalogu 12/6/6/7 ms, aktywne zadania 1/1/1/0. Review znalazł drobną ścieżkę cleanup: błąd zapisu attachment mógł pominąć dispose klientów. Dodano zagnieżdżone try/finally; sampler zawsze zatrzymany i dołączony przed dispose.

`eight-clients-final.json`: **1 PASS / 3,8 s** (runner 4,5 s), bez skip/flaky. Przyjęcie 90 ms, wyniki 2611 ms; cztery próbki katalogu HTTP 200 w 13/8/8/8 ms, aktywne zadania 1/1/1/0. Bezpieczny attachment zawiera tylko pomiary/liczniki, bez ID/sekretów/providerów. TypeScript, Prettier i diff PASS. Brak builda lub ponownego pełnego zestawu — zmieniono wyłącznie test już należący do core.

Sol xhigh potwierdził cleanup po poprawce, bez nowej uwagi. Oceny dowodu testu: poprawność 8, bezpieczeństwo 7, kwoty 7, zasoby 6, błędy 6, współbieżność 8, dowody 8, prostota 7, utrzymanie 7, wygląd N/A. To osiem klientów API, nie osiem przeglądarek, równoczesny parsing, benchmark pojemności/RSS lub ciągły pomiar globalnego limitu. Nie ponawiano testów przez reviewera i nie jest to ocena 10/10 produktu.


## Nieużywane zależności Rust

cargo-machete 0.9.2, `cargo machete --skip-target-dir .` w backend: exit 0, nie znaleziono nieużywanych zależności. `cargo metadata --offline --locked --no-deps --format-version 1`: exit 0, 13 crate’ów workspace / 99 deklaracji zależności. Brak ignorelist machete. Główny agent potwierdził wynik osobnym uruchomieniem; zapis manifestów/SHA w prywatnym rust-dependency-audit-74bc144.json. Nie zmieniono źródeł/locka/wersji, bez instalacji, sieci lub przebudowy.

Sol xhigh porównał zapisane hashe i zakres ze wszystkimi manifestami, bez konkretnej uwagi. To negatywny audyt bezpośrednich zależności; nie dowodzi braku dead code, prostoty, bezpieczeństwa lub 10/10 całego produktu. Kategorie produktowe poza tym zakresem pozostają N/A, a zapis komendy nie zastępuje surowego pełnego transkryptu. Kryterium zakończenia według wymaganego 10/10 wymaga doprecyzowania; pytanie do właściciela pozostaje otwarte, bez samodzielnego zmieniania celu lub zawyżania ocen.


## Upload — błędy magazynu i późnego zapisu

Nowy upload-failures.spec sprawdza rzeczywiste błędy w izolowanym katalogu/bazie. Mały cube 684 B daje baseline 201; podmiana wyłącznie testowego katalogu models na plik wymusza ENOTDIR/500, a trigger odrzucający private_models INSERT po przygotowaniu siatki/GLB daje 503. Po obu błędach brak nowego katalogu/rekordu, wcześniejsze trzy pliki zachowują SHA i prywatne uprawnienia. Kolejny upload 201, źródło i GLB dostępne właścicielowi, obcy bez cookie dostaje 401. Root i trigger przywracane w finally; bez źródeł użytkownika, mocków, providerów, dużych plików lub builda.

upload-failures-current: 1 PASS/1,6 s. Review wskazał dwa problemy harnessu: licznik przed baseline uploadem nie uwzględniał legalnego sprzątania wygasłych modeli; częściowy błąd writeFileSync mógł zostawić plik blokujący odtworzenie katalogu. Baseline przeniesiono po uploadzie, plik tworzony explicit openSync/create-new, ownership flag przed zapisem i close/finally.

upload-failures-final: **2 PASS/2,5 s**, 0 fail/skip/flaky. We wspólnej bazie poprzedzał go istniejący test retencji (313 ms), nowy scenariusz 1,3 s. Pierwszy raport zachowany. TypeScript, Prettier i diff PASS; dodano spec do core. To nie nowy pełny zestaw 136 i nie test rzeczywistego ENOSPC, crashu, wyczerpania kwoty, RSS lub UI.

Scoped Sol xhigh potwierdził poprawki bez nowej uwagi. Początkowe oceny dowodu: poprawność 8, bezpieczeństwo 9, kwoty N/A, zasoby 8, błędy 9, współbieżność N/A, dowody 9, prostota 8, utrzymanie 7, wygląd N/A. Reviewer nie ponawiał testów. Produkt bez zmian — potwierdzono istniejący PendingDirectory rollback; pytanie o końcowe kryterium 10/10 pozostaje bez odpowiedzi.


## Konsultacja reviewera → dwie poprawki zasobów

Właściciel polecił konsultować konkretne działania z reviewerem zamiast czekać na definicję 10/10. Sol xhigh wskazał rzeczywiste problemy: files.rs czytał cały plik do Vec (do 200 MiB na żądanie) na wątku Tokio; jedyny slot uploadu nie miał deadline. Praktyczna konsultacja wyłapała dodatkowo ryzyko timeoutu całego handlera: anulowanie podczas commit mogło uruchomić PendingDirectory i skasować pliki zatwierdzonego rekordu.

Poprawka: timeout 60 s wyłącznie dla odbierania body, zachowany limit rozmiaru i przekazanie Bytes bez kopii; handler/commit poza deadline. Prywatne source/GLB/mesh korzystają z async odczytu jednego zweryfikowanego deskryptora O_NOFOLLOW/O_NONBLOCK, stałych porcji do 64 KiB i czterech permitów trzymanych do EOF/drop. Długość/typ/rola/właściciel/nagłówki pozostają sprawdzane. GLB weryfikuje ograniczoną kopertę zgodnie z dotychczasowym parserem, bez pełnego bufora; nieprawidłowe length<12 zwraca błąd zamiast możliwego panic starego parsera. OpenAPI opisuje 408/429 i 503 przy nasyceniu pobrań. Dwa helpery futures-util/libc już były w lockfile, bez aktualizacji wersji; test-util Tokio tylko dev.

Rust: 10/10 celowanych tests PASS po korekcie baseline RSS przed tworzeniem odpowiedzi; offline/locked app build PASS, Clippy api --lib -D warnings PASS (pozostał wcześniejszy warning kompilatora generowanego CXX). Cztery otwarte ciała po 200 MiB, przeplatane porcje, 800 MiB odczytane: sampled VmRSS 13012→13528 KiB (+516 KiB), porcje <=64 KiB. To nie pomiar HTTP lub produkcyjnego obciążenia. Osobna tymczasowa reprodukcja starego algorytmu na jednym 200 MiB pliku: RSS 2392→208380 KiB, Vec capacity 256 MiB; pliki diagnostyczne usunięte. Stare zamknięte cache incremental sprzed 2 h zwolniono pod Cargo/session locks (~2 GB), prywatny zapis transfer-build-cache-cleanup; bez źródeł/modeli/sekretów.

upload-timeout-http-backend-final: **2 PASS**, rzeczywisty raw backend HTTP 408 po 60,005 s, równoległy upload 429, limity 200; recovery przez Vite upload 201/GLB 200, outsider 401, DELETE 204. Drugi scenariusz to prawdziwy UI upload STL/STEP/3MF i prywatny GLB. Wcześniejsze FAIL zachowano: niedostępny adres IPv4 Vite (serwer preview słuchał ::1), a potem zbyt ścisłe oczekiwanie cache nagłówka proxy. Przez proxy sam timeout działał po 60,007 s; cały tamten test pozostał FAIL. Finalny raw test używa ustalonego API 127.0.0.1:18436 z poprawnym Host/Origin, recovery i przeglądarka przechodzą przez proxy; nie zmieniano produktu dla harnessu.

private-transfer-http-final: **3 PASS/28,1 s**, osobny aktualny build: Swagger, widoki rzeczywistego ręcznego zamówienia klienta/admina i prywatne podglądy oraz błędy filesystem/DB. Dodatkowo dokładne bajty/content-length kanonicznego mesh przez endpoint admina, klient do admin mesh 403 i własny endpoint mesh 404. TypeScript/Prettier/Rust fmt/diff PASS. Hashy nowej implementacji/binarki przed tym przebiegiem nie łączymy ze starym pełnym zestawem 136; pełnego zestawu nie powtarzano.

Sol xhigh po kodzie i wynikach potwierdził zamknięcie obu konkretnych usterek, bez nowego blokera w diffie. Ocena w dziesięciu kategoriach jest oparta na właściwych dowodach: poprawność/bezpieczeństwo/limity/błędy/współbieżność — celowane PASS, zasoby — kod bounded i unit RSS, dowody — osobne aktualne raporty, prostota — lokalna zmiana bez nowej warstwy, utrzymanie — OpenAPI/docs i Clippy, wygląd — bez zmiany frontendowego produktu. Nie oznacza to uniwersalnego 10/10; reviewer nie uruchamiał ponownie testów. Kryteria tego patcha są spełnione; wcześniejsze zatrzymanie z powodu samej liczby oceny nie było potrzebne.


## Trwałość modelu i niepewny COMMIT

Kolejna konsultacja Sol xhigh wskazała dwie konkretne luki w files/storage: sync plików i katalogu UUID nie utrwalał wpisu tego katalogu w rodzicu; dowolny błąd commit uruchamiał PendingDirectory cleanup, choć PostgreSQL mógł już zapisać rekord i zgubić potwierdzenie. Dodano fsync katalogu magazynu po child/files i przed insertem. Storage oznacza wyłącznie błąd tx.commit typowanym ModelCommitUncertain; API w tym przypadku zachowuje pliki i zwraca 503/loguje statyczny event+UUID. Błędy wcześniejsze i odmowa kwoty nadal sprzątają. Bez nowych zależności, migracji, zmiany publicznych sygnatur lub wyglądu.

Rozszerzony upload-failures: rzeczywisty BEFORE INSERT trigger nadal daje 503 i brak nowych plików; deferred constraint trigger zgłasza błąd przy COMMIT i daje 503 z kompletnym zachowanym katalogiem, bez rekordu. Wcześniejszy model zachowuje SHA/permisje/dostęp, a następny upload daje 201. Ten kontrolowany trigger oznacza **znany rollback traktowany konserwatywnie**, nie rzeczywiście zapisany rekord z utraconym ACK. Tylko własny testowy orphan usunięto po zakończonych żądaniach, usunięciu triggera i potwierdzeniu braku rekordu; to nie implementacja produkcyjnego GC.

upload-commit-durability-current: **1 PASS/3,0 s** (przypadek 2,4 s), bez fail/skip/flaky. Attachment: 201/500/503/503(commitOrphanRetained=true)/201. Offline locked app build, TypeScript, Prettier/fmt/diff PASS. Osobno 10 testów api files PASS/0 ignored; Clippy storage+api --lib -D warnings PASS (wcześniejszy warning generowanego CXX pozostaje). Zapis hashów po przebiegu w prywatnym upload-commit-durability.sources.json, nie deklarujemy snapshotu sprzed uruchomienia. Brak powtarzania całego zestawu/providerów.

Sol xhigh potwierdził kod/kolejność sync/typed marker i artefakt bez materialnego blokera. Kategorie: zmieniona poprawność/odtwarzanie/prywatność/regresja poparte dowodami; power loss i rzeczywista utrata ACK NOT RUN; wygląd/płatności bez zmian, pozostałe niepotwierdzone obszary nie dostają fikcyjnego 10/10. Pozostaje operacyjna granica: katalog bez rekordu nie jest automatycznie uzgadniany ani liczony w kwocie DB; wymaga ręcznej weryfikacji po ustaleniu końca transakcji. Nie usuwać go na podstawie chwilowego braku w SELECT. Nie dodawano rozbudowanego sweepera do tej poprawki.


## Retencja osieroconych katalogów po niepewnym zapisie

Usunięto poprzednią granicę wymagającą ręcznego sprzątania każdego katalogu bez rekordu. Istniejący minutowy worker uzgadnia realne kanoniczne katalogi UUID starsze niż 24 h. Upload bierze nieblokujący flock katalogu przed utworzeniem UUID i trzyma go przez fsync/commit/cleanup; orphan pass używa tego samego locka. Skanuje 64 wpisy na przebieg, wliczając junk/protected/błędy, i zachowuje ReadDir cursor z kontrolą path/dev/inode zablokowanego deskryptora root. Po EOF następny przebieg zaczyna nowy cykl. Nie ma stałego prefiksu ani LIMIT stale wybierającego te same nieusuwalne katalogi.

SQL w storage: READ COMMITTED/READ ONLY, osobny pg_try_advisory_xact_lock tym samym kluczem co insert, a dopiero potem odrębny SELECT. Wymagana nieobecność dowolnego rekordu private_models, aktywnego queued/running job z ważną datą/lease oraz aktywnego order_files hold. Zajęty lock lub błąd zapytania nie usuwa niczego; 5 s deadline tylko read-only helpera. Po decyzji root flock pozostaje do zakończenia unlinków/fsync; inne uploady nie mogą odtworzyć wpisu. Błąd usunięcia jednego katalogu loguje się i nie przerywa skanu.

orphan-gc-storage: **23 PASS/0 fail/skip/flaky**, każda baza osobna. Nowy ignorowany test API z realnym PG obejmuje stare/fresh/future katalogi, każdy zarejestrowany model także deletion_started, brakujący model chroniony queued/running-lease/holdem, symlink do własnego zewnętrznego sentinela, nie-UUID, root lock innym FD, zajęty advisory lock → później widoczny committed row, błąd SQL bez kasowania, kontynuację po IOerror, >64 wpisy i reset inode. Dodano jawny target do istniejącego run-storage.py, bez publicznego GC endpointu/nowej migracji/dependencji/daemona.

orphan-gc-upload-regression: **1 PASS/3,1 s** — rzeczywisty upload, ENOTDIR, błąd inserta, niepewny commit i recovery. Początkowy preflight wskazywał nieistniejący systemowy Chromium przed utworzeniem bazy (NOT RUN); właściwy przebieg użył bundled Chromium. Build offline/locked, fmt, Python compile, diff PASS. Main Clippy storage+api --all-targets -D warnings PASS/10,2 s; po dodaniu bezpiecznych logów fmt check wymagał braces, zastosowano fmt i końcowy build PASS/5,3 s. Logi UUID-only: removed/info, failed/error reason filesystem; ogólny reason storage zamiast mylącego database. Nie ponawiano pełnego frontowego zestawu/providerów.

Sol xhigh zatwierdził protokół i kod bez blokera dla pojedynczego zaktualizowanego lokalnego wdrożenia. Nie należy obracać magazynu w trakcie pracy: inode check dotyczy kolejnych przebiegów, nie atomowej rotacji root w środku przebiegu. Wszystkie upload writers muszą być zaktualizowane przed włączeniem cleanup. Restart resetuje kursor; częściowe usunięcie może odświeżyć mtime i opóźnić retry o 24 h. To okresowa retencja, nie ścisłe SLA/odzyskanie po power loss. Zapis hashów po testach w orphan-gc-verified.sources.json; późniejsza zmiana względem testów dotyczyła tylko logów/formatowania.

Przed planowanym rolloutem odczyt normalnego magazynu: 2 katalogi, oba z rekordem, 0 starych niezarejestrowanych i 0 innych upload writers tego samego root. To snapshot bez fence, nie lista autoryzująca kasowanie; wdrożenie wymaga stop-old, nowej kopii DB/plików i start-new. Dane normalnego magazynu nie były używane do fault injection.

## 2026-11-07 — profil wyceny przy przygotowaniu płatności

Końcowa transakcja checkoutu sprawdza teraz hash profili materiału pod blokadą rekordu katalogu, obok kosztu i stawek. Zmieniony, brakujący lub nieprawidłowy hash blokuje przygotowanie płatności; wcześniejsze sprawdzenie API pozostaje. Bez zmian UI, migracji i zależności. To zamyka okno pomiędzy odczytem API a blokadą katalogu; tożsamość plików runtime slicera jest osobną kontrolą API, nie blokadą PostgreSQL.

Istniejący test checkout_is_atomic_owned_idempotent_and_recovers_leases rozszerzono o zmianę profilu oraz zgodne null/nieprawidłowe hashe i brak hasha wyceny; po przywróceniu poprawnego profilu równoległe przygotowanie nadal działa. Dokładny test na jednorazowej bazie: 1 PASS/0 FAIL/0 SKIP, raport storage-profile-fence-retry.json. Pierwszy runner nie wystartował przez brak lokalnego .env (NOT RUN); powtórzenie użyło konfiguracji kontenera tylko w pamięci. Scoped build storage, rustfmt i diff check PASS. Sol xhigh przejrzał rzeczywisty diff: bez blokera; nie deklarujemy 10/10 ani pełnego ponownego E2E.

### Regresja przeglądarki po 269bc8d

`E2E_REPORT_NAME=checkout-profile-fence-269bc8d python frontend/tests/run-integration.py --sandbox checkout.spec.ts`: **3 PASS/0 FAIL/0 SKIP/0 flaky, 46,5 s**, terminal 0. Filtr nazwy objął checkout.spec.ts oraz shipping-checkout.spec.ts: odbiór osobisty, paczkomat i kurier. Rzeczywisty upload/slicer, formularze, oferta przewoźnika i płatność Stripe test; osobna baza/storage/API i przeglądarka, bez zmiany zwykłego localhosta. Raport prywatny checkout-profile-fence-269bc8d.json.

Sol xhigh przejrzał raport i źródła testów: dowodzą poprawnej ścieżki ze zgodnym profilem, nie zmiany hasha pomiędzy wyceną i płatnością. Odmowę niezgodnego/brakującego profilu sprawdzono na razie testem transakcji PostgreSQL; brakuje celowanego testu HTTP potwierdzającego brak rozpoczętej płatności po takiej zmianie. Raport przeglądarki nie stanowi samodzielnego audytu zapisów Stripe. Bez deklaracji pełnego pokrycia/10 na 10.

### Odmowa HTTP po zmianie profilu

Nowy profile-drift.spec.ts: rzeczywisty upload, slicer i poprawny checkout, następnie zmiana profilu GFA00 wyłącznie w jednorazowej bazie. POST /api/checkout/intent zwraca 409 z komunikatem o nieaktualnej wycenie; brak payment_attempts i orders, szkic nieblokowany, snapshot wyceny niezmieniony. Hash przywracany w finally; ponowne przygotowanie checkoutu z tą samą wyceną daje 200. Regex nazwy bazy/current_database chronią zwykłą bazę.

`E2E_REPORT_NAME=profile-drift-http-20261107-a python frontend/tests/run-integration.py --sandbox profile-drift.spec.ts`: **1 PASS/0 FAIL/0 SKIP, 4,3 s**. check:integration, Prettier i diff PASS. Sandbox wymagany przez konfigurację wejścia płatności; test nie płaci. Sol xhigh przejrzał test, runner i ścieżkę: bez blokera. To dowód HTTP precheck, nie wyścigu wewnątrz transakcji ani bezpośredniej obserwacji ruchu Stripe. Liczniki potwierdzają brak lokalnych operacji płatności. Bez zmian backendu/UI i normalnego localhosta.

## 2026-11-07 — dokładna prezentacja ceny pozycji

Review wykrył niespójność prezentacji: combine zachowuje dokładną sumę pozycji, ale cena jednostkowa jest zaokrąglana w górę. Checkout wyświetlał np. 3 × 0,21 zł obok sumy 0,62 zł. Teraz mnożenie pojawia się tylko, gdy kwota jednostkowa w groszach × ilość równa się dokładnej sumie; w przeciwnym razie wyświetlamy samą ilość. Bez zmian kwot backendu, stylów i układu Piotra.

Nowy tests/e2e/checkout-line-prices.spec.ts renderuje prawdziwy checkout przez adapter API z kontrolowanymi fixture: 62 gr/3 → sama ilość, 63 gr/3 → mnożenie, 29 gr/1 → poprawne mnożenie mimo reprezentacji float. To kontrakt widoku, nie dowód cen dostawcy ani slicera. Worker: scoped Playwright 1 PASS/1,7 s, build (svelte-check 0 błędów/ostrzeżeń + Vite), scoped TypeScript, Prettier PASS. Main diff check PASS; Sol xhigh przejrzał źródło/test: zatwierdzone bez blokera.

## 2026-11-07 — EPIPE a limit uploadu

Proxy Vite błędnie zamieniało EPIPE dla każdego uploadu ponad 32 MiB na 413, chociaż lokalny profil large dopuszcza 96 MiB. Teraz fallback dotyczy wyłącznie długości przekraczającej oba obecne profile (>96 MiB). EPIPE samo nie dowodzi odrzucenia rozmiaru; dla mniejszych plików pozostaje zwykły błąd proxy. Rzeczywista odpowiedź 413 backendu jest przekazywana bez zmian.

Nowy `npm --prefix frontend run test:proxy`: prawdziwy Vite + kontrolowany upstream TCP, porty tymczasowe, bufor 256 KiB. 48 MiB/EPIPE → 500, 97 MiB/EPIPE → 413, upstream 413 pozostaje identyczne. Worker wykonał negatywną kontrolę: stara konfiguracja FAIL, poprawiona PASS. Main uruchomił nowy skrypt: 1 PASS/0 FAIL, 0,24 s. Build/svelte-check (0 błędów/ostrzeżeń), Prettier i diff PASS. Sol xhigh: bez blokera; dodano skrypt i komendę w testing.md zgodnie z uwagą. Bez testu Axum/preview w tym zakresie i bez zmiany normalnego runtime.

GitHub alert35 ponownie sprawdzony: dev deepmerge4.3.1 pochodzi z vite-plugin-svelte6.2.1; najnowszy7.3.1 nadal wymaga deepmerge, advisory GHSA-gcqx-w48m-w2h4 nie wskazuje poprawionej wersji. Nie wyciszano alertu ani nie wykonywano nieskutecznego upgrade Vite. Znane ograniczenie w security.md pozostaje.

## 2026-11-07 — nieudana ponowna wysyłka MFA

Przy resend nowy verifier zastępował dostarczony kod przed SMTP. Po pewnym braku wysyłki użytkownik dostawał 503, a stary kod nie działał. auth_factor_start zwraca teraz niejawny token odzyskania zawierający wyłącznie keyed verifiers i daty. Dla zdecydowanie niewysłanych maili API przywraca poprzedni verifier/expiry przez CAS na sesji, nowym verifierze, sent_at i etapie second_factor. Nie cofa limitów wysyłki, cooldownu ani nieudanych prób; nie przywraca zużytej sesji i nie nadpisuje późniejszego resend. Uncertain pozostaje konserwatywnie bez przywracania. Bez zmian UI, migracji/deps.

4 celowane testy realnego PostgreSQL/API PASS (mfa-resend-rerun.json): powrót kodu A i odmowa B, zachowanie limitów/expiry, zużyta sesja/późniejszy kod C, istniejące budżety i 2FA oraz prawdziwy Axum factor/resend z wyłączoną pocztą → 503 i stary kod nadal poprawny. API test dodany do zwykłych targets run-storage.py; test storage odkrywany automatycznie. Scoped storage+api build, rustfmt i diff PASS. Pierwsza próba runnera nie wykonała testów przez błędną nazwę targetu; poprawiony przebieg wykonał wszystkie 4. Sol xhigh zweryfikował CAS i klasyfikację błędów lettre: bez blokera. Brak zewnętrznej awarii SMTP w tym teście; Uncertain sprawdzono źródłowo, konkurencję kolejnością operacji, nie stress testem.

## 2026-11-07 — aktualizacja zwykłego localhosta do ed1fcd0

API18432 uruchomione ze stabilnej prywatnej kopii aktualnego binarium (PID2613002), SHA d1fb8a716fc05871e087b4fe2f099105ed49a5ab5bf0b16cb0b9ce1d58021cd6. Poprzedni proces zakończony czysto; przed startem zweryfikowany pg_dump133074B i kopia2 katalogów modeli. Bez zmian .env/migracji/root magazynu. Vite18430 już przeładował konfigurację. Szczegóły prywatne local-runtime-ed1fcd0/metadata.json.

PASS na zwykłym localhost: limit96MiB, prawdziwy upload201, prywatny GLB200 i odmowa obcemu, identyczny oryginał200, estimate202 → native succeeded, usunięcie wyłącznie własnych uploadów204. GFA00/10100:1263s,3,72065g,1 płyta,37,05zł brutto. Sol xhigh niezależnie potwierdził hash uruchomionego procesu, wynik zadania PostgreSQL i brak modelu po DELETE. Nie wykonywano płatności ani emaili.

Niezależny rachunek wg obecnych stawek DB: filament33gr + maszyna176gr + energia7gr + obsługa500gr + przygotowanie1200gr =1916gr; strata192gr → koszt2108gr; marża30% → netto3012gr; VAT693gr → brutto3705gr. Zaokrąglenia w górę zgodne z kodem. Cena zakupu8780gr/kg wynika z roboczej konwersji POC, nie dowodzi waluty/netto presetów Bambu ani rentowności firmy. Review nie obejmuje obciążenia, wyglądu ani całego produktu; brak deklaracji10/10.

### Prawdziwe logowanie i email po ed1fcd0

`E2E_REPORT_NAME=auth-real-mail-ed1fcd0 python frontend/tests/run-integration.py --sandbox auth.spec.ts`: **1 PASS/0 FAIL/0 SKIP/0 flaky,30,3s**. Klient: hasło + świeży kod SMTP/IMAP, rejestracja i logowanie passkey Chromium + kolejny kod email. Admin w osobnym kontekście: hasło/TOTP, admin/orders200; klient403. Jednorazowa baza/API, MAIL_MODE=test wyłącznie void@sieciowiec.xyz; bez zmiany zwykłych kont. Raport prywatny auth-real-mail-ed1fcd0.json.

Sol xhigh przejrzał raport, runner, helpers i implementację maila: jest prawdziwe przyjęcie przez iCloud SMTP i odbiór przez IMAP ze świeżego UID watermark, nie mock. Passkey to wirtualny authenticator, nie fizyczne urządzenie. Sprawdzono poprawną wysyłkę send_factor po zmianie recovery, ale test nie wywołuje factor/resend ani nie powoduje awarii SMTP. Następna konkretna luka pokrycia: skuteczny resend z nowym odebranym kodem, unieważnieniem poprzedniego i użyciem nowego. Osobne4 testy PG/API nadal dowodzą zachowania starego kodu po pewnym niewysłaniu. Brak podstaw do deklaracji10/10 całego produktu.

## 2026-11-07 — rzeczywisty resend kodu MFA

Nowy auth-resend.spec.ts: prawdziwe logowanie hasłem do etapu second_factor, odbiór kodu iCloud, pełne61s cooldownu (bez zmiany DB), kliknięcie resend i nowy odbiór IMAP. Stary kod daje401 i brak dostępu do orders, nowy200 i authenticated/orders200. Replay kodu z zachowanym zużytym cookie pending+CSRF daje401 dla verify/me/orders. Kolizja losowych kodów ma jeden ograniczony retry; wartości kodów nie są drukowane, trace/screens/video wyłączone.

`E2E_REPORT_NAME=auth-resend-real-20261107 python frontend/tests/run-integration.py --sandbox auth-resend.spec.ts`: **1 PASS/0 FAIL/0 SKIP/0 flaky,79,5s** według raportu. Integration TypeScript, Prettier i diff PASS. Sol xhigh przejrzał test/runner/raport: zakres potwierdzony; IMAP używa wspólnej skrzynki i UID watermark, bez korelacji z konkretną sesją, więc równoległe inne testy email mogą zakłócić odbiór. Nie dowodzi throttlingu (osobne testy), awarii SMTP ani całego produktu.

Dodano spec do grupy email w run-all.py (sandbox ma fixture TOTP, więc nie nadaje się do tego testu). Podłączono także już osobno sprawdzone profile-drift do sandbox i checkout-line-prices do contracts, aby zwykłe zestawy ich nie pomijały. Składnia runnera sprawdzona; pełnych grup po zmianie list nie ponawiano.

## 2026-11-07 — odzyskanie podglądu po utracie WebGL

Naprawiono trwały pusty canvas po webglcontextlost/restored. Viewer odtwarza program/bufory/lokacje/tekstury, przesyła zachowaną siatkę CPU i rysuje bieżący stan. PrintScene pokazuje istniejący fallback i blokuje kontrolki podczas utraty; pauzuje tweeny, a po uwadze review również pomija ticki rysowania. Dispose usuwa oba listenery. Bez redesignu/deps/backendu.

Rozszerzony showcase.spec.ts: model ASA pokazowy i przesłany12-trójkątowy STL, po2 rzeczywiste cykle WEBGL_lose_context. Sprawdza fallback, kontrolki, linked program/uniforms, niepustą sylwetkę, zachowanie ustawień i zmianę pikseli po obrocie. webgl-context-restore-20261107-final.json: **2 PASS/0 FAIL/0 flaky**. Negatywna kontrola oryginalnego kodu FAIL (brak fallbacku). Początkowy test miał1FAIL/1PASS przez symetrię obracanego PETG; użyto asymetrycznej obudowy ASA, bez dostosowania produktu pod test. Obejrzano końcowe ujęcia; main dodatkowo obejrzał ASA.

Build/svelte-check0, integration TypeScript, test Prettier i diff PASS. Pliki renderer/component miały błędy całoplikowego Prettier już na HEAD; bez masowego formatowania. Po końcowym dodaniu guardu unavailable do ticka main svelte-check0/diff PASS, bez powtarzania2 E2E. Sol xhigh: ścieżka skutecznego restore bez blokera. Granice: Chromium reduced motion/ASA, nie fizyczna awaria GPU; zmiana siatki w trakcie loss sprawdzona źródłowo, nie testem. Błąd odbudowy zasobów pozostawia bezpieczny fallback, bez osobnego retry; ścieżka częściowej awarii nie objęta testem. Nie deklarujemy10/10 wszystkich stanów renderowania.

## 2026-11-07 — wspólna regresja po poprawkach

Na3519cb9 przebudowano frontend (svelte-check0/Vite PASS) i uruchomiono storage/core po zebranych zmianach uploadów, profilu płatności, MFA i renderera. Pierwszy storage:24PASS/1FAIL, ponieważ etykieta factor-resend generowała niepoprawną niecytowaną nazwę DB; CREATE/DROP nie wykonały testu MFA. Zmieniono wyłącznie etykietę na factor_resend. Ponowny pełny storage: **25PASS/0FAIL/0SKIP/0flaky** (batch-storage-fixed-3519cb9).

Core z dokładnej listy run-all.py: **49PASS/0FAIL/0flaky/1SKIP**,392,4s, proces terminal0 (batch-core-current-3519cb9). Obejmuje pomiary cen1/10/1000, multi-model, konfigurator, wizualne zasoby/motywy/viewporty, WebGL restore, bezpieczeństwo kont, retencję i błędy/timeout uploadu. Skip: opt-in large removal — w tym przebiegu brak profilu large/prywatnego manifestu. Nie zmieniono wyniku skip na PASS. Providerów/email/refundów nie powtarzano w tej regresji.

Przed core zapisano hashe śledzonych źródeł, bieżącego binarium i dist; po przebiegu identyczne (batch-core-3519cb9/result.json + sources-before.json). Pierwszy wrapper storage/core zatrzymał się na harness FAIL; jego raporty zachowane batch-regression-3519cb9/. Sol xhigh przejrzał błąd runnera, poprawkę i oba końcowe raporty: brak blokera w tym zakresie, nie akceptacja całego produktu10/10.

### Osobny przebieg pominiętego dużego uploadu

Na czystym7961feb: `E2E_CORPUS_MANIFEST="$PWD/backend/test-models/makerworld/manifest.json" E2E_REPORT_NAME=large-removal-current-7961feb python frontend/tests/run-integration.py --large-models model-removal.spec.ts --grep 'opt-in large removal'` → **1PASS/0FAIL/0SKIP,16,7s**. UI upload korpusu8, DELETE204, katalog znika/GET404, upload korpusu20 przyjęty; suma oryginałów/STL/GLB obu przekracza256MiB, więc odzyskanie quota potwierdza drugi upload. Manifest size/SHA sprawdzony, osobne DB/storage, MAIL disabled, bez zmian zwykłego localhosta.

Sol xhigh przejrzał test/runner/raport/backend: wynik zatwierdzony w tym zakresie; zwolnienie quota wynika z drugiego udanego uploadu, nie bezpośredniego zapytania DB. Nie jest to test cen, współbieżności, GPU ani zachowania ponad aktualnymi limitami. Poprzedni core49PASS/1SKIP pozostaje niezmieniony, ten1PASS jest osobnym przebiegiem.

## 2026-11-07 — aktualna regresja Stripe/Furgonetka/paneli

Pełny sandbox na1ae9ed6: **12PASS/1FAIL/0SKIP/0flaky**,273,8s, wrapper exit1, bez provider-blocked. PASS obejmuje płatności i wznowienie, guest claim, dwie konfiguracje jednego uploadu, częściowe/pełne zwroty, pliki/chat/uprawnienia obu paneli, katalog/cennik, paczkomat/kuriera, ponowne użycie oferty oraz etykietę/anulowanie/przekazanie sandbox. To operacje testowe, nie fizyczna dostawa/rozliczenie bankowe. Źródła/binarium/dist niezmienione; raport i hashe zachowane batch-sandbox-1ae9ed6/.

FAIL profile-drift był błędem testu: przed własną mutacją wymagał zerowej liczby wszystkich payment_attempts/orders, choć wcześniejsze testy utworzyły8. Zawężono liczniki do własnego draft_id i właściciela świeżego gościa. Pozostają twarde asercje: własne0 przed,409, własne0 po, szkic odblokowany i snapshot niezmieniony. Nie usunięto sprawdzania skutków płatności.

Celowane ponowienie z rzeczywistą wcześniejszą płatnością w tej samej DB: `E2E_REPORT_NAME=profile-drift-after-payment-20261107 python frontend/tests/run-integration.py --sandbox --client-totp checkout.spec.ts profile-drift.spec.ts --grep 'real automatic pickup|full HTTP checkout'`: **2PASS/0FAIL/0SKIP,18,0s**. Sol xhigh zatwierdził zakres SQL/izolację; TS/Prettier/diff PASS. Pierwsze Prettier z root nie znalazło pluginu; poprawna komenda z frontend PASS. Nie ponawiano wszystkich13 providerowych scenariuszy po zmianie wyłącznie testu;12PASS+osobne2PASS nie jest jednym czystym przebiegiem13. Brak bezpośredniej obserwacji braku ruchu Stripe w odmowie409; test HTTP nie odtwarza wyścigu w transakcji.

## 2026-11-07 — dwa warianty jednego uploadu przez UI

Dodano niewielką akcję „Powiel wariant” obok usuwania, bez redesignu Piotra. Przyjęty plik otrzymuje nowy lokalny row UUID i niezależną kopię ustawień/ilości, zachowując wspólne analysis.id, File i mesh; limit10 pozycji. Storefront i zaawansowany konfigurator zwalniają model dopiero przy usunięciu ostatniej referencji. Przywracanie checkoutu wiąże ustawienia po row ID, więc jednakowa nazwa/analysis nie miesza wariantów.

Nowy core w model-removal: **1PASS/13,6s** (duplicate-variant-core-retry): limit10, niezależna edycja, reload/powrót z checkoutu, advanced usuwa pierwszy wariant bez DELETE, drugi nadal wyceniany, ostatnie usunięcie daje204/404, katalog znika, DBquota0|0 i następny upload działa. Pierwszy przebieg FAIL przez nawigację przed końcem asynchronicznego usuwania; dodano oczekiwanie na liczbę wierszy, raport retained duplicate-variant-core.

Zmodyfikowany same-upload-configs używa rzeczywistego przycisku zamiast wstrzyknięcia localStorage: **1PASS/21,7s** (duplicate-variant-same-upload), prawdziwy slicer/Stripe test, osobne konfiguracje/kwoty i oba panele. Build/Svelte0, integrationTS/diff PASS. Obejrzano desktop/390px; main dodatkowo390px. Sol xhigh: bez blokera, refs/deepcopy/rowID zweryfikowane. Bez testu wielu kart jednocześnie lub fizycznego telefonu; starsze wpisy o brakującym przycisku opisują stan historyczny.

## 2026-11-07 — granica kwoty brutto

Sol xhigh przejrzał pricing/cumulative allocation, cart snapshot i limity checkoutu; nie wykrył błędu arytmetyki. Dodano celowany test gross_overflow_is_rejected_even_when_net_and_vat_each_fit: kontrola VAT0 przy netto4_000_000_000gr działa; VAT23%=920_000_000gr również mieści się w u32, lecz brutto4_920_000_000gr przekracza typ i wycena zwraca błąd. Niezależna stała faktury odróżnia ten przypadek od dotychczasowego przepełnienia wcześniejszych pól. Bez zmian produkcyjnego algorytmu/cennika/UI.

`cargo test --offline --locked --manifest-path backend/Cargo.toml -p rapidmaker-pricing`: **7PASS/0FAIL/0SKIP**, rustfmt/diff PASS. Reviewer sprawdził rzeczywisty nowy test: potwierdza końcową konwersję brutto, bez blokera. To test granicy typu, nie cena dopuszczona w checkout (ma niższy limit), kalibracja kosztów firmy ani dowód rentowności.

## 2026-11-07 — jeden aktualny przebieg UI całego korpusu

Na czystym40e8a5d: corpus41-ui-authoritative-40e8a5d-20261107T004449Z — **41PASS/0FAIL/0SKIP/0flaky,676s,exit0**, bez rerun/poprawek/builda podczas testu. 36 przyjętych modeli rzeczywiście renderuje;4 wadliwe źródła14/21/32/37 poprawnie odrzucone,41 odmowa zasobów. Odmowy to poprawne wyniki scenariuszy, nie obsłużona geometria. Testy linked WebGL/pikseli/obrotu/zoomu PASS.

144 unikalne sceny:36×1440/390×jasny/ciemny. Worker obejrzał6 contact sheets, main1/6, Sol xhigh2–5; brak pustych podglądów lub przyciętych kontrolek w badanych scenach. Rozproszone zespoły są niewielkie przy dopasowaniu całości, szczególnie w ciemnym motywie; zoom działa. Nie jest to certyfikacja kontrastu/całej strony/fizycznego telefonu.

437 hashów przed/po identycznych (źródła,manifest,binarium,dist), HEAD bez zmian. ReportSHA16741edf62890ca6c011c1ab5e4ccfab8cb83a1af87fb04da05b41de225283ff, prywatne report/provenance/contact obok siebie w artifacts/. Izolowana DB/storage/profile large, normalny localhost/.env nietknięte, bez poczty/providerów/pobierania nowych modeli.

Reviewer zatwierdził aktualny zakres. Ten UI test sprawdza ceny przypadków9/20/29 oraz ręczną ścieżkę5/19/22/25; pozostałe podglądy nie dowodzą wyceny. Nie przypisujemy temu przebiegowi starszych24native/12manual z oddzielnego API. Wcześniejszy37PASS/4harnessFAIL+4PASS pozostaje historyczny; aktualny dowód UI jest jednym czystym41.

## 2026-11-07 — aktualne kontrakty frontendu

Na czystymc8d8514 `python frontend/tests/run-all.py --group contracts --no-build`: **54PASS/0FAIL/0SKIP/0flaky,12s,exit0**; raporty zachowane contracts-c8d8514/current-contracts.json i current-summary.json (poprzedni53PASS osobno). Testy obejmują parser GLB i limity, kwoty pozycji, formularze bezpieczeństwa/redirecty, CSRF w adapterach, panele, pending/retry zwrotów/nadań i operatora.

Sol xhigh przejrzał raport/listę/źródła: bez konkretnego błędu do poprawienia w tym zakresie. Są to testy produkcyjnego UI/parsera z wire fixtures i mockami API, wirtualny WebAuthn; nie Rust/DB, rzeczywiste operacje Stripe/Furgonetka/iCloud ani fizyczny klucz. Te integracje mają odrębne raporty. Brak zmian kodu i deklaracji10/10 całego produktu.
