# figlens for Herdr

- `figlens/` is the mod folder shared at herdr.dev/mods: keep it to `mods/figlens/`,
  `RECIPE.md` and `preview.png`. `figlens-layout/` is the "Herdr + figlens" layout:
  `layouts/herdr-figlens.json` with its settings picture, an identical copy of
  `mods/figlens/`, `RECIPE.md` and `preview.png`. Change the mod in
  `figlens/mods/figlens/` and copy it over; a test checks the copies match.
  Tests and docs stay outside both folders.
- Follow Herdr's installed `~/.config/herdr-app/AGENTS.md`, `herdr.d.ts` and
  `gpui-kit.d.ts`. Mods are plain JavaScript in QuickJS: no Node APIs,
  `setTimeout` or `Intl`; never keep a `cx` past its callback; never notify
  during render.
- Keep every module but `main.js` (`engine.js`, `frames.js`, `journal.js`,
  `session.js`, `work.js`, `place.js`, `store.js`, `cache.js`) free of Herdr
  imports so `node --test herdr/tests/*.test.mjs` runs them; `main.js` passes
  Herdr's functions in. `VERSION` in `engine.js` is
  bumped by Release Please through a `generic` extra-files entry that rewrites
  its marked line; never edit it by hand.
- Use Herdr theme tokens only; secondary text is `muted`, never `dim`. Every
  change that pastes into an agent or deletes files asks first.
- Network: only the engine download from GitHub Releases. Programs: `figlens`,
  `hc`, `sh` with its standard tools (`stat`, `find`, `dd`, `tar`, `realpath`,
  `sha256sum`/`shasum`, `awk`, `cp`, `ls`, `rm`, `tail`, `head`, `base64`, `env`, `du`,
  `wc`, `gzip`, `gunzip`, `mktemp`, `touch`), `curl` for the download, the platform file
  picker and `open`/`xdg-open`. Keep README, RECIPEs and
  code in step on hosts, programs, storage and behavior.
- Each `preview.png` is a screenshot of the real mod or layout running in
  Herdr, in English, on a public or synthetic design; never a private design.
- Check every change with `herdr-app check herdr/figlens/mods/figlens` and look
  at it running; load the `herdr` topic for the details.
