# Katalog z Bambu Studio

Snapshot lokalnego `~/.config/BambuStudio/system/BBL`, wersja profili `02.08.00.06`.
Źródło upstream: https://github.com/bambulab/BambuStudio/tree/master/resources/profiles/BBL
Presety pochodzą z Bambu Studio; obowiązują warunki licencji źródłowego projektu.

- `catalog.json`: 49 materiałów Bambu, 314 wariantów kolorystycznych (także wielokolorowych), gęstości i powiązania z profilami P1P 0,4 mm.
- `profiles.json`: 134 oryginalne presety materiałów, drukarki i druku wraz z zależnościami `inherits` oraz `include`. Katalog zawiera SHA-256 tego pliku.
- P1P ma zapisaną osobno zamierzoną zmianę dyszy na hardened steel. Oryginalny preset pozostaje bez zmian; sam snapshot nie konfiguruje slicera.

To dane wejściowe przyszłego importu, jeszcze nie działający katalog API ani wyceniarka. Nie zawierają kont, tokenów ani konfiguracji użytkownika Bambu Studio.

## Czego te dane nie potwierdzają

- `slicer_default_cost` jest wartością z presetu, bez potwierdzonej waluty i VAT. Cena zakupu netto w PLN pozostaje pusta; administrator wpisze ją z faktury/cennika.
- Kolory oznaczają katalog slicera, nie aktualną ofertę sklepu ani stan magazynu. Wszystkie mają początkowo `in_stock: false`.
- 47 materiałów ma profil deklarujący P1P 0,4 mm. To nie kwalifikacja produkcyjna: trzeba sprawdzić m.in. obudowę, ekstruder i wymagania materiału. `production_enabled` pozostaje wyłączone.
- Przed użyciem CLI trzeba rozwiązać dziedziczenie i dołączenia presetów oraz zastosować konfigurację rzeczywistej drukarki.

## Dalsze podłączenie

Rust: `storage` importuje materiały i kolory do PostgreSQL przez SQLx; `slicer` uruchamia kontrolowane profile; `pricing` liczy koszt z czasu i zużycia; `app` zbiera trasy HTTP. Panel admina edytuje cenę zakupu, kolory, magazyn i dopuszczenie do produkcji. Zapisy wymagają uprawnień sprawdzanych przez backend.
