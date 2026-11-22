# Design system

## The three realms
Home is three side-by-side realms. Switch with `Q`/`E`, `←`/`→`, swipe, or LB/RB.

### Left — THEATRE (cinematic console)
Full-bleed atmospheric artwork of the focused item, a whisper-quiet status line at the top,
bold card rows at the bottom, button-prompt footer. No file trees, no tiny buttons. Gamepad-first.

### Center — AETHER (spatial)
Content floats as smoked-glass panels with depth and soft light. Navigation and controls are
**detached pills** orbiting the content (tab rail on the left, toolbar below), never a heavy frame.
Mouse/keyboard-first.

### Right — THE KEEP (medieval dark fantasy)
Owner's references: medieval history, Elden Ring, The Witcher 3, RDR2. Ingredients:
- **Ink-brush black panels** with rough torn edges; ivory **engraved/woodcut-style** icons
- **Tarnished-gold hairline frames** with small ornamental corners and diamond dividers
- **Illustrated parchment maps** — the file manager is a world map (drives = regions, folders = landmarks)
- **Radial wheel** (hold) for quick actions; **inventory grid** for apps ("Satchel")
- **Skill-tree** layout for Settings; **wax-seal** notifications; one original **sigil** per app
- Footer button prompts like a game pause menu; monochrome pause backdrop

All art is original. The game images in the brief are mood references only.

## Tokens (shared; realms override)
| Token | Hex | Use |
|---|---|---|
| canvas-void | `#070709` | base background |
| smoked-glass | `#0C0B0E` @ 80% | floating panels |
| tarnished-gold | `#C8A96E` | focus, active frames, hairlines |
| dim-gold | `#6E5B3A` | inactive frames and rules |
| wax-crimson | `#8F1D21` | alerts, close, selection (Keep) |
| parchment-ivory | `#E6DEC8` | primary text, icons |
| ash-grey | `#8A8478` | metadata, hints |
| parchment | `#D9C9A3` | map/paper surfaces |
| ink | `#0E0D0C` | ink-brush panels |

## Type
| Role | Font | License |
|---|---|---|
| Display / headers | Cinzel | OFL |
| Body | EB Garamond | OFL |
| Telemetry / logs | JetBrains Mono | OFL |
| Theatre / Aether UI | Inter (proposal) | OFL |

## Spacing & shape
4-pt grid (4, 8, 12, 16, 24, 32, 48, 64). Aether radius 28 (panels) / 999 (pills).
Keep radius 0–2 with ornamented corners. Theatre cards radius 12.

## Motion
Realm switch 420 ms ease-out slide + parallax backdrop. Focus 120 ms. Radial wheel opens 160 ms.

## Status
Values are **proposals** until the owner confirms them in the Design Lab (`T` tweaker panel).
