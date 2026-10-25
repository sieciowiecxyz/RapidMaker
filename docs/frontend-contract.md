# Kontrakt frontendu Piotra

Decyzja właściciela 2026-10-25: zachowujemy pracę wykonaną w lokalnym frontendzie; backend dopasowuje się do niej. Źródło odtworzenia: lokalny Rapidmaker/src (prototyp), admin_panel_PG b6629bf i późniejszy lokalny przycisk Cofnij etap. Baza integracji: main e5a7f28. Nie zmieniono uprawnień, płatności ani nadawania przesyłek.

## Działające wzorce

- `/`: przywrócone kasety PLA/PETG/ASA/PA-CF/TPU, orientacyjne kolory przed materiałem, czarny domyślny, PA-CF wyłącznie czarny, koło barw, pokaz 10 STL, lokalny podgląd STL, kopia ustawień tylko na niekompletne pliki, dysza i jasny motyw. Źródło: src/storefront. Wycena ręczna nie przedstawia fikcyjnej kwoty jako ceny sprzedaży.
- `/admin-design.html`: kompletny zatwierdzony panel z danymi przykładowymi, notatkami, terminami, postępem sztuk, etapem kontroli jakości, cofnięciem etapu oraz szkicami faktur. Dane demonstracyjne zapisują się lokalnie, pliki tylko w pamięci. To wzorzec do integracji, nie panel rzeczywistych zamówień.
- `/checkout-design.html`: pierwotna kompozycja danych zamówienia, formularz i podsumowanie; niczego nie wysyła. Wzorzec wizualny do podłączenia istniejącej ścieżki zakupu.

## Zachowane API i braki do dopasowania

`/admin.html` nadal używa sesji/MFA/CSRF i prawdziwego API. Checkout pozostaje przy dotychczasowych chronionych ścieżkach. Wzorce nie mają dostępu do prawdziwych zamówień. Brakujące kontrakty: wewnętrzne notatki, termin, postęp sztuk, wewnętrzne etapy produkcji i ich cofanie, szkic faktury oraz konfiguracja stałych materiałów/kolorów w uploadzie i ręcznej ofercie. Nie wolno usuwać tych kontrolek ze wzorca z powodu braków API ani omijać autoryzacji, stawek i ograniczeń płatności, by udawać działającą integrację.

Do migrowania nie należy przywracać przykładowych cen jako rzeczywistych. Błąd wysyłki/analizy serwera nie może blokować lokalnego STL. Po odświeżeniu lokalny File nie istnieje: trzeba wybrać go ponownie; same metadane w localStorage nie są plikiem.

## Paleta PLA

178 wpisów w 18 seriach, pobranych 2026-10-25 z oficjalnego `bambulab/BambuStudio/resources/profiles/BBL/filament/filaments_color_codes.json`. PLA Basic: 38 wpisów, w tym odcienie wielokolorowe. Zachowane są oficjalne nazwy, kody i składowe HEX; podgląd siatki używa pierwszej składowej. To przybliżenie ekranowe, nie kalibracja ani deklaracja dostępności/technologii.

## Weryfikacja

Wykonać svelte-check/build oraz oględziny desktop/mobile: czarny default, wszystkie serie palety, PA-CF czarny, realny lokalny STL, przykład 10 sztuk, przekrój i dysza. Backend/sandboxy wymagają osobnej konfiguracji; same wzorce nie dowodzą integracji.

Przywrócona strona `/` zawiera pierwotny formularz `#zamowienie` i lokalne podsumowanie; nie wysyła zamówienia. Istniejące zintegrowane trasy `/zamowienie` i `/konto/...` pozostają dostępne osobno. Podłączenie zatwierdzonego formularza do API wymaga adaptera, który przeniesie jego dane bez zmiany kompozycji. Pierwsze odtworzenie nie obejmuje tej integracji.
