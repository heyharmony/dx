### Spec: Live streaming Asciinema + zdalna kontrola z przeglądarki (PoC → MVP)

#### Cel
- **Oglądanie na żywo** tego, co dzieje się w terminalu (proces/pty) oraz **możliwość przekazania sterowania** jednej osobie w danym momencie.
- Zachowanie zgodności z formatem Asciicast v2 (łatwe odtwarzanie/archiwizacja) i prosty model bezpieczeństwa (TLS + tokeny).

#### Zakres (MVP)
- Wyjście (out): live stream w formacie NDJSON zgodnym z Asciicast v2.
- Wejście (in): dedykowany WebSocket do przekazywania znaków do stdin procesu.
- Arbitraż kontroli: tylko jeden „kontrolujący” na raz (lease z timeoutem).
- Bezpieczeństwo: WSS (TLS), token Bearer/JWT, CORS/Origin, limity i audyt.

#### Architektura wysokiego poziomu
- Proces uruchamiany w **pty** (np. `bash`, komenda automatyczna lub zadany program):
  - stdout/stderr → serializacja do Asciicast v2 (NDJSON) → broadcast (WS) + append do pliku `.cast`.
  - stdin ← z serwerowego kanału kontroli (WS). Tylko właściciel „lease” może wysyłać.
- Dwa kanały serwera:
  - „Output WS” (broadcast wielu oglądającym) lub HTTP endpoint do pobierania `.cast` (po zakończeniu / w trakcie przyrostowo).
  - „Control WS” (pojedynczy nadawca inputu w danej chwili) z mechanizmem claim/release.

#### Format Asciicast v2 (NDJSON)
- Linia 1: nagłówek JSON m.in. `version`, `width`, `height`, `timestamp`, `env`.
- Dalsze linie: zdarzenia `[czas_od_startu, "o"|"i", "payload"]`.

```json
{"version":2,"width":80,"height":24,"timestamp":1710000000,"env":{"SHELL":"/bin/bash","TERM":"xterm-256color"}}
[0.001,"o","Welcome\n"]
[0.532,"i","ls -la\r"]
[1.100,"o","total 0\n"]
```

#### Interfejsy serwera
- HTTP upload pełnego nagrania (opcjonalne, do archiwizacji):
  - `POST /api/asciicasts` — body: NDJSON (pierwsza linia nagłówek, dalej wydarzenia).
  - Nagłówki: `Content-Type: application/x-ndjson` (dopuszczamy też `application/json`), opcjonalnie `Content-Encoding: gzip`.
  - Odpowiedź: `201 Created` + `{ id }`.

- Live streaming wyjścia (oglądanie):
  - `GET wss://.../stream/:sessionId` — WS tekstowy; serwer wysyła linie NDJSON (nagłówek → zdarzenia na bieżąco).
  - Alternatywnie: `GET /api/asciicasts/:id` (HTTP) zwracający rosnący `.cast` (chunked) — prostsze dla playerów, trudniejsze dla „live bez opóźnień”.

- Zdalne sterowanie (wejście):
  - `GET wss://.../control/:sessionId` — WS dwukierunkowy.
  - Klient po autoryzacji próbuje przejąć „lease” kontroli: `{ "type": "claim" }` → `{ "type": "lease", "granted": true|false, "owner": userId }`.
  - Wysyłanie inputu (gdy lease aktywny): `{ "type": "stdin", "data": "ls -la\n" }` (UTF-8, opcjonalnie per‑key: `data` to pojedyncze znaki, w tym kody sterujące).
  - Zakończenie: `{ "type": "release" }` lub timeout/rozłączenie.

#### Tryb bez wrappera (embedowany streamer w aplikacji)
- Cel: generujemy strumień Asciicast v2 bez uruchamiania `asciinema stream/record` — całkowicie wewnątrz aplikacji.

- Mechanika zapisu Asciicast v2:
  - Na starcie sesji tworzymy nagłówek JSON (linia 1) z `version=2`, `width`, `height`, `timestamp`, `env`.
  - Dla każdego zdarzenia wysyłamy linię NDJSON `[#sek_od_startu, "o"|"i", "payload"]`.
  - Zegar: czas względny liczony od `Instant::now()` (monotonic), serializowany jako `f64` sekund.

- Skąd bierzemy dane „o” (output):
  - Jeżeli uruchamiamy proces w pty (np. `bash`/komenda), czytamy bajty z pty stdout/stderr i 1:1 emitujemy jako zdarzenia `"o"` (łącznie z sekwencjami ANSI).
  - Jeżeli to nasz własny TUI (render w terminal), duplikujemy zapis do backendu terminalowego: każde `write()` kierujemy też do `AsciicastWriter` (mirror wyjścia).

- Skąd bierzemy dane „i” (input):
  - Zdarzenia klawiatury od UI/pty (kody klawiszy mapowane na bajty wejścia terminala, np. `\r`, `\u{1b}` itd.).
  - Uwaga na prywatność: `i` można włączyć/wyłączyć flagą (domyślnie wyłączone, jeśli istnieje ryzyko haseł/sekretów).

- Resize terminala:
  - MVP: rozmiar stały (z nagłówka). Jeśli rozmiar się zmieni, aktualizujemy metadane serwerowe i nowym widzom podajemy aktualny `width/height` (player Asciinema bazuje na nagłówku; dynamiczny resize można pominąć w MVP).

- Transport i trwałość:
  - Wyjście kierujemy jednocześnie: (1) do Output WS (live), (2) do pliku `.cast` (append) w storage.
  - Zamykanie: po końcu sesji domykamy writer, publikujemy identyfikator do `GET /api/asciicasts/:id`.

- Backpressure i niezawodność:
  - Bufory Bounded (np. `mpsc` 64–1024), w razie przepełnienia: drop najstarszych `o` (opcjonalnie) lub chwilowy backpressure na pty.
  - Zapisy plikowe asynchroniczne; w razie błędów I/O: sygnał do UI i przerwanie nagrywania, stream live może działać dalej.

- Interfejs w kodzie (proponowany szkic):
  - `AsciicastWriter::new(width, height, env) -> Writer` (zapisuje nagłówek).
  - `writer.emit_o(&[u8])`, `writer.emit_i(&[u8])` (dodaje linie NDJSON z czasem).
  - `writer.split_to_ws(tx)` (forward do Output WS) oraz `writer.split_to_file(path)` (append do `.cast`).

- Ograniczenia i kompromisy:
  - Player Asciinema oczekuje „surowych” bajtów terminala (ANSI). Własny TUI musi emitować dokładnie to, co normalnie maluje na terminal (mirror write), inaczej odtworzenie może się różnić.
  - Dynamiczny resize i nietypowe sekwencje można pominąć w MVP; dopracować później.
  - Zdarzenia `i` wyłączyć domyślnie, włączyć per‑sesja (ochrona sekretów).

Korzyści: mniej zależności, pełna kontrola nad protokołem, łatwiejsze spięcie z kanałem sterowania i telemetrią.

#### Arbitraż kontroli (lease)
- Tylko jeden aktywny właściciel per `sessionId`.
- Lease ma **timeout** (np. 30–120 s) i wymaga **heartbeat** (np. `{ "type": "hb" }` co 10 s).
- Zasady przyznawania: „pierwszy wygrywa”, kolejka lub rola „host” może nadawać/odbierać.
- Serwer broadcastuje stan do widzów: `{ "type": "control_state", "owner": userId|null }`.

#### Bezpieczeństwo („state of the art”)
- Transport: **TLS** (WSS/HTTPS, TLS 1.2+), HSTS; prod: terminacja w reverse proxy lub natywnie `rustls`.
- Uwierzytelnianie: krótkie **Bearer/JWT** per sesja. W przeglądarce: nagłówek `Authorization` lub cookie `Secure`+`HttpOnly` (+CSRF jeżeli cookie).
- Autoryzacja: uprawnienia do claim/release; tryb read‑only dla widzów.
- Twarde limity: rozmiar i tempo wiadomości (rate limit), idle timeout, maks. czas sesji.
- Ochrona WS: walidacja `Origin`, restrykcyjny CORS, sanity‑check payloadów.
- Audyt: logi zdarzeń stdin, przyznania/odebrania kontroli, identyfikacja użytkownika.
- Opcjonalnie (wyższa poufność): **mTLS** dla operatorów/CLI, rotacja kluczy, szyfrowane storage.

#### UX/Frontend
- Oglądanie: player Asciinema (wymaga kompletnego pliku) lub własny viewer „na żywo” (parsing NDJSON z Output WS i odtwarzanie czasów). 
- Sterowanie: przycisk „Poproś o kontrolę” → claim; widoczny status właściciela; tryb per‑line (po Enter) lub per‑key (mniejsze opóźnienia, większy wolumen).

#### Telemetria/monitoring
- Metryki: czas do pierwszej ramki, liczba widzów, błędy WS, kolejka claimów, czas posiadania kontroli.
- Logi strukturalne (JSON), ślady (tracing) z korelacją `sessionId`.

#### Edge cases
- Równoczesne claimy, utrata połączenia właściciela, flood inputu, bardzo długie linie, znaki sterujące.
- Zmiana rozmiaru terminala (emituj zdarzenie resize w nagłówku/dodatkowej metadanej).

#### MVP — kryteria akceptacji
- Uruchamiamy proces w pty; Output WS wysyła nagłówek + zdarzenia Asciicast v2 w czasie rzeczywistym.
- Control WS: jeden klient może przejąć kontrolę, wysłać `stdin`, inni oglądają.
- TLS w terminacji (reverse proxy) + token Bearer w nagłówku.
- Zapis `.cast` po zakończeniu sesji; `GET /api/asciicasts/:id` zwraca plik.

#### Rozszerzenia później
- Nagrywanie fragmentów/znaczniki, udostępnianie z odtworzeniem „jak live”.
- mTLS/role RBAC, integracje SSO.
- Transkodowanie do playera web z buforowaniem HLS.

#### Otwarte pytania
- Czy input ma być per‑key czy per‑line (latencja vs. koszty)?
- Jaki maks. czas lease i polityka jego odnowienia?
- Czy dopuszczamy więcej niż jednego „hosta” z prawem force‑revoke?


