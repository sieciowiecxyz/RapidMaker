# E2E — wymagania i dowody

Przeczytaj gdy: oceniasz pokrycie celu lub wybierasz test po zmianie.
Opis: faktyczne przebiegi i granice; szczegóły w [raporcie](corpus-report.md).

Raporty JSON są prywatne w `frontend/tests/artifacts/`. Nazwy poniżej pomijają `.json`. To osobne przebiegi, nie jeden wspólny wynik. Aktualny spis źródeł: 447 kontrolek w 45 plikach, [mapa](e2e-controls.md). Powtarzalny szablon nie oznacza sprawdzenia każdej możliwej kombinacji.

| Wymaganie | Test i wykonany dowód | Granica |
| --- | --- | --- |
| Różne modele i nazwy | `corpus.spec`: corpus41-large-current, 41 scenariuszy; 24 ceny slicera, 12 ręcznych, 4 wadliwe, 1 limit; corpus12-manual-geometry sprawdza uzasadnienie ręcznej ścieżki | 36 podglądów; odmowa nie jest obsługą modelu. STEP dodatkowo w models |
| Podgląd, obrazy i Piotr UI | `corpus-ui`: corpus41-ui-current 37 PASS/4 FAIL harnessu, corpus-ui-four-fixed 4 PASS; 144 unikalne ujęcia obejrzane. `public-visual`, `populated-visual`, `showcase`, `configurator` w core-current | Osobne przebiegi; 1440/390 i motywy, nie fizyczny telefon ani formalny audyt kontrastu |
| Ceny, ilości, przygotowanie raz | `pricing` w core-current: rzeczywiste 1/10/1000 sztuk, partie, jednostki, zaokrąglenia, rachunek fixture, zmienione/obce quote ID; order-identity-pricing po poprawce identyfikatorów | Jawne stawki testowe, bez dowodu rzeczywistych kosztów firmy lub globalnego optimum układania |
| Jedna siatka, różne konfiguracje | `same-upload-configs-final` 1 PASS: dwie wyceny/ilości/kolory/parametry, przypisanie kwot, Stripe, oba panele/pliki/reload | Powielenie wiersza szkicu to jawny szew; brak przycisku duplikacji istniejącego uploadu |
| Wszystkie oferowane opcje | `configurator` w core-current: pięć rodzin, paleta, jakość/wytrzymałość, zaawansowane, viewer, nawigacja i kontrolowane awarie sieci | [Mapa opcji](e2e-configurator-inventory.md); wymagania operatora mają ręczną ścieżkę |
| Rejestracja i logowanie | auth-journey-current 2 PASS; registration-password-current i registration-passkey-current po 1 PASS; `security` w core-current: błędne dane, rotacja, recovery, hasło/passkey/TOTP | Wirtualny passkey Chromium, nie fizyczne urządzenie; testy email tylko void@sieciowiec.xyz |
| Zakup, adres i dostawa | sandbox-current 9 PASS: checkout/shipping-checkout/delivery; prawdziwe Stripe/Furgonetka, paczkomat/kurier/pickup, adres/faktura/walidacja | Sandbox, nie produkcyjne ceny lub fizyczna dostawa |
| Wznowienie płatności | payment-resume-provider-fixed 1 PASS: odrzucona karta, opuszczenie/powrót, ten sam PI/kwota, paid po reloadzie, obcy gość odrzucony; niezależny Stripe GET | Jeden rzeczywisty scenariusz odzyskania, nie każda metoda/redirect Stripe |
| Gość → dobrowolne konto | guest-claim-fixed-final 1 PASS: płatność, link konta, login/MFA, trwałe wyłączne przypisanie, pliki/historia, ponowienie i obce konto | Istniejące konto; rejestracje nowych kont sprawdzone osobno |
| Klient/admin, chat i produkcja | `panels` w sandbox-current; order-identity-manual-compat po poprawce tożsamości; oba panele również same-upload-configs-final | [Mapa paneli](e2e-panels-inventory.md); celowo wyłączone kontrolki nie są funkcjami biznesowymi |
| Ręczna oferta, powiadomienia i email → chat | `journey` w auth-journey-current: zgłoszenie, admin cena, rzeczywisty iCloud, płatność, refund i odpowiedź email widoczna raz w chat | Odbiór osobisty; część wiadomości admina wysłana API, nie każdy krok przez UI |
| Zwroty i zgodność kwot | stripe-provider-proof-current 1 PASS oraz order-identity-manual-compat: 2500 gr pobrane, zwroty 500/1500/500, dokładne ID/suma; role i nadmierny zwrot odrzucone w panels | Niezależne odczyty Stripe test; nie rozliczenie bankowe |
| Nadanie i podjazd | `logistics` w sandbox-current: nadanie, PDF, tracking, anulowanie/przekazanie, podjazd DHL | Dawny carrierConnectionError DPD nie oznacza awarii DHL ani gwarancji każdego przewoźnika |
| Prywatność, retencja, równoległość | models/concurrency/boundaries/security/abandonment w core/sandbox; identity-storage-current 22 PASS; duże usuwanie core-large-removal-current 1 PASS | Równoległość ograniczona testami, nie szeroki load test/pentest lub dowód braku wszelkich luk |
| Faktyczny localhost | Profil large, proxy 96 MiB; smoke-fixed modelu 8: 78,4 MB → prywatny GLB i widoczny canvas, DELETE 204; kopia przed migracjami | Ten smoke nie powtarza cen/płatności/email. Wspólny target, profil tylko w środowisku procesu; [szczegóły](plans/large-models.md) |

## Aktualność i pozostałe ograniczenia

Core sprawdzono na fe47828; późniejsze zmiany produktu to powiązanie pozycji z uploadem/wyceną (a304129) oraz wznowienie PI/motyw Stripe (700aa0c). Ich zakres ma osobne testy powyżej; nie przypisujemy całemu core nowego przebiegu. Raporty i snapshoty hashów zachowane. Nowsze zmiany testów/dokumentacji nie są nowym dowodem działania produktu.

Brak pojedynczego pełnego przebiegu po wszystkich zmianach, pełnej macierzy każdej kombinacji kontrolek, fizycznego telefonu/passkey, szerokiego load testu i potwierdzenia rentowności na kosztach firmy. Zespół modelu 41 przekracza przyjęty budżet; 4 źródła mają wady. Wyniki review dotyczą konkretnych etapów i mają jawne ograniczenia; nie stanowią 10/10 całego produktu.

Runner ma wspólny domyślny Chromium Playwright w preflight i testach; własny executable wymaga jawnej zmiennej. `browser-default-consistency` potwierdza jeden rzeczywisty przebieg Swagger po tej poprawce; compile/start przeglądarki i symulacja obecnego systemowego Chromium również PASS. Sam check runnera nie zastępuje pełnego zestawu.
