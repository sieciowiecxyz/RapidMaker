# Katalog z Bambu Studio

Snapshot lokalnego `~/.config/BambuStudio/system/BBL`, wersja profili `02.08.00.06`.
Źródło upstream: https://github.com/bambulab/BambuStudio/tree/master/resources/profiles/BBL
Presety pochodzą z Bambu Studio; obowiązują warunki licencji źródłowego projektu.

- `catalog.json`: 49 materiałów Bambu, 314 wariantów kolorystycznych (także wielokolorowych), gęstości i powiązania z profilami P1P 0,4 mm.
- `profiles.json`: 134 oryginalne presety materiałów, drukarki i druku wraz z zależnościami `inherits` oraz `include`. Katalog zawiera SHA-256 tego pliku.
- P1P ma zapisaną osobno zamierzoną zmianę dyszy na hardened steel. Oryginalny preset pozostaje bez zmian; sam snapshot nie konfiguruje slicera.

Snapshot zasila już import PostgreSQL i katalog API; nowy slicer używa wybranych presetów. Frontend i stary checkout nie są jeszcze przełączone. Nie zawierają kont, tokenów ani konfiguracji użytkownika Bambu Studio.

## Czego te dane nie potwierdzają

- `slicer_default_cost` jest wartością z presetu, bez potwierdzonej waluty i VAT. Cena zakupu netto w PLN pozostaje pusta; administrator wpisze ją z faktury/cennika.
- Kolory oznaczają katalog slicera, nie aktualną ofertę sklepu ani stan magazynu. Wszystkie mają początkowo `in_stock: false`.
- 47 materiałów ma profil deklarujący P1P 0,4 mm. To nie kwalifikacja produkcyjna: trzeba sprawdzić m.in. obudowę, ekstruder i wymagania materiału. `production_enabled` pozostaje wyłączone.
- Przed użyciem CLI trzeba rozwiązać dziedziczenie i dołączenia presetów oraz zastosować konfigurację rzeczywistej drukarki.

## Podłączenie

Rust: `storage` importuje materiały i kolory przez SQLx; `slicer` dobiera zaufany profil P1P (oprócz materiałów podporowych), po osobnym zatwierdzeniu materiału przez operatora; `pricing` liczy koszt; `api` zbiera trasy HTTP, `app` uruchamia serwer. Chronione tokenem API operatora zapisuje cenę zakupu, kolory, magazyn i dopuszczenie do produkcji. Podłączenie panelu jest kolejnym krokiem.

## Cennik roboczy POC

`pricing-poc.json` jawnie zakłada EUR/kg netto dla liczb z `slicer_default_cost` (PLA/ABS 19,99, TPU95A 41,99). **Waluta i VAT presetów nie zostały potwierdzone**; to budżet POC, nie cena z faktury. Pozostałe stawki są jawnymi roboczymi kosztami obsługi/maszyny/energii i marżą.

Jednorazowo: `cargo build --locked --manifest-path backend/Cargo.toml -p rapidmaker-api --example configure_pricing_poc`, potem `python backend/run.py --pricing-poc`. Tylko AUTH_MODE=localhost, Stripe test i lokalny PostgreSQL:18434. Polecenie odmawia nadpisania istniejących cen/cennika. Aktualna baza jest już skonfigurowana.

Importer pobiera ostatni opublikowany kurs EUR z [NBP](https://api.nbp.pl/api/exchangerates/rates/a/eur/?format=json), zapisuje tabelę/datę i kurs ×10000 w `source.poc_pricing`. Przeliczenie całkowitoliczbowe: `ceil(eurocenty × kurs_x10000 / 10000)` → grosze PLN/kg. Na każdy upload/kalkulację nie ma nowego zapytania do NBP. Import katalogu zachowuje pochodzenie ceny. Błąd kursu nie zapisuje częściowej konfiguracji.

Użyty kurs: 23.10.2026, 207/A/NBP/2026, EUR=4,3917 PLN. Koszty robocze netto: PLA/ABS 87,80 PLN/kg, TPU 184,41 PLN/kg. Wersja cennika `poc-*` blokuje start backendu w AUTH_MODE=production. Przed sprzedażą trzeba potwierdzić walutę/cenę netto, warunki druku i podawania filamentu dla ABS/TPU i koszty operacyjne; potem zapisać zweryfikowany cennik przez admin API. Nie dodano samoczynnych odświeżeń ani scrapera sklepu.

Rozszerzenie istniejącego localhost POC: po zbudowaniu przykładu uruchom `python backend/run.py --pricing-poc-extend`. Dodaje brakujące koszty/dopuszczenia pięciu rodzin oraz serii PLA występujących w palecie (wartości `slicer_default_cost` w snapshotcie). Nie nadpisuje żadnej istniejącej ceny/dopuszczenia ani polityki cenowej; ponowienie niczego nie zmienia. Wymaga istniejącego cennika `poc-*`, lokalnej DB i Stripe test. To dopuszczenie do lokalnej symulacji wyceny, nie potwierdzenie rzeczywistych kosztów ani kwalifikacji drukarki do produkcji.
