# Produkcja i szkice faktur

Panel `/admin.html` korzysta z rzeczywistych zamówień. Link `/admin.html?order=UUID` otwiera szczegóły uprawnionemu administratorowi.

- Produkcja: przygotowanie → druk → kontrola jakości → pakowanie → gotowe do odbioru/nadania. Każdy model ma licznik wykonanych sztuk, ograniczony zamówioną ilością. Kontrola wymaga ukończenia wszystkich sztuk, pakowanie — zatwierdzenia kontroli. Cofnięcie do druku zeruje zatwierdzenie kontroli.
- Etapy wewnętrzne można zmieniać tylko dla opłaconego zamówienia w realizacji, przed nadaniem i bez konfliktu ze zwrotem. Nie zmieniają faktów płatności ani wysyłki. Termin i prywatne notatki pozostają oddzielnymi danymi operatora; klient ich nie otrzymuje.
- Rozliczenie zapisuje szkic nabywcy, adresu, NIP, daty i pozycji. Suma musi odpowiadać pełnej cenie zamówienia. To nie jest wystawiona faktura: bez numeru fiskalnego, PDF i KSeF. Status płatności pochodzi ze Stripe; nie można go przełączyć w panelu.

API administratora: `GET/PUT /api/admin/orders/{id}/production` oraz `GET/PUT /api/admin/orders/{id}/invoice-draft`. Zapis wymaga MFA i CSRF. `revision` chroni przed utratą równoczesnych zmian: konflikt zwraca 409, błędne ilości lub niespełniona kontrola — 422. Dane są trwałe w PostgreSQL; odczyt nie tworzy rekordów. Starsze zamówienia bez danych produkcji zachowują dotychczasowe przejścia statusów.

Sprawdzone: jednostkowe reguły ilości/QC/szkicu oraz test prawdziwego routera i PostgreSQL (`api/tests/access.rs`): role, CSRF, konkurencyjny zapis, obce modele, QC, niezmienność kwoty/płatności i brak prywatnych notatek w odpowiedzi klienta. Test routera używa lokalnych danych testowych, nie potwierdza operacji Stripe/Furgonetki. Pełne E2E i wygląd sprawdza główny agent podczas integracji.
