# SIGIL (working codename)

An independent operating system built from scratch: its own kernel, drivers, file system,
compositor and a three-realm desktop inspired by cinematic consoles, spatial computing and
medieval dark fantasy.

> Status: **Phase 0 — planning and Design Lab.** No kernel code yet.

## Repo map
| Path | What it is |
|---|---|
| `CLAUDE.md` | Rules the AI assistant follows in this repo |
| `docs/ARCHITECTURE.md` | Kernel, drivers, compositor and app model |
| `docs/DESIGN_SYSTEM.md` | The three realms, art direction, tokens, fonts |
| `docs/ROADMAP.md` | Phases with acceptance tests |
| `docs/SAFETY.md` | How we protect the host PC |
| `docs/HARDWARE_MATRIX.md` | Tested hardware status (honest) |
| `docs/DECISIONS.md` | Decision log |
| `design-lab/` | Browser prototype for finding the visual style |

## Try the Design Lab
Open `design-lab/index.html` in any browser (fonts load from Google Fonts). Keys:
- `Q` / `E` — switch realm (or click the names at the top)
- `←` / `→` — browse cards in Theatre; arrows move through the Satchel in the Keep
- `Z` / `C` — Keep pages (Satchel, Archive, Codex)
- `Tab` or right mouse button (hold) — radial wheel in the Keep, release to invoke
- `T` — style tweaker; **Copy tokens** gives the chosen values back to Claude
