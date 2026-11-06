# Modele i E2E — checkpoint 2026-11-06

Przeczytaj gdy: oceniasz dowody celu lub wybierasz kolejne sprawdzenie.
Opis: bieżące wyniki, granice i decyzje; plan w [corpus-quality.md](plans/corpus-quality.md).

**Cel pozostaje otwarty.** Pełny korpus: 36 PASS/5 FAIL, w tym 20 zmierzonych cen, 11 ręcznych zgłoszeń i 5 oczekiwanych odrzuceń 413. Osobny profil dużych modeli albo pozostanie przy STL 50 MiB wymaga odpowiedzi użytkownika; odpowiedzi nie ma.

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

`MODEL_PROFILE=large` działa tylko przy jawnym `AUTH_MODE=localhost`; **nie zmieniono `.env` ani domyślnych limitów/Twojego serwera**. Backend96MiB input/512MiB ZIP/200MiB STL, GLB25MiB. Parser nadal2GiB/25CPU/30wall, Bambu large4GiB/120CPU/125wall na partię; standard pozostaje60CPU/65wall. Całe zadanie150s/lease180s i kwoty256MiB/8GiB niezmienione. Profil nie jest jeszcze obsłużony przez limity/renderowanie aktualnego frontendu. Pełne ograniczenia/komenda: [plan](plans/large-models.md#explicit-backend-profile-not-enabled-on-the-user-server).

Zwykły profil `large-profile-standard-regression.json`:11 PASS/21,7s,3celowe pominięcia dużego profilu przez dopasowanie dawnej nazwy pliku (nowa `large-profile.spec.ts` usuwa kolizję selektora). W tym ceny1/10/1000, multi-model, własność/retencja i nowy100Continue/niekompletny body→429, po rozłączeniu200. `large-profile-storage.json`:20 PASS/0skip na osobnych bazach, migracja0023 i kwoty. Geometry39PASS/1opcjonalnyignored; API29PASS/1DBignored, slicer15PASS/3opcjonalneignored; właściwe DB przypadki wykonano osobno powyżej. Nie sumujemy tych przebiegów w pełny korpus/core.

Pierwszy test ceny20 (`large-profile-quote.json`) FAIL:3próby slicera z kodem137 przy60CPU (~33,4s wall każda), przyczyna sygnału nieustalona, bez twierdzenia OOM. Po jawnym120CPU `large-profile-quote120.json`1PASS52,8s; finalny3PASS obejmuje tę poprawkę. Pierwsze3FAIL testu uploadu dotyczyły zbyt ścisłego oczekiwania ciągu cache-control; prawidłowe `no-store` pozostawiono, test wymaga tej dyrektywy. Surowe wyniki zachowane.

Review wykrył i poprawiono: dostęp do wcześniej zapisanych dużych plików po powrocie do standardu (bezwzględne limity odczytu96/200, nadal właściciel/admin i dokładna długość); legacy analiza teraz blokuje drugie body przed alokacją i ma60s deadline całego żądania. Test prywatnych odczytów sprawdza funkcję/duży plik/admin/niezgodność długości, bez faktycznego restartu procesu. Clippy geometry/API/storage/slicer all-targets z-Dwarnings, app build, Python syntax, Rustfmt/Prettier/diff-check PASS; znane ostrzeżenie generowanego CXX pozostaje. Brak zmian UI lub nowych zależności. Cały cel pozostaje otwarty.
