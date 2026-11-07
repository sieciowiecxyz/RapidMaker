# Kontrakt frontendu Piotra

Decyzja właściciela 2026-10-25: zachowujemy pracę wykonaną w lokalnym frontendzie; backend dopasowuje się do niej. Źródło odtworzenia: lokalny Rapidmaker/src (prototyp), admin_panel_PG b6629bf i późniejszy lokalny przycisk Cofnij etap. Baza odtworzenia: main e5a7f28; integracja obejmuje także main 0db0058 i zachowuje kontrole sesji/MFA/CSRF oraz serwerową autoryzację płatności i dostawy.

## Działające wzorce

- `/`: przywrócone kasety PLA/PETG/ASA/PA-CF/TPU, orientacyjne kolory przed materiałem, czarny domyślny, PA-CF wyłącznie czarny, koło barw, pokaz 10 STL, lokalny podgląd STL, kopia ustawień tylko na niekompletne pliki, dysza i jasny motyw. Źródło: src/storefront. Cena pochodzi z zatwierdzonego profilu i slicera; wybór w UI nie dopuszcza automatycznie materiału do produkcji.
- `/admin-design.html`: kompletny zatwierdzony panel z danymi przykładowymi, notatkami, terminami, postępem sztuk, etapem kontroli jakości, cofnięciem etapu oraz szkicami faktur. Dane demonstracyjne zapisują się lokalnie, pliki tylko w pamięci. To wzorzec do integracji, nie panel rzeczywistych zamówień.
- `/checkout-design.html`: pierwotna kompozycja danych zamówienia, formularz i podsumowanie; niczego nie wysyła. Wzorzec wizualny do podłączenia istniejącej ścieżki zakupu.

## Kontrakt integracji

Stałe materiały i paleta renderują się lokalnie. Katalog serwera służy rozpoznaniu rzeczywistego profilu/koloru i wycenie, nie budowaniu kontrolek. PLA z serii wybranej na kole zachowuje właściwy profil Bambu. Kolor z palety PLA wybrany dla innego materiału pozostaje jawną preferencją do potwierdzenia; nie podmieniamy materiału ani nie wybieramy najbliższego koloru. Slicer mimo to szacuje koszt wybranego materiału na referencyjnym kolorze: UI pokazuje cenę orientacyjną i powód potwierdzenia. Zamówienie zachowuje oryginalny odcień i przechodzi przez ręczną ofertę przed płatnością. PA-CF ma lokalny wybór czarny, bez udawania nieznanego kodu SKU Bambu.

Cena wymaga dopuszczenia profilu, jawnego kosztu i aktualnej polityki cenowej. Profile i koszty nie włączają się samoczynnie. Lokalny POC można jawnie rozszerzyć przez --pricing-poc-extend; nie nadpisuje istniejących cen ani polityki, nie działa w production. Brak tych danych oznacza ręczną ofertę, którą admin publikuje, a klient opłaca przez Stripe. Dane testowe dotyczą wyłącznie izolowanych baz.

Błąd API nie blokuje lokalnego podglądu STL; analiza/upload pozostają osobnym stanem. STEP i 3MF korzystają z prywatnego GLB po konwersji backendu. Próbka jest jawnie próbką, nie plikiem przesłanym przez klienta. Po odświeżeniu sam File nie istnieje; zachowany serwerowy identyfikator nadal wymaga prawidłowej sesji/własności.

Formularz inline zachowuje kompozycję autora i przekazuje dane do rzeczywistego checkoutu. Kontakt/faktura są wpisywane raz; adres Stripe i wybór kuriera/punktu mają jedno źródło. Kwota, opakowania, oferta i zapłata pochodzą z API. Wynik Stripe prowadzi do trwałego zamówienia.

Panel `/admin.html` korzysta z trwałych metadanych produkcji i szkiców faktur. Notatki są prywatne, liczba gotowych sztuk jest ograniczona zamówieniem, wersja chroni przed nadpisaniem zmian. Cofnięcie etapu wewnętrznego nie zmienia wpłaty, refundu ani przesyłki. Szkic nie otrzymuje numeru fiskalnego i nie jest wystawioną fakturą. `/admin-design.html` i `/checkout-design.html` pozostają oznaczonymi wzorcami, bez prawdziwych danych klienta.

## Paleta PLA

178 wpisów w 18 seriach, pobranych 2026-10-25 z oficjalnego `bambulab/BambuStudio/resources/profiles/BBL/filament/filaments_color_codes.json`. PLA Basic: 38 wpisów, w tym odcienie wielokolorowe. Zachowane są oficjalne nazwy, kody i składowe HEX; podgląd siatki używa pierwszej składowej. To przybliżenie ekranowe, nie kalibracja ani deklaracja dostępności/technologii.

## Weryfikacja

Wykonać svelte-check/build oraz oględziny desktop/mobile: czarny default, wszystkie serie palety, PA-CF czarny, realny lokalny STL, przykład 10 sztuk, przekrój i dysza. Backend/sandboxy wymagają osobnej konfiguracji; same wzorce nie dowodzą integracji.

Wyniki integracji i ograniczenia: [integration-piotr.md](integration-piotr.md). Trasa `/zamowienie` nadal obsługuje wyniki/wznowienia płatności, a konto klienta — trwałe zamówienia i rozmowy.


## Akceptacja wyglądu 2026-10-31

Trzeci akcent: koral #FF5C77 na słowie „pomysł”, wskaźniku podglądu i aktywnym głównym przycisku. Ikony telefonu, emaila i ich strzałki pozostają limonkowe. Jasny motyw ma ciemniejszy koral #BD2548 dla czytelności.

Tło: szerokie 35 konturów prowadzone przez nagłówek, podgląd, główną akcję i kontakt. Wybrany został wariant „Po” z łagodniejszym zakrętem przy wycenie, ale zakończeniem z wariantu „Przed” przy kontakcie. Nie dodawać dodatkowego punktu omijającego akapit „Ustalimy…”: próba została odrzucona. Linie dochodzą poza dolną krawędź strony. Wzór jest statyczny, dopasowuje się do układu przez ResizeObserver, nie uruchamia ciągłej animacji. Ukryty w jasnym motywie. Usunięty dopisek „Od pierwszego prototypu po serię gotową do wykorzystania.”

Implementacja: `src/storefront/components/FlowBackground.svelte`, `coral.css`, `print-background.css`. Źródło akceptacji: lokalny podgląd 127.0.0.1:5173. Zmiana dotyczy wyglądu, bez zastępowania nowszych adapterów/API z main.

Poprawka PR #5 (2026-11-04): SVG wypełnia wysokość treści CSS `height:100%`; pomiar tła nie powiększa mierzonej strony. Regresja `flow-background.spec.ts` sprawdza skrócenie strony po przejściu do formularza, powrót do konfiguratora, jasny motyw i szerokość na desktop/mobile. Wynik: 2/2 PASS (1440 i 390 px), svelte-check 0 błędów/ostrzeżeń i Vite build PASS. Ceny i uploady są w tym teście stubowane; nie potwierdza on płatności ani wyceny backendu.

## Nazwy w zamówieniach

Prywatne `/api/{account|admin}/orders/{id}/files` zwraca opcjonalne `material_name` i `color_name`. Nowe wyceny automatyczne i ręczne zapisują nazwy z katalogu w snapshotach; `requested_color` zachowuje dokładny opis klienta. Starsze rekordy korzystają z bieżących nazw katalogu, a przy ich braku UI pokazuje ID. Brak katalogu nie usuwa pliku z odpowiedzi. Nazwy służą wyłącznie do wyświetlania: surowe ID, ustawienia, ceny i fingerprint slicera pozostają bez zmian. Lista kolorów grupuje parę kod/nazwa; tekst nazwy nie trafia do CSS ani HTML.

Limit uploadu32/96MiB pochodzi z publicznego API profilu; błąd daje25MiB i możliwość ponowienia. Backend large pozostaje opt-in. Lokalny STL≤25MiB nadal działa bez API; większy STL przechodzi na prywatny GLB, dekodowany do2mln trójkątów/25MiB. Bez zmiany wyglądu/normalnych/światła. Większy bufor nie dowodzi wydajności fizycznego telefonu; usuwanie pliku z listy oczekuje na chronione DELETE;409/awaria zachowuje model z prawdziwym błędem. Spóźnione zaakceptowane uploady po usunięciu/zastąpieniu są sprzątane; zamknięta strona/nieznana odpowiedź nadal polega na retencji.

Tożsamość danych: checkout `print_lines.model_id` pozostaje lokalnym ID pozycji dla zgodności cen/koszyka/pakowania. W zamówieniu `lines.line_id` jest kluczem pozycji UI/produkcji, `lines.model_id` rzeczywistym UUID pliku, a `lines.quote_id` wiąże dokładną konfigurację z `order_files`. Różne kolory/jakości tego samego uploadu mają osobne line_id/quote_id. Starsze/ręczne rekordy zachowują fallback; nie zgaduj pliku przez kwotę albo kolejność.

Upload: wspólny adapter automatycznie ponawia tylko HTTP429 z `code=model_upload_busy` i dokładnym `Retry-After: 1`. Backend oznacza tak wyłącznie odmowę admission przed odczytem body lub zajęty flock katalogu przed utworzeniem UUID/pliku. Limit:8 prób, maks.30s na planowanie ponowień, odstępy1/2/4s + jitter≤250ms; rozpoczętego POST nie przerywamy (zaakceptowany model wymaga sprzątania). Limity magazynu/parser429, auth,5xx i niepewny błąd sieci nie są ponawiane. Usunięcie lokalnego wiersza zatrzymuje kolejne próby; spóźnione zaakceptowane odpowiedzi zachowują dotychczasowe sprzątanie.

Zmiana materiału zachowuje wybrany kolor/odcień (także wybrany przed materiałem); wyjątek PA-CF wymusza czarny. Odcień palety PLA na innym materiale pozostaje preferencją do ręcznego potwierdzenia, nie deklaracją dostępnego kodu katalogowego.

Zmiana sesji w innej karcie usuwa prywatny widok i wymaga świeżego/me; opóźniona odpowiedź poprzedniego konta nie może przywrócić danych. Sygnał między kartami to losowy marker, bez danych użytkownika; szkice przeglądarki pozostają.
