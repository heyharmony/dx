#!/usr/bin/env bash
set -euo pipefail

DEMO_DIR="./demos/bolt.new"
DX_BIN_REAL="${DX_BIN:-}"
if [ -z "$DX_BIN_REAL" ]; then
  if command -v dx >/dev/null 2>&1; then DX_BIN_REAL="$(command -v dx)"; elif [ -x ./target/release/dx ]; then DX_BIN_REAL="./target/release/dx"; else echo "Build dx first: cargo build --release" >&2; exit 1; fi
fi

mkdir -p "$DEMO_DIR"

[ -f "$DEMO_DIR/MOTD.md" ] || cat > "$DEMO_DIR/MOTD.md" << 'EOF'
<!-- dx:ascii -->
 ____        _ _     .__          __       
| __ )  ___| | | ___|  |   _____/  |_  ____
|  _ \ / _ \ | |/ _ \  | _/ __ \   __\/ __ \
| |_) |  __/ | |  __/  |_\  ___/|  | \  ___/
|____/ \___|_|_|\___|____/\___  >__|  \___  >
                              \/          \/

Welcome to bolt.new demo
EOF

[ -f "$DEMO_DIR/README.md" ] || cat > "$DEMO_DIR/README.md" << 'EOF'
# Bolt.new demo — agent w przeglądarce, realny scenariusz

Bolt.new (StackBlitz) to agent kodujący działający w środowisku przeglądarkowym. To demo pokazuje „wewnętrzne narzędzie admina” budowane od promptu do działania, bez lokalnej instalacji.

Co demonstrujemy:
- Web‑IDE/agent: generacja i uruchomienie aplikacji w środowisku przeglądarkowym
- Typowy internal tool: CRUD + filtrowanie + role + eksport CSV
- Szybka iteracja: zmiana promptu → aktualizacja funkcji
- Stabilny runtime: logi i statusy bez „magii” w tle

Pliki pomocnicze:
- `SCENARIO.md` — skrypt prezentacji i mówione CTA
- `PROMPT.md` — prompt budujący narzędzie „Admin Portal”
- `menu.toml` — sekwencja kroków w `dx`
EOF

[ -f "$DEMO_DIR/menu.toml" ] || cat > "$DEMO_DIR/menu.toml" << 'EOF'
[[items]]
name = "SCENARIUSZ prezentacji"
description = "Flow krok po kroku"
file = "SCENARIO.md"

[[items]]
name = "README"
description = "Cel i wartości demo"
file = "README.md"

[[items]]
name = "Prompt → Admin Portal"
description = "Brief funkcjonalny"
file = "PROMPT.md"

[[items]]
name = "1) Inicjalizacja projektu"
description = "Analiza promptu i setup środowiska"
cmd = 'for s in "Analiza promptu" "Tworzenie projektu" "Instalacja zależności"; do printf "%s... " "$s"; sleep 0.4; echo OK; done'

[[items]]
name = "2) CRUD + role"
description = "Użytkownicy, role, filtrowanie, paginacja"
cmd = 'for s in "CRUD Users" "Role: admin/user" "Filtrowanie + paginacja"; do printf "%s... " "$s"; sleep 0.5; echo OK; done'

[[items]]
name = "3) Eksport CSV"
description = "Generowanie i pobieranie"
cmd = 'echo "Generowanie CSV…"; sleep 0.8; echo "users_2025-09-15.csv (demo)"'

[[items]]
name = "4) Podgląd w przeglądarce"
description = "URL do instancji w WebContainers"
cmd = 'echo "Start serwera…"; for i in $(seq 1 5); do echo "[web] log $i"; sleep 0.4; done; echo "URL: https://bolt.new/preview/admin-portal-xyz (demo)"'
EOF

[ -f "$DEMO_DIR/SCENARIO.md" ] || cat > "$DEMO_DIR/SCENARIO.md" << 'EOF'
## Skrypt prezentera (Bolt.new)

Cel: pokazać jak agent w przeglądarce tworzy „Admin Portal” do zarządzania użytkownikami i rolami, łącznie z eksportem danych.

Kroki w menu:
1) SCENARIUSZ prezentacji — roadmapa kroków
2) README — wartości: zero‑install, WebContainers, szybkie iteracje
3) Prompt → Admin Portal — krótki brief
4) Inicjalizacja projektu — analiza promptu i setup
5) CRUD + role — logika i interfejs
6) Eksport CSV — operacje wsadowe
7) Podgląd w przeglądarce — URL instancji

CTA: „Takie narzędzie wewnętrzne jesteśmy w stanie dostarczyć w dniu rozmowy.”
EOF

[ -f "$DEMO_DIR/PROMPT.md" ] || cat > "$DEMO_DIR/PROMPT.md" << 'EOF'
## Brief do Bolt.new (Admin Portal)

Zbuduj „Admin Portal” dla aplikacji B2B:
- Ekran logowania i sesje (demo)
- Widok listy użytkowników z wyszukiwaniem, filtrami (status, rola) i paginacją
- Formularz edycji użytkownika: imię, e‑mail, rola (admin/user), status (active/suspended)
- Akcje masowe: zawieś/aktywuj, reset hasła
- Eksport CSV widocznego zestawu danych
- Logi audytowe (lista ostatnich akcji adminów)
- Prosty system ról i uprawnień po stronie UI i API
EOF

cd "$DEMO_DIR"
exec "$DX_BIN_REAL"


