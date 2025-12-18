#!/usr/bin/env bash
set -euo pipefail

DEMO_DIR="./demos/lovable"
DX_BIN_REAL="${DX_BIN:-}"
if [ -z "$DX_BIN_REAL" ]; then
  if command -v dx >/dev/null 2>&1; then DX_BIN_REAL="$(command -v dx)"; elif [ -x ./target/release/dx ]; then DX_BIN_REAL="./target/release/dx"; else echo "Build dx first: cargo build --release" >&2; exit 1; fi
fi

mkdir -p "$DEMO_DIR"

[ -f "$DEMO_DIR/MOTD.md" ] || cat > "$DEMO_DIR/MOTD.md" << 'EOF'
<!-- dx:ascii -->
 _                 _           _          
| | ___   ___ __ _| |__   ___ | | ___   _ 
| |/ _ \ / __/ _` | '_ \ / _ \| |/ / | | |
| | (_) | (_| (_| | |_) | (_) |   <| |_| |
|_|\___/ \___\__,_|_.__/ \___/|_|\_\\__,_|

Welcome to lovable demo
EOF

[ -f "$DEMO_DIR/README.md" ] || cat > "$DEMO_DIR/README.md" << 'EOF'
# Lovable demo — realistyczny scenariusz sprzedażowy

Ten demo pokazuje, jak w kilka minut zaprezentować wartość Lovable na przykładzie „MVP SaaS z subskrypcjami”. W `dx` przechodzimy krok po kroku przez flow budowy aplikacji z promptu, integrację płatności i szybki deploy preview — bez odpalania zewnętrznych usług.

Co demonstrujemy:
- Praca „prompt → działająca aplikacja”: analiza wymagań i generacja projektu
- Integracje „out of the box”: Stripe (tryb testowy), e‑mail auth, prosty onboarding
- Podgląd lokalny i pre‑deployment URL dla recenzji
- Transparentna konfiguracja i wyłączona telemetria (zgodność/bezpieczeństwo)

Dla rozmów sprzedażowych podkreśl:
- Skrócenie czasu „idea → produkt” z tygodni do minut/godzin
- Minimalny koszt eksperymentów (szybkie prototypy, szybki feedback)
- Jakość i rozszerzalność: wygenerowany kod jest czytelny i gotowy do rozwoju

Pliki pomocnicze:
- `SCENARIO.md` — skrypt prezentera (co mówić na każdym kroku)
- `PROMPT.md` — przykładowy prompt biznesowy
- `menu.toml` — sekwencja kroków używana przez `dx`
EOF

[ -f "$DEMO_DIR/menu.toml" ] || cat > "$DEMO_DIR/menu.toml" << 'EOF'
[[items]]
name = "SCENARIUSZ prezentacji"
description = "Instrukcja krok po kroku"
file = "SCENARIO.md"

[[items]]
name = "README"
description = "Cel i wartości demo"
file = "README.md"

[[items]]
name = "Prompt → Aplikacja"
description = "Przykładowy brief biznesowy"
file = "PROMPT.md"

[[items]]
name = "1) Generowanie projektu"
description = "Analiza promptu, projekt, kod, testy"
cmd = 'for step in "Analiza promptu" "Projekt bazy danych" "Generowanie backendu" "Generowanie frontendu" "Testy i lint" "Pakowanie"; do printf "%s... " "$step"; sleep 0.4; echo OK; done; echo; echo "Repo: github.com/acme/saas-starter (demo)";'

[[items]]
name = "2) Lokalny podgląd"
description = "Dev server i logi uruchomienia"
cmd = 'echo "Uruchamianie dev servera na http://localhost:3000"; for i in $(seq 1 6); do echo "[dev] log $i"; sleep 0.4; done; echo "Dev server gotowy (symulacja)"'

[[items]]
name = "3) Integracja: Stripe (test)"
description = "Ustawienia kluczy + webhook"
cmd = 'echo "Konfiguracja Stripe (tryb testowy)"; echo "STRIPE_PUBLIC_KEY=pk_test_xxx"; echo "STRIPE_SECRET_KEY=sk_test_xxx"; sleep 0.6; echo "Rejestrowanie webhooku…"; sleep 0.6; echo "Webhook OK: https://example.test/stripe/webhook"'

[[items]]
name = "4) Deploy preview"
description = "URL do akceptacji zmian"
cmd = 'for s in "Budowanie" "Publikacja artefaktów" "Tworzenie URL"; do printf "%s... " "$s"; sleep 0.5; echo OK; done; echo; echo "Preview: https://preview.lovable.dev/acme-saas-123"'

[[items]]
name = "Konfiguracja: telemetry OFF"
description = "Potwierdzenie w config.toml"
file = "config.toml"
EOF

[ -f "$DEMO_DIR/SCENARIO.md" ] || cat > "$DEMO_DIR/SCENARIO.md" << 'EOF'
## Skrypt prezentera (Lovable)

Cel: pokazać, jak w 7–10 minut powstaje MVP SaaS: e‑mail login, onboarding, subskrypcje Stripe i podgląd deploy.

Kroki w menu:
1) SCENARIUSZ prezentacji — wprowadzenie (to okno)
2) README — wartości biznesowe i co mierzymy (czas do wartości, koszt iteracji)
3) Prompt → Aplikacja — krótki brief produktowy, który „karmimy” Lovable
4) Generowanie projektu — symulacja etapów (analiza, kod, testy)
5) Lokalny podgląd — pokazujemy gotowość dev servera
6) Integracja: Stripe (test) — klucze i webhook w trybie testowym
7) Deploy preview — link do akceptacji zmian
8) Konfiguracja: telemetry OFF — compliance/bezpieczeństwo

Call‑to‑action: „Ten sam proces możemy odpalić na Twoich wymaganiach jeszcze dziś.”
EOF

[ -f "$DEMO_DIR/PROMPT.md" ] || cat > "$DEMO_DIR/PROMPT.md" << 'EOF'
## Brief do Lovable (przykład)

Zbuduj MVP SaaS „Acme Analytics”:
- Logowanie przez e‑mail (magic link) i reset hasła
- Dashboard z metrykami (MAU, przychód MRR, churn) i filtrem zakresu dat
- Panel admin: lista użytkowników, nadawanie ról (admin, user)
- Subskrypcje Stripe: plany Basic/Pro, webhook do aktualizacji statusu konta
- Strona pricing, checkout i faktury PDF
- Onboarding po rejestracji: 3 kroki (profil, integracja, pierwszy raport)
- Responsywny UI (Tailwind), i18n (EN, PL)
- Testy podstawowe i instrukcja uruchomienia
EOF

cd "$DEMO_DIR"
exec "$DX_BIN_REAL"


