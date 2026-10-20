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

## Automatyczne szyfrowanie przed commitem

Po sklonowaniu każdy uruchamia raz:

```sh
git config --local core.hooksPath .githooks
```

Edytuj lokalny `.env`. Hook `pre-commit` zaszyfruje zmiany do `secrets/dev.enc.env` i automatycznie doda plik do commita. Bez zmian nie szyfruje ponownie. Wymaga Python 3 i SOPS; używa kluczy publicznych, bez hasła do klucza prywatnego. Zrestartuj API po zmianie `.env`.

Hash stanu jest lokalnie w `.git/sops-env-state.json`, poza repozytorium. Błąd szyfrowania lub jawny `.env` w indeksie blokuje commit. Nowszy SOPS przy niezmienionym lokalnym `.env` również blokuje commit: odszyfruj aktualny plik do `.env`. Po pullu z cudzymi zmianami sekretów zrób to przed własną edycją. Pierwszy commit z lokalnym `.env` tworzy nowy szyfrogram.

Możesz też edytować przez `sops edit secrets/dev.enc.env`; następnie odszyfruj go do `.env`. Bez lokalnego `.env` hook pomija szyfrowanie. Globalne hooki commit-msg, post-checkout, post-commit, post-merge i pre-push nadal działają przez delegowanie. `--no-verify` omija hook; nie jest to zabezpieczenie serwerowe.

## Dodanie osoby

Dodaj publiczny klucz SSH do listy `age` w `.sops.yaml`, oddzielając odbiorców przecinkami. Następnie osoba z obecnym dostępem wykonuje:

```sh
sops updatekeys -y secrets/dev.enc.env
```

Zacommituj konfigurację i zaszyfrowany plik. Usunięcie odbiorcy nie odbiera mu wcześniej poznanych sekretów; przy odebraniu dostępu zmień również tokeny u dostawców.

Sprawdzenie konfiguracji: plik zaszyfrowano dla obu odbiorców. Odszyfrowanie nie zostało sprawdzone — lokalny klucz prywatny wymaga hasła.
