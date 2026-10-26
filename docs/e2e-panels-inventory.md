# E2E — panele klienta i operatora

Inwentaryzacja 2026-10-26. Źródła: `frontend/src/admin/{Admin,Finance,Refunds,Shipment}.svelte`, `pages/account/*.svelte`, współdzielone `OrderDetail`, `OrderChat`, `ManualOffer`. Powtarzalne przyciski dla kolejnych zamówień/kolorów są jedną grupą akcji, nie osobnym kontraktem. Nie oznacza to sprawdzenia wszystkich kombinacji danych.

Testy rzeczywiste: `frontend/tests/integration/panels.spec.ts`. Wymagają `--sandbox --client-totp`, osobnej bazy/pliku fixture oraz Stripe test. Przygotowanie ręcznego zgłoszenia używa rzeczywistego API po uploadzie UI; publikacja, płatność i obsługa odbywają się przez interfejs. To nie są testy z podstawionymi odpowiedziami. Wykonane wyniki i ograniczenia: [E2E-REPORT.md](E2E-REPORT.md).

## Mapa akcji

| Obszar | Klikalne elementy / wszystkie wartości grupy | Oczekiwanie / pokrycie |
|---|---|---|
| Klient — zamówienia | Nowy wydruk; Szczegóły; Wiadomości; lista dla własnego konta; retry przy błędzie | `panels`: własne zamówienie i szczegóły; nowe zamówienie / retry pozostają w innych scenariuszach |
| Klient — szczegóły | Wszystkie zamówienia; Parametry wydruku; podgląd; Pobierz oryginał; ręczna oferta → przygotowanie → zapłata; sprawdzanie płatności | `panels`: rzeczywisty STL/GLB/plik, parametry i Stripe paid; `journey`: pełne zgłoszenie formularzem i email |
| Klient — chat | Wiadomość; Wyślij; Oznacz jako przeczytane (gdy widoczne) | `panels`: klient → admin → klient przez UI, odczyt warunkowy; email/inbound osobno w `journey` |
| Klient — ustawienia | Dwa checkboxy: wiadomości, status/wysyłka; Zapisz; link usunięcia konta/prywatności | `panels`: wyłączenie obu, reload i odczyt zapisu, przywrócenie; link email |
| Zabezpieczenia konta | Hasło, passkeys, TOTP, recovery, logout-all | `security`, `registration`, `auth`; wyniki w raporcie |
| Admin — nawigacja | Zamówienia, Produkcja, Wiadomości, Płatności, Faktury; Materiały i ceny; Ustawienia wyceny; Konto/Bezpieczeństwo; strona sklepu; Wyloguj | `panels`: pięć głównych widoków, katalog/cennik, wylogowanie; linki konta/sklepu pokrywane mapą globalną |
| Admin — nagłówek | Odśwież; jasny/ciemny motyw | `panels`: odświeżenie i oba motywy |
| Admin — lista | Sprawdź kolejkę; Wszystkie / Wymaga reakcji / Nowe / W produkcji / Do wysłania; szukanie; wyczyszczenie frazy; wyczyszczenie filtrów; eksport CSV; dwa wejścia w szczegóły | `panels`: pięć filtrów, wyszukiwanie istniejące/puste, reset, CSV z pełnym UUID, wejście ikoną; równoważny link numeru i skrót kolejki w mapie globalnej |
| Admin — drawer | Modele i wycena / Pliki / Dostawa / Rozliczenie / Wiadomości / Historia; zamknięcie przyciskiem/Escape | `panels`: wszystkie zakładki, oba zamknięcia |
| Admin — status | Dropdown następnego etapu; Dalej w stopce; Dalej na karcie produkcji | `panels`: paid → in_production → ready → completed dla odbioru osobistego; stan potwierdza API klienta |
| Admin — pliki | Podgląd, Pobierz oryginał | `panels`: prawdziwy GLB i download STL |
| Admin — wiadomości | Wiersz rozmowy; textarea; Wyślij; Pokaż kolejne gdy kolejna strona istnieje | `panels`: wiersz, odczyt i wysyłka; fixture dodaje 52 historyczne wiadomości wyłącznie do własnego zamówienia w bazie rapidmaker_test_browser_*, następnie klik Pokaż kolejne pokazuje ostatnią wiadomość i dokładną liczbę bez dubli (bez fałszowania płatności i emaili) |
| Admin — płatności | Wszystkie / Nieopłacone / Opłacone; szukanie; wyczyszczenie; Szczegóły rozliczenia | `panels`: wszystkie filtry, pusty wynik/reset i otwarcie właściwego zamówienia |
| Admin — refund | Kwota; Zleć; anulowanie/potwierdzenie dialogu; Cała pozostała kwota; Odśwież; Ponów przy niepewności | `panels`: zbyt duża kwota bez operacji, 5 zł + pozostałe 20 zł w Stripe test, idempotentne powtórzenie API, brak środków po pełnym zwrocie; niepewność/anulowanie dialogu w testach kontraktowych `operations.spec.ts`, nie jako dowód providera |
| Katalog | Szukanie; Dodaj materiał; ID/nazwa/gęstość/cena; edycja; dopuszczenie; Zapisz materiał; Dodaj kolor; kod/nazwa/RGBA; Kolor; Zapisz kolor | `panels`: nowy materiał testowy, edycja i reload, obie wartości checkboxa bez dopuszczania niezaufanego profilu, nowy kolor i edycja; nie zmienia normalnej bazy |
| Cennik | Wersja; przygotowanie, obsługa płyty, drukarka, energia, moc, straty, marża, minimum, VAT; Zapisz | `panels`: wszystkie pola, odrzucenie marży 100%, poprawny zapis i reload + wartości w API |
| Dostawa — odbiór | Odśwież i informacja o odbiorze bez nadania | `panels`: rzeczywisty odczyt; brak fikcyjnej przesyłki |
| Dostawa — kurier | Dodaj/usuń paczkę, wymiary/masa, kontrola refundu, Zapisz, koszt, akceptacja, Zamów, zaznaczenie paczek, PDF, przekazanie, Podjazd → data/ponowny podjazd/propozycje/wybór/rezerwacja, Anuluj → potwierdzenie, ponowienie niepewnej operacji | `logistics.spec.ts`: osobny automatyczny checkout DHL, rzeczywista oferta i nadanie; scenariusze anulowania i przekazania. Wyniki oraz blokada DPD w raporcie. `operations.spec.ts` osobno sprawdza kontrakty niepewnych operacji; nie dowodzi nadania |

## Alternatywne trasy workspace

`panels` przechodzi również przez `/admin/zamowienia`: wszystkie dziewięć statusów plus wszystkie statusy, uwagę on/off, wyszukiwanie pustego/własnego numeru, reset pól i otwarcie szczegółów. `/admin/zamowienia/{id}`: sekcje, parametry, GLB/oryginał, disabled terminu/notatek, prawdziwy chat i powrót do listy. `/admin/produkcja`: rzeczywiste własne zamówienie widoczne jako Przygotowanie, potem W druku po zmianie w drugim panelu, poprawny następny przycisk i odnośnik Szczegóły. Alternatywny Zapisz status jest sprawdzany jako disabled bez zmiany; przejścia statusu wykonuje główny panel opisany wyżej.

## Celowo niedostępne elementy

`panels` sprawdza `disabled`, zamiast obchodzić zabezpieczenie: Zamów ponownie; Termin realizacji; Cena / szt.; Zmień plik; Wystaw fakturę. Pozostałe celowo niedostępne: postęp pojedynczych sztuk, notatki wewnętrzne i etykieta w starszym widoku szczegółów (logistyka działa w osobnym panelu). Nie wolno raportować ich jako działających funkcji biznesowych.

## Granice uprawnień

Klient nie może zlecić refundu admina (403); inne konto nie widzi zamówienia, plików ani rozmowy (404). Po wylogowaniu admin/klient nie czytają chronionych list (401). Refundy zachowują status produkcji; pełny zwrot nie pozwala zwrócić dodatkowego grosza (409). Dowód providera wymaga stanów `succeeded`, nie samego HTTP 202.

## Zmiana UI

Karty produkcji i rozmowy admina mają etykiety z pełnym UUID, aby zapewnić jednoznaczne dostępne nazwy przy skróconym numerze. Podgląd parametrów admina rozpoznaje także jakość `draft`, zamiast pokazywać „—”. Bez nowej zależności i bez zmiany uprawnień.

Wykryty przez prawdziwy test błąd: rozciągnięty link numeru zamówienia przechwytywał klik „Szczegóły rozliczenia” w Finance. Poprawka CSS podnosi niezależny przycisk ostatniej komórki ponad nakładkę wiersza; test nadal klika normalnie, bez force.
