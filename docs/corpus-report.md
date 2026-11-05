# Modele i E2E — checkpoint 2026-11-05

Cel nadal aktywny. Pobrano **41/100** różnych projektów MakerWorld: 34 × 3MF, 7 × STL; bez STEP z MakerWorld. Oryginały, warianty nazw i manifest SHA-256 pozostają lokalnie w ignorowanym `backend/test-models/makerworld/`. Licencje nie są w pełni zweryfikowane; nie publikujemy plików.

## Rzeczywisty korpus

`corpus41-indexed`: 41 przypadków, 195,2 s, bez pominięć.

| Wynik | Liczba | Co potwierdza |
| --- | ---: | --- |
| Automatyczna cena | 15 | Upload, GLB, prawdziwy slicer, dodatnie pomiary i całkowitoliczbowe koszty |
| Ręczna wycena | 6 | Podgląd działa; gabaryt przekracza politykę automatycznej wyceny/P1P |
| Za duży upload | 5 | Prawidłowe HTTP 413 powyżej 32 MiB |
| Limit geometrii | 15 | HTTP 422 `model_resource_limit`; pełna wycena nie została wykonana |

Runner zwrócił **26 PASS / 15 FAIL**, nie „41 działa”. Piętnaście testów nadal oczekuje automatycznej obsługi, mimo ochronnego odrzucenia. Pozostają jawnie oznaczone `resource-limit`; nie zmieniono asercji, żeby uzyskać zielony raport. Limity: 1 mln unikalnych wierzchołków i 1 mln trójkątów, ZIP 50 MiB łącznie / 25 MiB wpis / 256 wpisów, GLB 25 MiB. Zwykły frontend dodatkowo ogranicza plik do 25 MiB. Duże poprawne projekty MakerWorld mogą być nieobsługiwane.

Przed poprawkami było 19 PASS / 22 FAIL: 13 cen, 4 ręczne, 2 prawidłowe 413, 17 limitów, 3 błędy proxy oraz 2 niewyjaśnione odrzucenia parsera. Model 2 otrzymuje **wycenę ręczną**, nie automatyczną cenę.

## Poprawki i dowody

- Bambu `paint_*`/`face_property`: pomijane tylko jako ustawienia trójkątów w kopii geometrycznej. Wierzchołki, transformacje, oryginał i pełna walidacja pakietu pozostają. 21 testów geometrii PASS; model 28 przechodzi do ręcznej wyceny, model 36 po indeksowaniu uzyskuje podgląd i ręczną wycenę. Znaczenie pól sprawdzono w [źródle Bambu v02.08.02.61](https://github.com/bambulab/BambuStudio/blob/v02.08.02.61/src/libslic3r/Format/bbs_3mf.cpp).
- Dokładne indeksowanie wierzchołków STL/3MF: zachowane bity współrzędnych, kolejność i orientacja każdego trójkąta, bez uproszczenia siatki. ASCII kontroluje limity podczas czytania. Geometria23 PASS, Clippy PASS; realne modele3/6 dostają cenę,36 podgląd/ręczną wycenę. Budżety czasu/RAM, trójkątów, ZIP i GLB nie zostały zwiększone. Porównanie całego41: trzy poprawione przypadki, zero nowych FAIL ani zmian klasyfikacji wcześniejszych sukcesów. `indexed-models-ui.json`1 PASS2,7s: pięć wariantów STL/STEP/3MF, prywatne GLB i UI; obejrzany desktop zachowuje styl autora. HashMap ma koszt pamięci/CPU; nie jest to dowód działania każdego dużego modelu. Sol xhigh: bez blokera w tym zakresie, oceny8–9/10.
- Model 4 wcześniej przekraczał 60 s CPU w 6,45 s czasu rzeczywistego. Slicer ograniczony do dwóch dozwolonych CPU przeszedł dla PLA/PETG i przez API. Nie podniesiono limitów; polityka `cpu2` unieważnia stare snapshoty.
- Proxy Vite zwraca 413 zamiast 500 dla odtworzonego EPIPE przy zbyt dużych uploadach. Trzy regresje PASS. Obsługa nie obejmuje EPIPE bez deklarowanego Content-Length; zwykłe błędy proxy nie są maskowane.
- Poprawiono zakres CSS etykiet dostawy i fałszywe ostrzeżenie o braku analizy mimo oszacowanych paczek. Zachowano styl autora. Test wyboru kurierów kończy przewijanie przed normalnym kliknięciem, bez force/retry.

## Pozostałe sprawdzone przepływy

| Raport prywatny w `frontend/tests/artifacts/` | Wynik i zakres |
| --- | --- |
| `security-current.json` | 2 PASS: błędne dane, zmiana hasła, passkey/TOTP, sesje |
| `auth-email-current.json` | 1 PASS: prawdziwy iCloud do void, hasło/email, wirtualny passkey/email, admin TOTP i klient → admin 403 |
| `commerce-current.json` | 5 PASS: Stripe test, dostawa sandbox, panele/chat/pliki, częściowy/pełny/nadmierny zwrot, cennik |
| `checkout-final-current.json` | 3 PASS: dokładna kwota oferty/przycisku/płatności, odbiór/paczkomat/kurier; obejrzane podsumowania desktop/mobile |
| `manual-email-chat-current.json` | 1 PASS: ręczna wycena w UI, publikacja admina, prywatny podgląd, email iCloud, Stripe test, warunki zakupu i odpowiedź email → chat |
| `public-account-visual-current.json` | 5 PASS38,2s: 14 wariantów tras, jasny/ciemny,1440/390px,56 zrzutów; fonty, `<img>`, błędy zasobów, szerokość i motyw; prawdziwe lokalne MFA, puste panele |
| Test `assets.spec.ts` | 2 PASS: obrazki `<img>` strony głównej i oba motywy; głównie logo, nie wszystkie grafiki serwisu |

To izolowane bazy i jawne koszty testowe. Nie dowodzi to produkcyjnej rentowności, fizycznego passkey, wszystkich przewoźników ani nadania przesyłki. Raportów zawierających dane formularzy nie publikujemy.

## Rachunek, partie i konfigurator

- Rust pricing: **6 PASS**, w tym pięć stałych, ręcznie obliczonych przykładów kosztów, zaokrągleń, marży i minimum zamówienia.
- Prawdziwy slicer: partie 1/2/10/100/1000 PASS (2 ściany, 15% wypełnienia). Testy sprawdzają sumy płyt, czasu i masy, fizyczne ograniczenie pola stołu oraz masę pełnej kostki 20 mm przy 100% wypełnienia (około 9,96 g wobec 10,08 g wynikających z objętości i gęstości profilu). Artefakt prywatny: `slicer-quantity-physical.log`.
- `batch-pricing-oracle.json`: **1 PASS**, trzy wyceny API z jawnego fixture (PLA, standard, 3 ściany, 20% wypełnienia), niezależne kontrole każdej składowej rachunku:

| Sztuki | Płyty | Filament | Brutto |
| ---: | ---: | ---: | ---: |
| 1 | 1 | 4,476 g | 29,97 zł |
| 10 | 1 | 43,906 g | 40,18 zł |
| 1000 | 40 | 4385,116 g | 1608,21 zł |

Koszty: 100 zł netto/kg, 2 zł/h maszyny, 150 W i 1,20 zł/kWh, 5 zł/płytę, 10 zł przygotowania, 5% straty, 30% marży, minimum 20 zł netto, testowy VAT 23%. Potwierdzono powiązanie kosztu z pomiarami, jednorazowe przygotowanie i malejącą cenę jednostkową. Są to **stawki testowe**, nie weryfikacja rzeczywistych zakupów i rentowności.

`configurator-current.json`: **6 PASS** (130 s): nawigacja/motywy, pięć rodzin i 178 kolorów PLA, ustawienia, rzeczywiste wyceny, sterowanie WebGL (powiększenie potwierdzone zmianą obrazu), zaawansowane wartości i powrót, błędy połączenia oraz wybór/usuwanie STL/3MF. Awarię sieci wstrzyknięto celowo; wyceny pochodziły z rzeczywistego API. Kolory sprawdzono w UI/stanie, nie jako 178 osobnych wycen/pomiarów pigmentu. To nadal nie jest pełna wizualna kontrola wszystkich ekranów.

## Spójność motywu i widoków

Odtworzono błąd: jasny motyw wybrany na stronie głównej znikał po bezpośrednim wejściu na logowanie. Wspólny `lib/theme.ts` przywraca preferencję przy starcie aplikacji i panelu admina; oba przełączniki zapisują ten sam klucz. Brak dostępu do browser storage nie blokuje zmiany motywu. Bez zmian CSS, palety i fontów autora: Workspace używa Manrope, storefront/admin.html ma nagłówki Oxanium.

`public-visual.spec.ts`:5 PASS38,2s (56 zrzutów). Trasy publiczne: strona główna, wycena, logowanie/rejestracja, dokumenty/regulamin/prywatność. Konta: lista/ustawienia klienta oraz admin.html, starsza lista, produkcja, materiały i cennik. W obu motywach1440/390px: właściwy nagłówek, fonty i obrazy, brak błędów zasobów/poziomego overflow; admin przełącza motyw i zachowuje go po reloadzie. Obejrzano4 reprezentatywne zrzuty (login/materiały mobile, admin/konto desktop). Build frontend0 błędów/ostrzeżeń. Przegląd Sol xhigh: bez blokera; oceny8–9/10, kwoty nie dotyczą zmiany.

To puste panele fixture i kontrola `<img>`/fontów/HTTP zasobów; nie obejmuje wszystkich wypełnionych zamówień, dialogów, SVG ani klatek Stripe. Nie mierzono błysku motywu przed załadowaniem JS.

## Odtwarzanie korpusu

[Skrypt przygotowania](../frontend/tests/corpus-tool.md) działa offline na jawnie wskazanym katalogu. Zachowuje kolejność, hashe i nietypowe nazwy istniejących przypadków; dopisuje nowe pobrania, nie zmienia README ani wyników testów. `--check` jest tylko odczytem. Weryfikuje ścieżki, ograniczone odczyty ZIP, sygnatury oraz unikalność projektów/payloadów. Pięć syntetycznych regresji PASS; ponowny check wszystkich41 PASS. Nie zastępuje testów geometrii ani sprawdzenia licencji. Przegląd Sol xhigh: brak blokera; zakres ocen8–9/10, pieniądze/wygląd nie dotyczą skryptu.

## Tożsamość slicera

SHA-256 launchera i programu Bambu jest częścią zadania, klucza deduplikacji i snapshotu. Worker sprawdza tożsamość, checkout odrzuca brakujący/niezgodny hash. Zmiana plików wymaga restartu API/workerów; granice mechanizmu opisuje [kolejka](slicer-queue.md).

Świeży build PASS (pozostaje wcześniejsze ostrzeżenie wygenerowanego mostka C++). `runtime-fingerprint-current.json`: **1 PASS / 8,3 s** — świeży automatyczny checkout, brakujący/zmieniony hash → 409, przywrócony → 200, rzeczywiste partie 1/10/1000 i rachunek. Zmiany snapshotu wyłącznie w izolowanej bazie testowej. Test nie podmienia instalacji Bambu. Przegląd Sol xhigh: bez blokera w tym zakresie; współbieżność 7/10, dowody 8/10, zakres wizualny bez zmian 10/10, pozostałe kategorie 8–9/10. Nie oznacza to zakończenia całego celu.

## Otwarte

- Pozostałe 59 pobrań: MakerWorld wyświetla CAPTCHA (obecnie „Wybierz w tej kolejności”). Weryfikacja została przekazana użytkownikowi; nie obchodzimy jej.
- Świadoma polityka obsługi dużych modeli zamiast bezwarunkowego zwiększania limitów.
- Pełna tożsamość środowiska Bambu (biblioteki i zasoby poza dwoma plikami), rzeczywiste koszty zakupu/energii, wszystkie grafiki/ekrany, współbieżność i pozostałe E2E. Kontrola rachunku i ilości dla opisanych fixture jest wykonana.
- `cargo audit`: 0 podatności/ostrzeżeń. `npm audit`: trzy wpisy low pochodzące z jednego `deepmerge` w narzędziach deweloperskich; [brak opublikowanej poprawki](https://github.com/advisories/GHSA-gcqx-w48m-w2h4). Nie jest to dowód podatności przepływu uploadu.
- Przeglądy Sol xhigh pozostają poniżej 10/10. Cel nie jest zakończony.
