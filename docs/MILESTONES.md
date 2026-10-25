# Finalizacja — kolejność i kryteria

Stan: **NIEUKOŃCZONE — F2: automatyczna wycena w zwykłym formularzu localhost**. Poprzednie zamknięcie celu było błędne: testy z osobną bazą nie potwierdziły gotowości używanej strony. iCloud i journey ręcznej wyceny mają PASS. Kontrakt: [PLAN.md](PLAN.md). PASS oznacza rzeczywiste sprawdzenie; testy mock i sandbox opisujemy osobno.

## F0 — Stan i uruchomienie

- [x] Odczyt kodu, stylów i Git; osobne DB/storage i porty testów, ochrona cudzych zmian.
- [x] Wyjściowy Rust: 26219 LOC łącznie z testami; lokalny frontend/API dostępne.
- Przejście: znany stan, działający localhost, rozłączne zadania implementacyjne.

## F1 — Podgląd i wybór druku

- [x] STL/STEP/3MF → biały rzeczywisty podgląd bez koloru, prywatny GLB; klient/admin mają podgląd własnych plików zamówienia.
- [x] PLA/ABS/TPU jako przyciski; kolory bez warunku magazynu, również przy slicerze i checkout. Cena zakupu i dopuszczenie profilu nadal wymagane.
- [x] Wiele modeli, usuwanie, odtworzenie szkicu, ponowny upload i błędne pliki; bez zastępowania nieudanego uploadu przykładem.
- Przejście: rzeczywiste fixture trzech formatów, desktop/mobile, build i regresje PASS.

## F2 — Wycena i dostawa

- [ ] Zwykłe localhost:18430 i jego rzeczywista baza mają jawny cennik, koszt filamentu i dopuszczony profil; poprawny model w formularzu otrzymuje automatyczną cenę, bez seedowania wyłącznie tymczasowej bazy testowej.

- [x] Headless slicer 1/2/10/100/1000 sztuk, partie/płyty, jakość/ściany/wypełnienie; zmiana parametrów unieważnia cenę.
- [x] Grosze, VAT/minimum, jeden koszt przygotowania, energia/filament, granice liczb; wyłącznie jawne stawki.
- [x] >500 mm na osi → manual; rzeczywista P1P 256×256×250 i wycięcie stołu również obowiązują. Rotacja XY nie powiększa wysokości drukarki.
- [x] Serwerowe wymiary/masa, kilka pakowań, rzeczywista Furgonetka sandbox i ponowne użycie oferty; bez stałej ceny dostawy za dowolną ilość.
- Przejście: obliczenia zgodne z pomiarami/API; bez płacenia poprzedniej kwoty. Rentowność produkcyjna wymaga rzeczywistych kosztów operatora.

## F3 — Ręczna oferta i zapłata

- [x] Trwałe zgłoszenie i własne pliki, cena NULL do wyceny; admin ustala całość brutto, zawartą dostawę i zakres/termin wykonania.
- [x] Publikacja + outbox atomowo; link do chronionego zamówienia, Payment Element na naszej stronie. Rzeczywista płatność Stripe test potwierdzona.
- [x] Wersje, ważność 7 dni, ponowienia/idempotencja, własność i kwoty. Jedna próba na ręczne zamówienie; po starcie brak zmiany ceny, po zamknięciu nowa prośba.
- [x] Cały przebieg z rzeczywistym odbiorem kodu/oferty w iCloud i potwierdzoną płatnością; nie tylko SMTP accepted.
- Przejście: klient → admin → doręczony email → klient → paid, bez dubli zapłaty/outboxa.

## F4 — Retencja i bezpieczeństwo

- [x] Upload 24 h i rzeczywiste fizyczne usunięcie wszystkich plików. Po 24 h nieopłacona próba jest zamykana w Stripe przed zwolnieniem hold.
- [x] Manual 14 dni / oferta 7 dni; niepewne, processing i opłacone płatności zachowują ochronę. Wspólny model chronią wszystkie aktywne zamówienia; completed → 90 dni.
- [x] Negatywne modele/archiwa/ścieżki/zewnętrzne zasoby, limity parsera i quota, własność/role, CSRF, powtórzenia i granice kwot sprawdzone na własnym środowisku.
- Przejście: testy plików/DB, regresje Rust i audyt zależności PASS; nie oznacza certyfikacji bezpieczeństwa ani pentestu publicznego wdrożenia.

## F5 — Wygląd i spójność

- [x] Zachowany styl Piotra: fonty, limonka, motyw i tokeny; nowe oferty/płatność/podglądy w istniejących widokach. Poprawiony brak CSS w produkcyjnym konfiguratorze.
- [x] Usunięte widoczne teksty demo i pozorne pole załącznika; wspólne kontrolki, błędy/loading, krótsze numery zamówień (API nadal pełne UUID).
- [x] Obejrzane kluczowe screenshoty desktop/mobile: konfigurator, checkout, manual klient/admin i opcje zaawansowane; bez poziomego overflow. Regresje paneli/auth/operacji sprawdzone.
- Przejście: build, wizualny przegląd i właściwe regresje PASS; bez pobocznego redesignu.

## F6 — Rzeczywista całość i wielu klientów

- [x] Powtarzalny Playwright: [testing.md](testing.md#finalizacja-f0f6-rzeczywisty-backend), osobna DB/storage, testowi użytkownicy i stawki; bez sekretów w repo/trace.
- [x] Upload → podgląd → slicer → checkout → Stripe paid; ręczne zgłoszenie/admin/oferta → Stripe paid. Furgonetka wycenia, test nie nadaje przesyłek.
- [x] Rzeczywiste hasło+email, passkey+email i admin hasło+TOTP; uprawnienia dwóch kont i admina. Po przywróceniu doręczeń ponownie potwierdzone hasło+email w pełnym journey.
- [x] Cztery niezależne konwersje/klienci, responsywne HTTP i ograniczone workery; PostgreSQL lease/retry/fencing po utracie workera, brak pomieszania wyników.
- [x] Najnowsza oferta/chat → doręczony email → odpowiedź SMTP → ta sama rozmowa, dokładnie jedna odpowiedź i widoczność w UI.
- Przejście: cały rzeczywisty zestaw PASS. **Pełny journey PASS po rozwiązaniu blokady; wcześniejsze FAIL pozostają historią poniżej.**

## Checkpoint 2026-10-25

Plan F0–F6: commit 31c767b. Lokalne AGENTS.md i result.json są cudzą pracą i pozostają poza commitami celu. Nowa migracja 0019 i API działają także na normalnym localhost:18430/18432. Stawek fixture nie przeniesiono do normalnej bazy: PLA/ABS/TPU nie mają tam potwierdzonych kosztów i dopuszczenia; bez nich działają przez ręczną ocenę.

PASS: svelte-check/Vite build; Rust fmt/Clippy workspace all-targets, 107 testów (22 ignored osobno), cargo audit 0 podatności/ostrzeżeń. Dodatkowe 7 rzeczywistych PG: kolejka/lease, hold/retencja, chat/inbound, oba warianty limitów auth, claim/własność i quota. Worker sprawdził też PG katalogu/checkout stock=false/manual wersji i kwot oraz prawdziwy slicer 1–1000. 1000 szt. fixture: 40 płyt, 339720 s, 3629,2926 g — to pomiar testowy, nie oferta sprzedażowa.

PASS Playwright rzeczywisty: models+pricing 6 scenariuszy (8,7 s), auth 1 (28,2 s), dostawa sandbox 1 (2,8 s), nieopłacona sesja Stripe/physical cleanup 1 (2,5 s), automatyczna płatność na stronie 1 (10 s). Manual journey potwierdził formularze, admina, podgląd/własność i Stripe paid; cały test FAIL na emailach. UI kontrakty z mockami: 7/9 od razu; dwa po aktualizacji selektora numeru i usuniętego stock checkboxa PASS (2,2 s). Stary flows.spec.ts nie jest dowodem obecnej integracji.

Ostatnie wiadomości testowe odczytane w folderze iCloud: 24.10 23:57 UTC. Kolejne wysyłki SMTP (także osobny --mail-test) są przyjmowane, lecz odczyt IMAP nie znalazł nowych testów w żadnym folderze. Przyczyna doręczenia nieustalona; nie nazywamy tego potwierdzonym limitem iCloud. Wcześniejsze kody email i oferty rzeczywiście odebrane. TOTP pozwolił sprawdzić płatność niezależnie, ale test doręczenia pozostał FAIL. Nie usuwano wiadomości ani nie zmieniano reguł konta.

Rust 26219 → 27651 (+1432): backend/crates/**/*.rs, kod bez pustych/komentarzy/generowanych, łącznie z testami inline i przykładem fixture (bez wiarygodnego podziału production/test). Nowe moduły: manual_quotes API/storage, migracja i podgląd/oferta/płatność UI oraz izolowany runner. Brak nowych zależności produkcyjnych.

Kontynuacja: wykryto i poprawiono pomijany powrót z logowania do konkretnego zamówienia z emaila oraz `next=/zamowienie`. Allowlista obejmuje konkretne lokalne trasy i pełny UUID; klient nie może wracać do admina, adresy zewnętrzne są odrzucane. PASS build i 6 regresji Playwright UI (5 od razu, checkout po uzupełnieniu odpowiedzi fixture o config/katalog). Testowa odpowiedź SMTP dostaje poprawne Date i unikalny Message-ID; py_compile PASS, odbioru nadal nie deklarujemy. Odczyt iCloud nadal kończy się na UID 343; bez kolejnego wysyłania kodów. Rust i zależności bez zmian w tej poprawce.

Audyt blokady: trzeci kolejny obrót celu nadal potwierdza ten sam warunek — świeży odczyt IMAP kończy się na UID 343. Brak czynnego procesu testowego, na który należy czekać; zakończony journey pozostaje FAIL na doręczeniach. Po poprawce powrotu po logowaniu nie pozostała niezależna implementacja F0–F5 do wykonania. Nie ponawiamy tych samych wysyłek. Wznowienie: potwierdzić, gdzie trafiają nowe testy do void, lub przywrócić doręczenie/konfigurację odbioru; następnie uruchomić rzeczywisty journey i dopiero po PASS zamknąć F3/F6. Pytanie do użytkownika o odbiór nowych wiadomości pozostaje bez odpowiedzi.

## Zamknięcie F3/F6 — 2026-10-25

Na prośbę „sprawdź czy działa teraz” ponowiono `python frontend/tests/run-integration.py --sandbox journey.spec.ts` (bez TOTP obejścia emailowego wariantu fixture): **1 PASS, 2,4 min**. Prawdziwe hasło+kod z iCloud → upload/model manual → admin+TOTP → publikacja → email oferty → klient/podgląd/własność → Stripe test paid → wiadomość admina → kopia odebrana w iCloud → odpowiedź SMTP → dokładnie jeden wpis w tej samej rozmowie i widoczność na stronie. Świeży odczyt skrzynki wzrósł z UID 343 do 345 już na początku przebiegu. Baza i pliki testowe usunięte przez runner; bez rzeczywistych obciążeń/nadań i bez zmiany sekretów. Przyczyny wcześniejszej przerwy nie ustalono; obecny odbiór jest potwierdzony.

Audyt zamknięcia: jedyne otwarte kryteria F3/F6 dotyczyły doręczeń i email → chat; powyższy aktualny wynik je potwierdza. Pozostałe dowody F0–F5 (Rust/PG, modele/wycena, izolacja, retencja, sandboxy, wygląd i bezpieczny powrót po logowaniu) pozostają zapisane wyżej; nie powtarzano zielonych zestawów bez zmiany ich zakresu. To finalizacja lokalna/testowa: koszty i dopuszczenie profili normalnej bazy nadal ustala operator, publiczny deployment/live i kalibracja sprzedażowa są poza tym zakończeniem. Zmiany tej weryfikacji: tylko dokumentacja; Rust LOC i zależności bez zmian.

## Korekta błędnego zamknięcia — 2026-10-25

Po uwadze użytkownika odtworzono problem przez Chromium na zwykłym `http://localhost:18430/wycena`: cube_20mm.stl → PLA/kolor/standard/wytrzymałość → „Wymagana ręczna wycena”, bez ceny automatycznej. Odczyt normalnej DB: GFA00/GFB00/GFU01 mają cost_net_grosze_per_kg=NULL i production_enabled=false; pricing_config ma 0 rekordów, PRICING_RATES_JSON nie jest ustawione. Wcześniejsze PASS wyceny dotyczyły osobnej bazy z jawnymi stawkami fixture. To nie spełniało wymagania działającego formularza użytkownika. Poprzednie ogłoszenie osiągnięcia całego celu jest wycofane.

Do domknięcia: ustalić rzeczywiste koszty albo jawny lokalny cennik orientacyjny POC, zweryfikować/dopuścić wspierane profile i uruchomić wycenę przez normalny formularz. Nie wyłączać kontroli kwot/profili ani nie wpisywać fikcyjnych kosztów jako produkcyjnych. Pytanie o wariant konfiguracji wysłane użytkownikowi. Kod Rust i zależności w tej diagnozie bez zmian.
