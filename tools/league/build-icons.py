"""Original vector UI emblems; raster character/arena art comes from the fleet."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ICONS = {
    'swarm-q': '<path d="m32 6 10 9-10 9-10-9Zm-17 25 10 9-10 9L5 40Zm34 0 10 9-10 9-10-9Z"/><circle cx="32" cy="15" r="3"/><circle cx="15" cy="40" r="3"/><circle cx="49" cy="40" r="3"/><path d="m27 29-5 5m15-5 5 5M27 48h10"/>',
    'swarm-w': '<circle cx="15" cy="32" r="9"/><circle cx="15" cy="32" r="3"/><path d="M24 32h34M29 24l27-7M29 40l27 7m-9-20 11 5-11 5"/>',
    'swarm-e': '<circle cx="19" cy="30" r="12"/><circle cx="19" cy="30" r="4"/><circle cx="46" cy="30" r="12" stroke-dasharray="4 4"/><path d="m20 49 12 7 12-7M28 8l4 6 4-6"/>',
    'swarm-r': '<path d="m32 21 11 11-11 11-11-11ZM9 9l9 5-4 4Zm46 0-9 5 4 4ZM9 55l9-5-4-4Zm46 0-9-5 4-4Z"/><path d="m18 18 7 7m21-7-7 7M18 46l7-7m21 7-7-7"/><circle cx="32" cy="32" r="4"/>',
    'knight-q': '<path d="M28 58c-7-5-6-12 4-17 19-10 21-16 10-21M14 48c-11-13 23-14 31-25M15 35C7 22 41 21 41 9c10 6 15 18 5 27M26 20c-7-4-9-10-5-15 1 7 8 5 9 10"/>',
    'knight-w': '<path d="m32 6 21 8v18c0 13-13 22-21 27-8-5-21-14-21-27V14Z"/><path d="M32 44c-10-1-12-10-5-17l3-7 4 9 5-4c9 10 4 18-7 19Z"/>',
    'knight-e': '<path d="m11 53 7-7m-7-7 14 14M20 43 46 8l9-2-2 9-29 32ZM28 35l14-17"/><path d="M39 43c-4-9 4-12 6-17 7 9 12 15 5 22-6 6-12 4-15-1"/>',
    'knight-r': '<path d="m18 28-9-8-3-13 16 10 10-5 10 5L58 7l-3 13-9 8 4 17-18 14-18-14Z"/><path d="m19 33 10 5m16-5-10 5m-11 9 8 5 8-5M32 20v11"/>',
    'hallow-q': '<path d="M32 52 11 33C-3 18 17 5 32 23 47 5 67 18 53 33Z"/><path d="M32 29v16m-8-8h16M10 8l3 6m38-6-3 6"/>',
    'hallow-w': '<path d="M10 45C16 21 33 8 57 8 44 15 38 24 35 34l-13 5m-4-11h17M5 52l30-9m-5 11 15-5"/><path d="m32 20 11 3m-6-11 12 2"/>',
    'hallow-e': '<path d="m32 16 19 8v14c0 10-12 18-19 21-7-3-19-11-19-21V24Z"/><ellipse cx="32" cy="10" rx="20" ry="5"/><path d="M32 29v17m-8-9h16"/>',
    'hallow-r': '<path d="M17 40V28C17 7 47 7 47 28v12l8 16H9Z"/><path d="m24 28 8-10 8 10-8 8ZM32 41v10m-5-5h10M8 22A25 25 0 0 1 48 8m-1-6 3 8-9 1"/>',
    'maw-q': '<path d="M36 7v27c0 18-27 18-27 2V25l8 9m-8-9-4 11M29 7h14M45 11l9 8m-9 0 9 8m-9 0 9 8"/>',
    'maw-w': '<path d="M10 50c-8-9-2-21 8-22-4-20 22-25 28-10 17-1 19 24 7 28 2 10-7 15-15 10-9 7-20 2-20-5Z"/><circle cx="23" cy="38" r="3"/><circle cx="41" cy="30" r="4"/><path d="M9 59h47"/>',
    'maw-e': '<path d="m32 6 22 12-4 25-18 15-18-15-4-25Z"/><path d="m10 18 22 9 22-9M14 43l18-16 18 16M32 6v21m0 0v31m-14-9 14-8 14 8"/>',
    'maw-r': '<path d="M5 45h12l6-16 9 30 8-30 7 16h12M8 24l8-6m32 0 8 6M14 10l9 7m18 0 9-7M28 7l4 10 4-10"/><path d="m23 26 9-5 9 5"/>',
    'tessera-q': '<path d="m32 6 19 10v24L32 58 13 40V16Zm0 0v52M13 16l19 16 19-16M13 40l19-8 19 8"/>',
    'tessera-w': '<circle cx="32" cy="33" r="18"/><circle cx="32" cy="33" r="4"/><path d="M32 15V5m0 56V51M14 33H4m56 0H50M19 20l-7-7m33 7 7-7M19 46l-7 7m33-7 7 7M25 28l14 10m-14 0 14-10"/>',
    'tessera-e': '<path d="M23 10h32M23 54h32M27 10c0 12 6 14 12 22-6 8-12 10-12 22m24-44c0 12-6 14-12 22 6 8 12 10 12 22M5 21h17M8 32h14M5 43h17"/>',
    'tessera-r': '<circle cx="32" cy="33" r="23"/><path d="M27 24v19m10-19v19M24 4h16M32 10V4M12 13l-5-5m-2 7L14 4M18 33h-5m38 0h-5M32 14v5m0 28v5"/>',
    'flash': '<path d="M37 4 12 36h17l-4 24 27-35H35ZM5 19h13M3 48h13m32-4h13"/>',
    'heal': '<path d="M24 8h16v16h16v16H40v16H24V40H8V24h16Z"/><path d="m13 9 4 4m34-4-4 4m4 38-4-4M13 51l4-4"/>',
    'smite': '<path d="m35 5-8 21h10l-8 23M10 42l10 15h24l10-15M14 31l6 6m30-6-6 6M32 57v5"/>',
    'exhaust': '<path d="M16 14h32c0 12-4 13-16 20-12 7-16 8-16 21h32c0-13-4-14-16-21-12-7-16-8-16-20Zm-3-6h38M10 22l-6 8 6 8m44-16 6 8-6 8"/>',
    'sword': '<path d="m12 54 9-9m-9-6 13 13M22 44 46 8l10-1-1 10-29 31Zm8-13 14-14"/>',
    'crystal': '<path d="m32 5 19 16-7 29-12 9-12-9-7-29Zm0 0 7 19-7 35-7-35Zm-19 16 12 3 14 0 12-3"/>',
    'heart': '<path d="M32 55 10 34C-1 22 15 8 32 24 49 8 65 22 54 34Z"/><path d="m20 30 12 14 12-14"/>',
    'boots': '<path d="m14 9 18 3-2 25 22 9v10H10l1-18Zm0 13 17 3m-18 8 17 3M10 50h42M20 10l-2 25"/>',
    'ring': '<ellipse cx="32" cy="39" rx="18" ry="18"/><ellipse cx="32" cy="39" rx="11" ry="12"/><path d="m22 17 10-12 10 12-10 11Z"/>',
    'coin': '<circle cx="32" cy="32" r="24"/><circle cx="32" cy="32" r="18"/><path d="m32 17 5 10 11 2-8 8 2 11-10-5-10 5 2-11-8-8 11-2Z"/>',
    'dagger': '<path d="m9 54 13-13m-10-3 14 14M24 40 45 6l9 3-3 16-25 17ZM35 30l12-14"/>',
    'font': '<path d="M14 26h36c0 13-8 20-18 20s-18-7-18-20Zm18 20v10M21 58h22M32 5c-12 14-5 22 0 22s12-8 0-22Z"/>',
    'windstep': '<path d="M9 13h20l-1 21 25 11v11H8V38Zm1 12h18M36 12h20M34 21h24M37 30h14"/>',
    'orb': '<circle cx="32" cy="30" r="18"/><path d="M14 48c10 10 26 10 36 0M8 19l7 8m34 0 7-8M32 46V16m-9 10 9 6 9-6"/>',
    'emberbrand': '<path d="m13 55 9-11m-10-4 14 13M25 40 40 9l9-5 3 13-23 26Z"/><path d="M40 48c9-4 15-10 10-19 14 15 6 30-11 28"/>',
    'storm': '<path d="M8 29c-5-9 2-16 11-15 5-14 24-11 27 1 13-2 18 16 7 21H39M32 21 17 44h13l-3 17 18-26H33Z"/>',
    'duskveil': '<path d="M10 54V26C10 1 54 1 54 26v28L43 43l-11 8-11-8Zm12-21c6-10 14-10 20 0l-10 8Z"/>',
    'sunspear': '<path d="m11 56 23-29m-6 0L44 4l12 5-20 25ZM15 9l6 8m17 28 11 4M7 30l10 2m35-10 8 1"/>',
    'aegis': '<path d="m32 5 22 10v20c0 12-14 21-22 25-8-4-22-13-22-25V15Zm0 9 14 7v14c0 7-8 14-14 17-6-3-14-10-14-17V21Z"/>',
    'ruin': '<path d="M11 28V14h9v14-18h10v18-20h10v20-15h10v24l-9 19H22L9 40l-4-12Zm13 7h18M22 46h20"/>',
    'hpotion': '<path d="M24 5h16v8l-3 8 14 18c9 16-2 20-19 20S4 55 13 39l14-18-3-8Zm-3 8h22M15 38h34M32 41v12m-6-6h12"/>',
    'mpotion': '<path d="M24 5h16v8l-3 8 14 18c9 16-2 20-19 20S4 55 13 39l14-18-3-8Zm-3 8h22M15 38h34m-17 2-7 10 7 5 7-5Z"/>',
}

out = ['<svg xmlns="http://www.w3.org/2000/svg">']
for name, markup in ICONS.items():
    out.append(f'<symbol id="{name}" viewBox="0 0 64 64"><g fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round">{markup}</g></symbol>')
out.append('</svg>')
target = ROOT / 'web/games/league/v2/art/icons.svg'
target.write_text('\n'.join(out) + '\n', encoding='utf-8', newline='\n')
print(f'{len(ICONS)} original vector emblems, {target.stat().st_size} bytes')
