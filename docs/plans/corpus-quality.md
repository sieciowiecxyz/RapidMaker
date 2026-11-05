# Cel: rzeczywiste modele i wiarygodne E2E

## Wymagania

- Zaakceptowane41 różnych projektów MakerWorld, lokalnie/prywatnie z pochodzeniem i SHA. Użytkownik zakończył pobieranie; bez kolejnych CAPTCHA i poszukiwania100 plików.
- Rzeczywiste uploady STL/STEP/3MF, prywatny podgląd, slicer i sensowny rachunek kosztów; jawne ograniczenia, bez udawania ceny.
- Wszystkie renderowane obrazy i przykłady, desktop/mobile i oba motywy; klikalne przepływy z mapy kontrolek wymagają testów działania, nie tylko obecności. Konfigurator, płatność/refundy, dostawa, klient/admin, logowanie/MFA, chat i rzeczywiste emaile. Wygląd Piotra i istniejące interakcje pozostają źródłem prawdy.
- Minimalny kod Rust/proste API, bez zbędnych warstw; nie usuwać walidacji kwot, własności ani zasobów dla zielonego raportu.

## Etapy

| Etap | Kryterium | Stan |
| --- | --- | --- |
| C1 Korpus | 41 unikalnych źródeł/hashów, manifest i ignorowane pliki | Zrobione:34×3MF,7×STL; STEP z fixtures |
| C2 Modele i ceny | Prawdziwy pipeline, granice, ilości i niezależny rachunek; każdy wynik opisany | Częściowo:pełny34/7; później odzyskano24/39 w osobnych testach API/UI |
| C3 UI | Renderowane obrazy/przykłady, podglądy i klikalne przepływy, desktop/mobile, oba motywy; naprawy zgodne ze stylem | Wykonane zakresy w tabeli dowodów; bez twierdzenia wszystkich kombinacji/pikseli |
| C4 Konta i zakup | Realne MFA/ownership/email/Stripe/Furgonetka/refundy i wznowienia | Przepływy sprawdzone, wyniki oddzielone według źródła |
| C5 Zamknięcie | Raport + aktualny audyt całego celu w10 kategoriach, brak niepotwierdzonych wymagań | Otwarte; nie deklarować10/10 bez dowodów |

## Najnowsze dowody

Pełne wyjaśnienia, źródła commitów, komendy i ograniczenia: [raport](../corpus-report.md). JSON/obrazy są prywatne w `frontend/tests/artifacts/`; nie są publikowane w Git. Oddzielnych przebiegów nie łączymy w jeden pełny PASS.

| Dowód | Wynik i zakres |
| --- | --- |
| `contracts.json` — HEAD97c10dd | 21 PASS/5,7s; kontrolowane kontrakty UI/narzędzi, nie dowód dostawców |
| `core.json` — HEAD97c10dd | 31 PASS/237,4s; realne API/DB/slicer/MFA/konfigurator/panele/zasoby; dostawcy osobno |
| `corpus41-96.json` | 34 PASS/7 FAIL/419,5s:19 cen,10 ręcznych,5 oczekiwanych413,6 limitów422,1 błędny model422 |
| `expanded96-ui.json` | 7 PASS/194,8s:4 nowe podglądy i3 poprawnie pokazane odrzucenia; nie7 cen |
| `sandbox.json` — baza f60b381 | 9 PASS/168,0s bez provider-blocked; późniejsze poprawki mają osobne regresje |
| `selected-carrier-payment.json` | 2 PASS/35,3s; wybrany przewoźnik/kwota/intent/trwałe zamówienie |
| `concurrent-unpaid-payment.json` | 1 PASS/5,9s;4 równoległe POST→jedna próba i zamówienie, konflikty/retencja |
| `concurrent-refund-final.json` | 1 PASS/25,7s; konkurujące zwroty nie przekraczają zapłaconej kwoty |
| `paid-refunded-visual-final.json` | 1 PASS/37,6s;16 ujęć klient/admin/paid/refunded/motywy/rozmiary obejrzane, status zwrotu i oś naprawione |
| `refund-mail-journey.json` | 1 PASS/211,1s; iCloud MFA/oferta/płatność/dokumenty/zwrot500/odebrany email/odpowiedź do chatu |
| `server-receipt-recovery.json` | 1 PASS/22,1s; guest zakup/cache/reload/purge danych; brak szczegółów404 jako kontrolowana usterka |
| `registration-password/passkey.json`, `email.json` | Wcześniejsze realne rejestracje/email MFA i wirtualny WebAuthn; szczegóły w raporcie |

## Granice i decyzje

- Większe modele dopuszczono decyzją użytkownika: ZIP96 MiB łącznie/na wpis,512 wpisów. Nadal1 mln unikalnych wierzchołków; trójkąty1 048 574 wynikają z niezmienionego STL50 MiB, worker2 GiB/25s CPU/30s wall, GLB25 MiB, upload API32 MiB/UI25 MiB. Optymalizacja dev tylko geometry/lib3mf, bez wyłączania sprawdzeń.
- Pełny przebieg odrzucił14/19/20/24/39/40 zasobowo i32 przez metadane. Po nim24/39 odzyskano: API2 PASS60,5s (24 ręczna printer_fit,39 cena), UI2 PASS58,6s z ponad1M widocznych trójkątów, GLB i niezależną kontrolą zerowego pola. Pozostałe14/19/20/40 i32 nie są obsługiwane; pełne41 po tej ostatniej zmianie NOT RUN. Nie obchodzimy walidatora ani nie zgadujemy namespace. W obecnym API24 jest kontrolowanym422, nie historycznym niezidentyfikowanym exit137.
- Rachunek testowy100 PLN netto/kg dowodzi arytmetyki, nie produkcyjnej rentowności. Fingerprint slicera chroni launcher/ELF, nie wszystkie zasoby instalacji.
- Brak twierdzeń o fizycznym passkey/przewozie, wszystkich przewoźnikach, produkcyjnym wdrożeniu, exactly-once SMTP lub każdym pikselu. Niski niezałatany upstream `deepmerge` pozostaje opisany w raporcie.
- Usunięto345 martwych linii makiety i zbędne lokalne kopie prawdziwych zamówień. Szkic kontaktu celowo pozostaje w przypisanym `sessionStorage`.

- Rozwinięcie scen potwierdziło, że14/19/20/40 wymagają74–193 MB STL, nie ma nieużywanych trójkątów.32 pozostaje nieprawidłowy; mały preflight skrócił obserwowany scenariusz odrzucenia z22,8s do633ms (1 PASS1,3s). Geometry33 PASS/1 optionalignored; pełna walidacja/limity pozostają, Sol review bez blokera.

- Uproszczenie geometrii:494 linii mniej,16 pakietów usuniętych bez zmiany wersji; pozostają wejścioweSTL/STEP/3MF, GLB wyjściowy. Naprawiony gateUnsupported; geometry31 PASS/1 optionalignored, realAPI5 PASS4,2s, workspace/build/ClippyPASS, Sol8–9/N/A.

## Review i następne kroki

Po kroku: GPT-6-Sol xhigh,10 kategorii: poprawność, uprawnienia/bezpieczeństwo, kwoty, zasoby, błędy, współbieżność, dowody testowe, prostota, utrzymanie, zgodność wizualna/kontrakt. Naprawiać konkretne uwagi; nie wymuszać oceny10 ani utożsamiać oceny małej zmiany z oceną całego produktu.

1. Utrzymać aktualne dowody core/kontraktów; następne sprawdzenia kierować na konkretne zmiany lub wykazane luki, bez powtarzania zielonego zestawu bez powodu.
2. Rozwijać bezpieczną obsługę pozostałych modeli i uzupełniać wykazane luki interakcji/wyglądu; bez arbitralnego rozszerzania zakresu i dublowania testów.
3. Audyt zamknięcia odnieść do całego wymagania i rzeczywistych źródeł. Cel pozostaje otwarty przy nieudowodnionych wymaganiach lub ocenach poniżej żądanego poziomu.
