# E2E — logowanie, checkout i granice

Mapa aktywnych działań, nie deklaracja PASS. Wykonane wyniki zbiera E2E-REPORT.md; pełne deklaracje kontrolek są w e2e-controls.md. Jedna grupa obejmuje kolejne wiersze/rekordy tej samej funkcji.

| Obszar | Akcje i wartości | Scenariusze |
| --- | --- | --- |
| Rejestracja | Mam konto / Utwórz konto, email, regulamin/prywatność, wysłanie i ponowienie kodu, ustawienie pierwszego hasła lub pierwszego passkey, ochrona ostatniej metody | registration (dwie osobne bazy/metody); legal |
| Logowanie | Hasło/passkey, email/TOTP, kod ratunkowy i powrót, błędne hasło/kody, zmień konto, bezpieczny next, role klient/admin | auth; security; legal; kontrakty auth-factors/live-panels |
| Ustawienia zabezpieczeń | Odśwież, hasło i powtórzenie, włącz/wyłącz TOTP, klucz/URI, potwierdzenie, ukrycie recovery, add/remove passkey, reauth, logout-all | security; registration-passkey; auth |
| Administracyjny bootstrap | Formularz po jawnym ?bootstrap=1 wymaga lokalnego grantu; nie jest zwykłą ścieżką klienta | kontrakt operations + istniejące testy backendu; bez nowego odzyskania prawdziwego admina |
| Dane zamówienia | Imię, email, telefon, adres Stripe dla kuriera; kod pocztowy dla paczkomatu; faktura on/off, firma/NIP, walidacja | checkout; shipping-checkout |
| Dostawa | Pickup/kurier/paczkomat; najtańsza i pozostałe dostępne oferty; wybierz/zmień punkt; szukaj w innym miejscu; retry | shipping-checkout; delivery; kontrakty operations dla utraty odpowiedzi |
| Podsumowanie | Parametry modelu, opakowania/paczki/zawartość, zmiana danych i powrót, etapy postępu, regulaminy | shipping-checkout; checkout; legal |
| Płatność | Formularz Stripe, zamawiam i płacę, odrzucona karta, utrata odpowiedzi i retry z tą samą próbą, reload wyniku, fałszywy powrót, wznowienie/wygaśnięcie | shipping-checkout; checkout; abandonment; boundaries |
| Ręczna wycena | Zgłoszenie z wymaganiami, kontakt i odbiór/faktura, admin cena/zakres/dostawa, email, oferta/pay | journey; panels; konfigurator; gość ma kontakt email bez wysyłania emaila z testu |
| Dokumenty | Hub, oba dokumenty, pobranie TXT, powrót, kontakt email, brak checkboxa i nieaktualna wersja | legal; registration |
| API/ochrona | Prawdziwe pakowanie 1000+7 sztuk; obce zasoby, role, CSRF/Origin, kwoty, prywatne GLB, usuwanie nieopłaconych plików, równoległość | boundaries; models; delivery; abandonment; panels |
| Email | Kod, ręczna oferta, pełne warunki, kopia rozmowy, odpowiedź email → chat dokładnie raz | auth; registration; journey; tylko void@sieciowiec.xyz |

Linków tel/mailto i zewnętrznej mapy nie uruchamiamy jako rozmów/wysyłek; sprawdzamy ich adres. Opcje wyłączone mają pozostać wyłączone z prawdziwym opisem. Nie generujemy rzeczywistych faktur ani nie deklarujemy dostępności niezaimplementowanych funkcji.
