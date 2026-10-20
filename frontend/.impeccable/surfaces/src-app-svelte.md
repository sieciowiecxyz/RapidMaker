---
version: 1
slug: "src-app-svelte"
primary_target: "src/App.svelte"
related_targets: []
---

# RapidMaker: próbka katalogu części
Mode: Persuade z osadzonym Operate. Zakres: jedna próbka wyglądu i wyceny, nie pełne ekrany paneli.

## Direction contract
THESIS: Wycena zaczyna się od części i jej zastosowania. Formularz stoi obok oferty, zamiast ginąć pod długą wizytówką.
OWN-WORLD: Użytkownik zmienił styl na cybercore. Grafit #0B100C, jasny tekst #EDF4E9, limonkowa zieleń #B5EE4B; Oxanium w nagłówkach, Manrope w formularzu; kontrolki 3px i panele 6px. Domyślnie ciemny motyw z jasnym wariantem do porównania; żadnego dekoracyjnego migania.
STORY: Użytkownik rozpoznaje druk części i krótkich serii, dodaje lokalny plik lub przykład, wybiera opcje, widzi oznaczoną przykładową cenę i przechodzi do kontaktu.
FIRST VIEWPORT: Na górze logo i trzy proste linki. Po lewej nagłówek do 96px, krótkie objaśnienie bez zdjęcia (użytkownik wyraźnie zażądał jego usunięcia), z generatywną neonową częścią w canvas, składaną z 31 warstw i reagującą na kursor; to grafika poglądowa, nie podgląd modelu. Po prawej otwarta karta formularza z modelem, materiałem, liczbą sztuk i wynikiem. Sygnatura interakcji: wynik od razu traci aktualność po zmianie opcji, a dopiero najnowsza odpowiedź lokalnego mocka go zastępuje. Ruch: GSAP orchestruje jednorazowe wejście i zapętloną, pauzowalną animację warstw; Transitions.dev daje łagodną zmianę ceny i zamianę ikon. Ograniczenie ruchu i inicjowanie klawiaturą wyłącza animacje interakcji. Pętla sceny jest celowo silniejsza na wyraźną prośbę użytkownika; 30fps, DPR do1.5, zatrzymanie poza ekranem, statyczny wariant reduced-motion.
FORM: Katalog części, trzecia pozycja listy kierunków; seed 86bd85fc; użytkownik wybrał assigned, code-first. Dwie pozostałe pierwsze propozycje: karta zlecenia i próbnik materiałów; pozostałe: raport kontroli, interfejs CAD, szuflada próbek, karta przebiegu produkcji.
FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

Próbka ma mapę z istniejącej strony, kontakt oraz jawnie pokazane stany komponentów. Cennik jest syntetyczny; upload nie wysyła plików. Żadne zamówienie nie jest składane. Strona produkcyjna i stare repozytorium pozostają punktem odniesienia.
