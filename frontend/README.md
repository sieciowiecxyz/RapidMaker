# RapidMaker: klikalna propozycja

Svelte 5, TypeScript, Vite i Tailwind CSS. Kierunek cybercore z lokalną wyceniarką, kontaktem i mapą. Zakres: ocena stylu i przepływu wyceny; zakup i panele pozostają kolejnym etapem.

## Uruchomienie

```sh
npm ci
npm run dev
```

Podgląd: http://127.0.0.1:5173/.

```sh
npm run check
npm run build
```

## Dane i granice próbki

- `src/lib/api.ts` oddziela widoki od `mock.ts` i `types.ts`.
- Kwoty są fikcyjnymi wartościami demonstracyjnymi. Pliki nie są wysyłane. Wybrany STL jest odczytywany lokalnie w celu podglądu; cena nadal nie wynika z jego geometrii.
- Próbka obsługuje nazwy plików STL, STEP i 3MF, liczbę sztuk, materiał, kolor, jakość, trzy warianty wytrzymałości, odbiór/wysyłkę, ręczną wycenę, błąd i ponowienie.
- Implementuje lokalny podgląd binarnych i tekstowych STL. Nie implementuje podglądu STEP/3MF, zamówień, kont, płatności ani wysyłania zapytań. Przyciski email/telefon otwierają odpowiednią aplikację użytkownika.
- `PrintScene.svelte`, `partViewer.ts` i `partMesh.ts` tworzą lekki podgląd WebGL bez Three.js. Modele są proceduralne: PLA figurka szachowa, PETG zębatka, ASA obudowa, PA-CF ramię z trzema tulejami według referencji użytkownika, TPU mieszek harmonijkowy. Nie są to projekty dopuszczone do zastosowań konstrukcyjnych.
- GSAP steruje budowaniem, obrotem, zmianą koloru i ugięciem TPU. Shader pokazuje poglądowe warstwy zależne od jakości. Odświeżanie ma limit 30fps i DPR1.5; poza ekranem i w ukrytej karcie obrót zatrzymuje się. Reduced-motion daje statyczny podgląd. Jest przycisk obrotu, pauza i ponowienie.
- Materiały i jakość używają własnych etykietowanych selektorów opartych na natywnych grupach radio. Wytrzymałość zmienia przykładową cenę (Lekka ×0.9, Standard ×1, Wzmocniona ×1.25), bez deklarowania fizycznej nośności.
- `src/theme.css` jest wspólnym miejscem zmiany motywu. Interaktywne elementy wykorzystują wspólne komponenty w `src/components/ui`.

## Źródła

- Oferta, logo i kontakt: https://rapidmaker.pl/ oraz https://github.com/RapidMaker/RapidMaker.
- Triangulacja kształtów z otworami: https://github.com/mapbox/earcut.
- GSAP: https://github.com/greensock/GSAP.
- Zamiana ikon i łagodna zmiana wartości: https://github.com/Jakubantalik/transitions.dev, receptury09 i34. Użycie i modyfikacje są dozwolone w projektach osobistych i komercyjnych; nie jest to redystrybucja biblioteki.
- Zasady szczegółów interfejsu: https://github.com/jakubkrehel/make-interfaces-feel-better.
- Czcionki Manrope i Oxanium: lokalnie hostowane pakiety Fontsource. Ikony: Tabler.

Weryfikacja: Svelte bez błędów i ostrzeżeń, build Vite, przegląd desktop/mobile, przykładowe przeliczenia i ich nieaktualność, ręczna wycena, błąd/ponowienie oraz przejście fokusu do kontaktu. Pełna wydajność na fizycznych telefonach i czytnikach ekranu nie była mierzona.

Kolor Niestandardowy rozwija paletę 10 orientacyjnych odcieni. Wybór zmienia podgląd oraz nazwę i HEX w podsumowaniu kontaktu; nie deklaruje dostępności filamentu. PA-CF ma wyłącznie czarny kolor; klient wybiera go jawnie. Nowe pliki mają puste ustawienia. Każdy model ma własną konfigurację; kopiowanie działa z aktualnie wybranego modelu tylko do plików z niekompletnymi ustawieniami, zachowując gotowe konfiguracje i liczby sztuk.

Wizualizacja wytrzymałości: po zmianie wariantu model dzieli się jednym poziomym przecięciem. Górna połówka unosi się i zanika do 0% widoczności. Dolny przekrój pokazuje siatkę i obrys ścianek (Lekka: cienki, Standard: średni, Wzmocniona: gruby). Po jednorazowym pokazie części składają się i wracają do pełnej widoczności. Nie ma stałego przekroju ani przycisków skanera. Wypełnienie i ścianki są poglądowe; brak analizy STL i symulacji nośności.


## Na później: podgląd plików STL/STEP klienta

Zapisano 2026-10-20 na prośbę użytkownika. Aktualizacja tego samego dnia: użytkownik zlecił uruchomienie STL; poniższy plan pozostaje punktem odniesienia dla STEP i dalszych pomiarów. Szczegóły wdrożenia STL poniżej.

Cel: zastąpić proceduralne próbki rzeczywistym modelem wybranego pliku, zachowując szybką stronę i responsywny konfigurator.

- Importer ładować dopiero po dodaniu pliku. Początkowe wczytanie strony nie powinno czekać na obsługę formatów CAD.
- STL odczytać jako siatkę trójkątów. STEP wymaga importera CAD i triangulacji, np. OpenCascade; będzie osobnym, cięższym etapem. Wybór konwersji lokalnej lub serwerowej pozostaje do rozstrzygnięcia.
- Odczyt i przygotowanie siatki wykonywać w Web Workerze; przekazywać bufory bez zbędnego kopiowania. Formularz ma pozostać aktywny podczas przygotowania podglądu.
- Dla wielu plików wyświetlać jeden aktualnie wybrany model. Pozostałe przygotowywać na żądanie; cache ograniczyć budżetem pamięci i zwalniać zasoby GPU po usunięciu pliku.
- Dla ciężkich modeli przewidzieć uproszczoną siatkę do podglądu. Oryginał zachować do dalszej obsługi zlecenia. Limit MB nie zastępuje kontroli liczby trójkątów i pamięci.
- Zachować obrót, przybliżanie, zmianę koloru i poglądowy efekt warstw/budowania, po dopasowaniu skali i orientacji modelu.
- Przekrój, obrys ścianek i wypełnienie wymagają osobnego przygotowania oraz obsługi niezamkniętych/uszkodzonych siatek. Aktualny pokaz nie jest wynikiem slicera.
- Animację uginania TPU pozostawić na modelu pokazowym; dowolny plik klienta nie daje wiarygodnej informacji o elastyczności ani nośności.
- Zaplanować stany: przygotowywanie, gotowy podgląd, błąd formatu, zbyt ciężki model, ponowienie i anulowanie. Szybkie przełączanie plików nie może wyświetlić spóźnionego wyniku poprzedniego pliku.

Przed deklaracją szybkości wykonać pomiary na rzeczywistych STL/STEP: mały detal, gęsty skan, złożenie CAD i zestaw 10 plików, także na fizycznym telefonie. Zmierzyć czas do pierwszego podglądu, przełączanie modeli, płynność interakcji i zużycie pamięci. Nie ma jeszcze pomiarów ani gwarantowanych czasów.

Materiały do powrotu:
- STLLoader: https://threejs.org/docs/pages/STLLoader.html (referencja odczytu STL; nie oznacza decyzji o wdrożeniu Three.js).
- Import STEP/OpenCascade: https://github.com/kovacsv/occt-import-js.
- Web Workers: https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers.


## Lokalny podgląd STL — wdrożenie 2026-10-20

- `stlParser.ts` odczytuje binarny i tekstowy STL; oblicza normalne i granice, centruje model, przyjmuje Z jako pion i skaluje wyłącznie geometrię podglądu.
- `stl.worker.ts` wykonuje odczyt poza głównym wątkiem. Worker powstaje na żądanie, przenosi wynikowy bufor i kończy pracę; zmiana pliku przerywa poprzednie zadanie.
- Podgląd działa przed wyborem ustawień. Kolory są dostępne od razu; nowy plik nadal nie ma przypisanych ustawień. PA-CF ogranicza paletę do czarnego.
- Jeden aktywny model WebGL; cache siatek LRU z budżetem 48 MiB. Usunięcie pliku zwalnia jego cache. Import do 25 MiB i 500 tys. trójkątów; cięższy model dostaje komunikat zamiast blokować stronę.
- Obrót myszą obejmuje dwa kierunki, przyciski plus/minus sterują przybliżeniem. Działają zmiana koloru, poglądowe warstwy i animacja budowania. Wybór wytrzymałości uruchamia jednorazowy, poglądowy przekrój również na wgranym STL: górna część unosi się i zanika, widoczne są różne grubości ścianek i gęstości wypełnienia, a model wraca do całości. Maska przekroju jest przygotowywana w workerze i przechowywana z siatką. Pokaz nie jest wynikiem slicera ani analizą wytrzymałości.
- STEP/3MF można dodać do listy i skonfigurować, ale podgląd jasno informuje, że potrzebny jest STL. Plików nie wysyłamy na serwer.
- Sprawdzono parser na binarnym STL z nagłówkiem `solid`, ASCII z wykładnikami i błędnych plikach. Testy przeglądarki używają własnych plików kontrolnych. Brak jeszcze pomiarów na pliku użytkownika i fizycznym telefonie.
