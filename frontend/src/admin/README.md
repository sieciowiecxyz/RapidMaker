# Panel operatora

Otwórz `http://localhost:18430/admin.html`. Logowanie jest wspólne: kod email oraz passkey admina. Sesja i uprawnienia należą do serwera; panel nie odczytuje roli z localStorage i nie pokazuje fikcyjnych zamówień.

Działają lista/filtry/CSV, rzeczywiste kwoty i płatności, statusy `paid → in_production → ready`, odbiór osobisty `ready → completed`, chronione pliki i rozmowa z klientem. Zapis wymaga CSRF. Błąd sesji czyści widok. Przy odświeżeniu panel pobiera aktualny stan; rozmowa ma paginację i kursor przeczytania.

Zmiana ceny/terminu, liczba wykonanych sztuk, notatki wewnętrzne, nowe załączniki i faktury są niedostępne, bez lokalnego udawania zapisu. Zakładka **Dostawa** obsługuje potwierdzenie paczek, aktualny koszt i jawne zamówienie przewozu, PDF etykiet, przekazanie kurierowi, podjazdy i anulowanie. Zakładka **Rozliczenie** pokazuje zwroty i pozwala zlecić część lub pozostałą kwotę. Niepewna odpowiedź zachowuje identyfikator i dane także po przeładowaniu; bez automatycznych operacji u dostawcy. Materiały i cennik: linki w menu.

`npm run build` buduje obie strony. Test kontraktu HTTP: `PLAYWRIGHT_CHROMIUM_EXECUTABLE=/usr/bin/chromium npx playwright test operations.spec.ts live-panels.spec.ts operator.spec.ts`. Testy podstawiają odpowiedzi serwera i nie wysyłają maili, zwrotów ani przesyłek do dostawców.
