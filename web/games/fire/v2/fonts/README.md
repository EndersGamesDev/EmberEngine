# Fire Racer page fonts

The V2 page sets its type in Barlow and Barlow Condensed. These are the seven faces the stylesheet actually uses, served from this site rather than fetched from a third party: `style.css` used to open with an `@import` of `fonts.googleapis.com`, which was the only external runtime reference anywhere in `web/`, and which put a request to another operator between the player and first paint on every load.

The files are Google Fonts' `latin`-subset woff2 builds of upstream Barlow, taken from the `latin` `@font-face` blocks of `https://fonts.googleapis.com/css2?family=Barlow+Condensed:wght@600;700;800&family=Barlow:wght@400;500;600;700&display=swap` requested with a woff2-capable browser user agent. Each file is renamed to say what it is; nothing inside it is modified.

|File|Family|Weight|Bytes|Source|
|---|---|---|---|---|
|`barlow-latin-400.woff2`|Barlow|400|22,196|`https://fonts.gstatic.com/s/barlow/v13/7cHpv4kjgoGqM7E_DMs5.woff2`|
|`barlow-latin-500.woff2`|Barlow|500|22,008|`https://fonts.gstatic.com/s/barlow/v13/7cHqv4kjgoGqM7E3_-gs51os.woff2`|
|`barlow-latin-600.woff2`|Barlow|600|22,772|`https://fonts.gstatic.com/s/barlow/v13/7cHqv4kjgoGqM7E30-8s51os.woff2`|
|`barlow-latin-700.woff2`|Barlow|700|22,788|`https://fonts.gstatic.com/s/barlow/v13/7cHqv4kjgoGqM7E3t-4s51os.woff2`|
|`barlow-condensed-latin-600.woff2`|Barlow Condensed|600|22,308|`https://fonts.gstatic.com/s/barlowcondensed/v13/HTxwL3I-JCGChYJ8VI-L6OO_au7B4873z3bWuQ.woff2`|
|`barlow-condensed-latin-700.woff2`|Barlow Condensed|700|22,444|`https://fonts.gstatic.com/s/barlowcondensed/v13/HTxwL3I-JCGChYJ8VI-L6OO_au7B46r2z3bWuQ.woff2`|
|`barlow-condensed-latin-800.woff2`|Barlow Condensed|800|22,464|`https://fonts.gstatic.com/s/barlowcondensed/v13/HTxwL3I-JCGChYJ8VI-L6OO_au7B47b1z3bWuQ.woff2`|

Seven, not nine: the old import also requested Barlow Condensed 500 and 900, and the stylesheet sets neither, so they were downloaded by every player and used by nobody.

Each `@font-face` rule in `style.css` keeps the `font-display: swap` and the `unicode-range` the imported stylesheet declared, so the page renders exactly as it did — fallback stacks first, these faces when they arrive, and system glyphs for the symbols outside the latin range that Barlow never covered.

`OFL.txt` is the SIL Open Font License 1.1 that Barlow is released under, copied from the upstream project at `https://github.com/jpt/barlow`. It is deployed beside the fonts because the licence requires its notice to travel with the font files, so it belongs in the published tree rather than in the repository alone.

`deploy/deploy-pages.sh` places this directory with the rest of the page and `deploy/tests/test-pages.sh` pins that, because a face the assembly does not ship is a silent fallback to system type rather than an error anyone sees.
