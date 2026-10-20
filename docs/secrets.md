# Sekrety lokalne: SOPS + SSH

SOPS szyfruje pliki dla dwóch kluczy publicznych w `.sops.yaml`. Każdy właściciel może odszyfrować je osobno swoim kluczem prywatnym. Do Gita trafiają tylko zaszyfrowane `secrets/dev.enc.env`; jawny `.env` pozostaje ignorowany.

## Pierwsze uruchomienie

Zainstaluj `sops` i `age`. W terminalu ustaw ścieżkę do swojego klucza prywatnego SSH (bez `.pub`):

```sh
export SOPS_AGE_SSH_PRIVATE_KEY_FILE=/ścieżka/do/klucza
umask 077
sops decrypt secrets/dev.enc.env > .env
python backend/run.py
```

Klucz chroniony hasłem wymaga podania hasła w terminalu. Nie zapisuj hasła ani klucza prywatnego w repo. Jedyny plik `secrets/dev.enc.env` zawiera pełną konfigurację lokalną. Launcher czyta `.env` i przekazuje do API wyłącznie dozwolone zmienne dostawców.

## Zmiana wartości

```sh
sops edit secrets/dev.enc.env
```

Po edycji odszyfruj ponownie pliki lokalne i zrestartuj API.

## Dodanie osoby

Dodaj publiczny klucz SSH do listy `age` w `.sops.yaml`, oddzielając odbiorców przecinkami. Następnie osoba z obecnym dostępem wykonuje:

```sh
sops updatekeys -y secrets/dev.enc.env
```

Zacommituj konfigurację i zaszyfrowany plik. Usunięcie odbiorcy nie odbiera mu wcześniej poznanych sekretów; przy odebraniu dostępu zmień również tokeny u dostawców.

Sprawdzenie konfiguracji: plik zaszyfrowano dla obu odbiorców. Odszyfrowanie nie zostało sprawdzone — lokalny klucz prywatny wymaga hasła.
