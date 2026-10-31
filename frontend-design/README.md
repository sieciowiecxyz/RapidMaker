# RapidMaker: zatwierdzony lokalny frontend

Pełny stan prototypu z `http://127.0.0.1:5173/`, zapisany 2026-10-31. Osobna aplikacja Svelte 5 / TypeScript / Vite / GSAP / WebGL. Aktualny wzorzec wyglądu i interakcji właściciela. Zintegrowana aplikacja `../frontend` i Rust API pozostają osobno; ten zapis ich nie zastępuje.

## Uruchomienie

Z katalogu `frontend-design`:

```sh
npm ci
npm run dev
```

Strona: http://127.0.0.1:5173/; panel demonstracyjny: http://127.0.0.1:5173/admin.html.

Sprawdzenie: `npm run check` i `npm run build`.

## Zatwierdzony wygląd i zachowanie

- Ciemny cybercore, limonka i koral, jasny motyw, szerokie kontury tła prowadzone do akcji i kontaktu. Ikony kontaktu pozostają limonkowe. Równe nagłówki i opisy usług.
- Kasety PLA / PETG / ASA / PA-CF / TPU. PA-CF wyłącznie czarny.
- Kolory Czarny / Biały / Dowolny / Niestandardowy. Dowolny pokazuje „10% taniej” i różowo-limonkowy gradient modelu. Opis: „Zostaw wybór koloru nam i zapłać 10% mniej za wydruk tego modelu.”
- Wykończenia dopiero po Niestandardowym. Mniejsze kafelki Basic / Matte / Silk i wyraźny przycisk z miniaturami dodatkowych wykończeń; jasny tekst, zwykłe ciemne tło i limonkowe zaznaczenie po wybraniu dodatkowej serii.
- Prostokątny wzornik z nazwami i wyszukiwaniem zastępuje koło. Statyczne dane Bambu Lab zależą od materiału i serii; nie są stanem magazynowym ani dopuszczeniem profilu.
- Mat, połysk Silk, nieregularne mineralne drobinki Marble, Sparkle i lekko przepuszczający światło Translucent. Wielobarwne Silk pokazują dostępne składowe koloru w gradiencie przestrzennym. Efekty są poglądowe, nie symulują procesu druku.
- Lokalny parser STL w workerze i natywny WebGL; osobne ustawienia dla każdego pliku, kopiowanie aktualnych ustawień tylko do niekompletnych modeli, podgląd warstw i animowany pojedynczy przekrój ścian/wypełnienia.
- Dysza zakrywająca stronę przy przejściu do formularza, dane zamówienia, kontakt i mapa. Przykłady 1 i 10 modeli. Panel demonstracyjny z etapami produkcji i finansami.

## Granice i integracja

`src/lib/api.ts` korzysta z lokalnego `mock.ts`. Kwoty demonstracyjne nie wynikają z geometrii. Rabat 10% działa per model w demonstracyjnej wycenie i nie obejmuje dostawy. Nie zastępuje to serwerowej reguły rabatowej: adapter i backend muszą wyliczyć autorytatywną kwotę przed płatnością.

STL jest przetwarzany lokalnie, bez uploadu. STEP/3MF nie mają tu lokalnego podglądu. Dane panelu są przykładowe; zapis jest lokalny. Formularz nie składa rzeczywistego zamówienia. Niestandardowy filament otwiera przygotowanie wiadomości e-mail do potwierdzenia; pliki trzeba samodzielnie dołączyć. Mail nie jest wysyłany automatycznie.

Integracja ma zachować te widoki i interakcje oraz rzeczywiste sesje, kontrolę plików, checkout, płatności, wiadomości i serwerowe kwoty aplikacji `../frontend`. Nie kopiować mocków do produkcyjnego adaptera ani podmieniać endpointów.

## Animowane światło tła

Zatwierdzono Glow Premium: jasny cienki rdzeń, miękka poświata i wygasające końce. Domyślnie działa Impuls; po dwóch jego przebiegach pojawia się jeden przebieg Fali, potem wraca Impuls. Oba mają długość Długa i prędkość Wolno. Przebieg trwa około 28 sekund aktywnego czasu; animacja zatrzymuje się w ukrytej karcie i przy ograniczonym ruchu. W jasnym motywie tło pozostaje ukryte.

Panel porównania jest schowany pod Narzędzia podglądu → Porównaj animacje tła. Zachowuje trzy wersje Smuga / Zgrubienie / Glow Premium oraz Impuls / Pas / Fala / Kometa, długość i prędkość. Zmiana wyboru restartuje animację i wyłącza pauzę. Zamknięcie porównania przywraca zaakceptowane ustawienia automatyczne. `FlowBackground.svelte` zachowuje geometrię linii; `FlowGlow.svelte` rysuje krótkie, zwężające się segmenty wzdłuż krzywych, do 30 fps, z usunięciem tickera przy odmontowaniu.

Weryfikacja zapisu: svelte-check i Vite build. Lokalne oględziny: wzornik, Silk Neon City, Marble, przycisk dodatkowych wykończeń i gradient Dowolny. Osobno sprawdzono rabat per model i brak rabatu na dostawę. Backend, produkcyjne płatności i pełne E2E nie są zakresem tego zapisu.
