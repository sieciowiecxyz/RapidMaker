# Panel operatora RapidMaker

Uruchom `npm run dev` i otwórz `/admin.html`. Osobny punkt wejścia rozszerza prototyp frontend, bez zmiany witryny klienta. `npm run build` buduje oba punkty wejścia.

Próbka zawiera 8 fikcyjnych zamówień: listę i filtry, wyszukiwanie po numerze/kliencie/pliku, eksport CSV, szczegóły modeli, ceny, liczbę gotowych sztuk, osobny stan płatności, termin, etapy produkcji, notatki, historię i szkice odpowiedzi. Dane zapisywane są pod kluczem `rapidmaker-admin-demo-v1` w localStorage. Daty są stałym scenariuszem z 24 października 2026.

Brak logowania i połączenia z backendem. Brak rzeczywistych plików, podglądu 3D, wysyłania wiadomości, przyjmowania płatności i etykiet wysyłkowych. Nie używać do przechowywania prawdziwych danych klientów. Integracja z formularzem zamówień i zabezpieczonym API to kolejny etap.

## Płatności i faktury

Menu Płatności pokazuje wspólny z zamówieniami stan `paid`, filtr rozliczenia i sumy wartości modeli. Faktury pokazują dokumenty robocze powiązane z `orderId`. W szczegółach zamówienia zakładka Rozliczenie pozwala oznaczyć płatność i utworzyć/edytować szkic. Szkic kopiuje pozycje i sygnalizuje rozbieżność po zmianie zamówienia; aktualizacja jest jawna. Nie nadaje numeru faktury, nie nalicza podatków i nie wystawia dokumentu. Stan zapisuje się lokalnie pod `rapidmaker-admin-invoices-v1`.

Proponowane połączenie z backendem: osobne encje Order, Payment i Invoice, połączone przez orderId. Payment powinien zawierać amount/currency/providerId/status i historię potwierdzonych zdarzeń operatora; wielokrotne webhooki nie mogą dublować wpłat. Invoice zachowuje snapshot danych nabywcy i pozycji, identyfikator systemu fakturowania, numer dokumentu, status i chroniony link PDF. Wystawienie dokumentu nie oznacza automatycznie opłacenia zamówienia. Płatności częściowe, zwroty i korekty wymagają osobnych zapisów zamiast obecnego demonstracyjnego pola boolean. Autoryzacja administratora, dane w bazie oraz klucze integracji pozostają po stronie serwera. Wybór dostawców płatności i fakturowania jest jeszcze otwarty.
