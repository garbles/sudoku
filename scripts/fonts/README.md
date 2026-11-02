# Fonts

The images use **reMarkable Sans**. This is the UI font of the reMarkable Paper Pro. reMarkable owns
the font. Do not put the font in version control, and do not include it in the app. The app must load
the font from the tablet when the app starts.

To make `rmsans-400.ttf` (Regular) and `rmsans-500.ttf` (Medium) again from a tablet in developer
mode, run `make fonts` from the root of the repository. It uses the tablet at `REMARKABLE_HOST`
(`.env`), or USB, and the SSH key of the installer.

## Terminal font

The screenshots of the installer (`images/install-*.png`) use **DejaVu Sans Mono** 2.37
(`dejavu-mono-400.ttf` and `dejavu-mono-700.ttf`, from
https://github.com/dejavu-fonts/dejavu-fonts/releases). The license of this font permits copies.
`DEJAVU-LICENSE` gives the license.
