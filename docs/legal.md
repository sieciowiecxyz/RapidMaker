# Regulamin i prywatność

Stan: wdrożone lokalnie. Autorski wzór dopasowany do funkcji, bez deklaracji pełnej zgodności prawnej.

## Ustalenia właściciela

Piotr Grudniak Rapid Maker, Zielony Krąg 1, 05-500 Piaseczno, NIP 1231537290, REGON 525918817, +48 537 320 816, piotr@rapidmaker.pl. Polska/PLN, konsumenci i firmy, tylko wydruki według specyfikacji klienta. Konto dobrowolne. Umowa sprzedaży po potwierdzonej płatności; wykonanie i nadanie/gotowość do odbioru do 14 dni kalendarzowych, przewóz osobno. Inny termin wymaga uzgodnienia przed zakupem. Usunięcie konta na wniosek email nie anuluje zamówień.

## Implementacja

- `/regulaminy`, `/regulamin`, `/prywatnosc`: publiczne strony i pobranie TXT. Linki w obu stopkach i przy rejestracji/płatności. Akceptacja przy przycisku, bez checkboxa; prywatność jest informacją, nie zgodą marketingową.
- Źródło: `backend/data/legal/2026-10-25/*.txt`; `notifications/legal.rs` współdzieli treść z API i emailami. `GET /api/legal/documents` bez sesji. Frontend renderuje tekst bez HTML. Nie edytuj opublikowanej wersji: dodaj nową i zachowaj obsługę starej dla zaległych emaili.
- `legal_version` wymagane przy rejestracji i rozpoczęciu płatności, także przez `/api/checkout/intent`, `/session` i ręczne oferty. Brak/stara wersja: 409 przed wysyłką kodu lub utworzeniem płatności. Wersja frontendu: `src/lib/legal.ts`.
- Migracja 0020: wersja/czas przy koncie i zamówieniu, termin produkcji przy zamówieniu. Akceptacja przy przygotowaniu płatności; umowa sprzedaży dopiero po zapłacie. Retry zachowuje pierwotny zapis. Starsze rekordy pozostają NULL, bez dopisywania rzekomej akceptacji.
- `legal_documents` w istniejącym outboxie: konto po potwierdzeniu emaila; sprzedaż po zapłacie, ze specyfikacją i pełnymi dokumentami. Transakcja i kopia są atomowe; stały event key zapobiega duplikatom przy retry. Preferencje marketingowe nie sterują tą korespondencją. Tylko ten typ ma limit payload/body 128 KiB; pozostałe limity zachowane.
- Google Maps to zwykły link; fonty lokalne. Import Stripe `pure` nie pobiera SDK przy samym otwarciu dokumentów/logowania; formularz adresu/płatności ładuje je na żądanie. Bez nowej analityki i marketingu.
- Ręczne zgłoszenie w panelu nadal wymaga konta; bez konta dostępny jest kontakt email. Nie dodano gościnnego API ręcznych ofert.

## Wniosek o usunięcie konta — ręcznie

Odnotuj wniosek i termin odpowiedzi, co do zasady miesiąc. Proporcjonalnie potwierdź tożsamość. Ustal aktywne zamówienia, rozliczenia i spory. Zamknij dostęp i sesje, usuń zbędne dane uwierzytelniania/profilu i odwołaj trasy odpowiedzi email. Nie usuwaj automatycznie dokumentów podatkowych, zamówień ani holdów niepewnych płatności. Ogranicz zachowane dane do właściwej podstawy i terminu; sprawdź również pocztę i kopie. Wyjaśnij klientowi, co pozostaje i dlaczego. To procedura operatora wymagająca indywidualnej kontroli DB, nie nowy przycisk lub automatyczny job.

## Przed publicznym uruchomieniem

- Przegląd prawny treści, bezpieczeństwa produktów (w tym GPSR), tolerancji/zastosowań oraz dokumentów sprzedaży. Personalizacja nie zwalnia z reklamacji i bezpieczeństwa produktu.
- Potwierdzenie możliwości dotrzymania 14 dni również dla dużych serii i wielu klientów. Nie ma harmonogramu rezerwującego moce. Inny termin trzeba uzgodnić przed płatnością.
- Audyt cookie i rzeczywistych żądań Stripe w checkout, w tym sygnałów antyfraudowych. Jeśli pojawi się funkcja opcjonalna wymagająca zgody, trzeba ją uzyskać przed uruchomieniem.
- Role, umowy i transfery Stripe/Furgonetka/Apple; szczególnie przydatność wybranego iCloud do poczty firmowej i powierzenia. Nie potwierdzono wymaganych umów ani podstaw wszystkich transferów.
- Konfiguracja rotacji backupów/logów i przeglądu wiadomości/dowodów. Polityka podaje kryteria, nie obiecuje nieistniejącego automatycznego czyszczenia wszystkiego. Zachowano istniejące 24 h/hold/90 dni dla modeli.
- Monitorowanie piotr@rapidmaker.pl, obsługa praw/reklamacji i sprawdzenie produkcyjnego doręczania. Obecnie sandboxy i testowa poczta do void@sieciowiec.xyz; dokumenty nie przełączają integracji na live.

## Podstawy

[UOKiK — informacje przed zakupem](https://prawakonsumenta.uokik.gov.pl/pytania-i-odpowiedzi/prawo-do-informacji/), [towary na zamówienie](https://prawakonsumenta.uokik.gov.pl/prawo-odstapienia-od-umowy/wylaczenia-prawa-do-odstapienia/), [reklamacje](https://prawakonsumenta.uokik.gov.pl/reklamacja/niezgodnosc/), [RODO](https://eur-lex.europa.eu/eli/reg/2016/679/oj?locale=PL), [usługi elektroniczne, art. 8](https://eli.gov.pl/api/acts/DU/2002/1204/text/U/D20021204Lj.pdf), [MF — przechowywanie dokumentów](https://podatki.gov.pl/poradniki-i-informatory/zakonczenie-dzialalnosci-gospodarczej). EROD 2/2025 o kontach odczytano w wersji konsultacyjnej, nie jako nową ustawę.
