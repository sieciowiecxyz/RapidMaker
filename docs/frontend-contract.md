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

## Aktualizacja lokalnego wzorca 2026-10-31: wykończenia i kolory

Pełny zatwierdzony prototyp z portu 5173 jest w [`frontend-design`](../frontend-design/README.md), wraz z konfiguracją i lockfile. Nie podmienia zintegrowanej aplikacji `frontend` ani API. Nowe decyzje zastępują poprzednie zasady koła i zielonego koloru w lokalnym wzorcu: po Niestandardowym wybiera się wykończenie, potem kolor z prostokątnego wzornika; koło zostało odrzucone. Trzecim kolorem jest Dowolny z etykietą „10% taniej” i koralowo-limonkowym gradientem.

Zatwierdzono poglądowe efekty Matte / Silk / Marble / Sparkle / Translucent i wielobarwne gradienty Silk. Przycisk dodatkowych wykończeń ma miniatury, opis liczby opcji, zwykły kolor pozostałych kafelków i trwałe limonkowe zaznaczenie po wyborze dodatkowej serii. Rabat wymaga serwerowej reguły przy integracji; prototyp liczy demonstracyjne ceny. Szczegóły i granice: README wzorca.

### Światło tła: finalny wybór 2026-10-31

W lokalnym wzorcu `frontend-design` pozostaje Glow Premium, Impuls, długość Długa, prędkość Wolno; co dwa przebiegi Impulsu włącza się jeden przebieg Fali z tymi samymi parametrami. Wcześniejsze Smuga/Zgrubienie i pozostałe warianty są w porównaniu dostępnym z Narzędzi podglądu. Każda zmiana wyboru restartuje animację. Bazowa geometria szerokich konturów pozostaje zatwierdzona. Ograniczony ruch, ukryta karta i jasny motyw nie uruchamiają ruchu tła.
