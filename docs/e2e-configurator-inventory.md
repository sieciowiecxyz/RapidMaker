# Konfigurator — mapa kliknięć

Stan: scenariusze zaprogramowane; wykonanie i wyniki zapisuje główny raport E2E. Poniższa lista nie jest deklaracją PASS. Główny inwentarz szablonów: [e2e-controls.md](e2e-controls.md).

Źródła: `frontend/src/storefront/Storefront.svelte`, `components/ProductionControls.svelte`, `components/PrintScene.svelte`, `pages/configure/{Configure,PrintChoices,AdvancedOptions}.svelte`. Testy: `frontend/tests/integration/configurator.spec.ts`.

| Obszar i działania | Warianty / warunek | Scenariusz |
|---|---|---|
| Marka, przejście do treści, Oferta/Wycena/Kontakt, powrót na górę | Kotwica i nawigacja; skip link klawiaturą | navigation |
| Motyw jasny/ciemny | Zmiana i zachowanie po reloadzie | navigation |
| Kontakt telefon/email, mapa | Sprawdzamy docelowy protokół i adres; nie wykonujemy połączenia ani wysyłki | navigation |
| Regulaminy i moje zamówienia | Przejście do rzeczywistej podstrony; gość trafia do logowania | navigation |
| Wybierz plik, przeciągnij/upuść | Rzeczywisty plik STL, dragover/dragleave/drop, prywatny podgląd | navigation; models.spec.ts |
| Błędny format, pusty plik, ponad 25 MiB, ponad 10 plików | Odrzucenie przed uploadem, czytelny komunikat | navigation |
| Przykład / wczytaj ponownie | Próbka oznaczona jako pokazowa; zastąpienie prawdziwych plików wymaga potwierdzenia; anulowanie zachowuje plik | navigation |
| Wybór części, następna część, usuń | Ustawienia przypisane do właściwego pliku, zmiana aktywnej części, reload | all material colors |
| Liczba sztuk | 0 i 1001 odrzucone; 1, 2, 1000 akceptowane w konfiguracji; nie uruchamiamy bez potrzeby slicera dla 1000 sztuk | all material colors; pricing.spec.ts |
| Uzupełnij ustawienia / uzupełnij dla wszystkich | Wybór brakującej części i kopiowanie bez zmiany ilości | all material colors |
| Materiały PLA/ABS/TPU | Każda rodzina; rzeczywista serwerowa wycena osobno dla każdej | all material colors |
| Kolory | Każdy kolor z rzeczywistego katalogu dla każdej rodziny, zaznaczenie i zapis; obecny snapshot 38 PLA + 16 ABS + 2 TPU, test nie zakłada stałej liczby | all material colors |
| Jakość | Standard, Dokładny, Roboczy; zapis każdej oferowanej wartości | all material colors; advanced controls |
| Wytrzymałość | Lekka, Standard, Wzmocniona | all material colors; advanced controls |
| Odbiór/wysyłka, nietypowe wymagania | Ręczna wycena jako odrębny stan; dalsze szczegóły dostawy w delivery.spec.ts | all material colors; delivery.spec.ts |
| Przejdź do dostawy / opcje zaawansowane | Nieaktywne przy niekompletnej konfiguracji, aktywne po wycenie, rzeczywista nawigacja | all material colors; advanced controls; checkout.spec.ts |
| Podgląd: zoom +/−, obróć, pauza/wznów, odtwórz, drag | Reakcja sterowania, restart wznawia animację, zmiana obrazu przy zoomie; mobile i reduced motion | viewer mouse controls |
| Ponów przeliczenie | Celowe chwilowe odłączenie sieci, potem rzeczywista poprawna wycena | real connection failures |
| Spróbuj ponownie przy prywatnym GLB | Rzeczywisty upload STEP; jednorazowe przerwanie żądania GLB, następnie ponowne pobranie z backendu | real connection failures |
| Nieudany upload | Nie pokazujemy próbki jako pliku klienta; usuń i ponownie dodaj | real connection failures |
| Zaawansowane: dodaj plik, próbka, lista części, usuń ostatnią | Prawdziwy upload 3MF, metadata API, odróżnienie próbki, aktywna część i stan pusty | advanced upload |
| Zaawansowane: wybór materiału i koloru | Każdy oferowany kolor, stan aria-pressed; świadomie ręczna wycena | advanced controls |
| Zaawansowane: jakość i konstrukcja | Wszystkie przyciski, własne ściany/wypełnienie | advanced controls |
| Ściany, wypełnienie, wzór | Wartości 8 / 100%, siatka i gyroid; błędne zakresy blokują dalszą wycenę, retry zachowuje ostatnią poprawną wartość | advanced controls |
| Jednostki STL | mm/cm/in i zachowanie cm po powrocie do konfiguratora i ponownym wejściu | advanced controls |
| Orientacja, podpory, wykończenie, pasowanie, termin | Wszystkie wartości selectów, zapis i ręczna obsługa zamiast fikcyjnej ceny | advanced controls |
| Inserty | Liczba 3, rozmiary M3/M4/M5, ujemna liczba odrzucona | advanced controls |
| Uwagi | Zapis i zachowanie po nawigacji | advanced controls |
| Wróć do konfiguratora / dalej: wysyłka | Zachowanie jednostek, Roboczej jakości, własnych ścian/wypełnienia, insertów i uwag | advanced controls |
| Rysunek PDF | Widoczny, ale świadomie disabled; nie udajemy działającego uploadu PDF | advanced controls |
| Ukryte narzędzia prototypu | `details[hidden]`: nie są aktywnymi kontrolkami klienta i nie są częścią wyklikiwanej ścieżki | poza aktywnym UI |

`models.spec.ts` osobno sprawdza STL/STEP/3MF (w tym ZIP64 i części 3MF), błędne siatki/archiwa, prywatność oraz retencję. Testy konfiguratora nie podmieniają odpowiedzi API ani cen; celowe awarie sieci są oznaczone jako fault injection. Koszyk roboczy i wybory zaawansowane są stanem lokalnym aplikacji; automatyczna wycena korzysta z prawdziwego slicera/API. Opcje wymagające operatora sprawdzamy jako ręczne, bez deklarowania ich automatycznej realizacji.

Wykryte i poprawione w tym zakresie: powrót z zaawansowanych gubił jednostki i zaznaczenie wysyłki oraz zamieniał Roboczą jakość na Standard; przycisk odtworzenia nie wznawiał wcześniej zatrzymanej animacji. Regresje znajdują się w odpowiednich scenariuszach, a wyniki wykonania wymagają potwierdzenia przez runner.
