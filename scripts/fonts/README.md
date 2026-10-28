# Fonts

The images use **reMarkable Sans**. This is the UI font of the reMarkable Paper Pro. reMarkable owns
the font. Do not put the font in version control, and do not include it in the app. The app must load
the font from the tablet when the app starts.

To make `rmsans-400.ttf` (Regular) and `rmsans-500.ttf` (Medium) again from a tablet in developer
mode, do these steps:

```sh
scp root@10.11.99.1:/usr/share/remarkable/webui/reMarkableSans.woff2 .
python3 -m venv venv && venv/bin/pip install fonttools brotli
venv/bin/python - <<'PY'
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
for w in (400, 500):
    f = TTFont("reMarkableSans.woff2"); f.flavor = None
    instantiateVariableFont(f, {"wght": w, "ital": 0}, inplace=True)
    f.save(f"rmsans-{w}.ttf")
PY
```

## Terminal font

The screenshots of the installer (`images/install-*.png`) use **DejaVu Sans Mono** 2.37
(`dejavu-mono-400.ttf` and `dejavu-mono-700.ttf`, from
https://github.com/dejavu-fonts/dejavu-fonts/releases). The license of this font permits copies.
`DEJAVU-LICENSE` gives the license.
