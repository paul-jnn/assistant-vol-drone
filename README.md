# Assistant Vol Drone (MGI)

Application desktop (Tauri + Rust) de préparation de vol drone : référentiel exploitant/télépilotes,
assistant SORA, formulaires (Cerfa 15476, dérogation R5-UAS-DEROG, AOT), liens utiles.

Version desktop de l'outil, reprenant l'interface de l'assistant HTML existant.
Fonctionne en ligne et hors ligne. Accès protégé par code PIN (évolutif vers clé de licence).

## Lancer en développement (nécessite Rust + Node + WebView2)
```
cargo tauri dev
```

## Compiler les installateurs
Automatique via GitHub Actions (Windows + Linux) à chaque push sur `main`.
Les installateurs sont téléchargeables dans l'onglet **Actions** du dépôt (artefacts de build).

Build local :
```
cargo tauri build
```

## Structure
- `src/`         interface (HTML/JS)
- `src-tauri/`   coque Rust (fenêtre, verrou PIN, accès fichiers)
- `.github/workflows/build.yml`  compilation Windows + Linux

_MGI — Maintenance Générale Industrielle. Aide à la préparation de vol, ne se substitue pas à la réglementation._
