# Assistant Vol Drone — Dossier technique

**Éditeur :** M.G.I. — Maintenance Générale Industrielle
**Version du document :** 2.0 (application en version 0.2.7)
**Application :** Assistant Vol Drone — préparation de vol drone (référentiel, régimes d'exploitation, SORA 2.5, formulaires officiels, MANEX, rapport de mission, terrain)
**Public visé :** l'utilisateur-mainteneur de l'application (vous), pour comprendre, exploiter, faire évoluer et dépanner l'outil.
**Code source :** public, https://github.com/paul-jnn/assistant-vol-drone

---

## 1. À quoi sert l'application

L'Assistant Vol Drone est une application de bureau qui accompagne la préparation d'un vol drone professionnel, du choix du régime jusqu'au dossier présentable en contrôle et au rapport remis au client.

Elle conserve les informations de l'exploitant et des télépilotes, gère un dossier par mission, et pour chaque mission propose le **régime d'exploitation** adapté (catégorie ouverte A1/A2/A3, ou spécifique via STS, PDRA ou SORA), déroule l'analyse correspondante, produit une check-list pré-vol et un journal de vol, et génère un dossier de vol PDF complet. Elle embarque une base de drones DJI, pré-remplit les formulaires officiels (Cerfa 15476, dérogation, AOT), localise le site par adresse ou sur une carte, récupère la météo aéronautique, génère une trame de MANEX et un rapport de mission client à votre en-tête, et stocke vos justificatifs (MANEX, assurance, attestations) pour les consulter et les présenter en cas de contrôle, y compris dans un lecteur PDF intégré.

Elle fonctionne hors ligne pour l'essentiel (seules la carte, la météo et la localisation nécessitent Internet). Les données restent sur le poste. L'accès est protégé par un code PIN, et l'application se met à jour automatiquement.

Elle ne remplace pas la réglementation ni le jugement du télépilote : c'est une aide à la saisie, au calcul et à l'archivage, à vérifier avant tout dépôt officiel.

---

## 2. Sur quoi l'application est construite

C'est un logiciel **Tauri** : deux moitiés qui coopèrent.

**La coque (back-end), en Rust.** Le programme natif Windows/Linux : il ouvre la fenêtre, lit et écrit les fichiers sur le disque, calcule l'empreinte du PIN, détient les gabarits PDF, va chercher la météo, ouvre et enregistre les fichiers, et pilote les mises à jour. C'est la seule partie qui a le droit de toucher au disque et au réseau.

**L'interface (front-end), en HTML/CSS/JavaScript.** Tout ce que vous voyez, dans une vue web intégrée à la fenêtre (WebView2 sur Windows, WebKitGTK sur Linux). Elle ne touche jamais au disque directement : elle **demande à la coque Rust** via un appel `invoke`.

Ce choix donne la légèreté d'une interface web avec la robustesse et l'accès système d'un logiciel natif, dans un seul exécutable de quelques mégaoctets, installable par clé USB et fonctionnant hors ligne. Le pont unique entre les deux moitiés est `invoke('commande', {arguments})`.

Deux bibliothèques externes sont récupérées automatiquement à la compilation et embarquées dans l'application : **pdf-lib** (remplissage des PDF officiels) et **Leaflet** (carte). Côté Rust, l'application s'appuie sur trois composants Tauri : **opener** (ouvrir un fichier ou une URL), **updater** (mise à jour automatique) et **dialog** (boîte « Enregistrer sous »), plus **reqwest** (requêtes météo).

---

## 3. Arborescence du projet

```
assistant-vol/
├─ src/                         INTERFACE (front-end)
│  ├─ index.html                toute l'application web : écrans, données, calculs
│  └─ vendor/                   bibliothèques récupérées au build par la CI
│     ├─ pdf-lib.min.js         remplissage des PDF
│     ├─ leaflet.js             carte interactive
│     └─ leaflet.css
│
├─ src-tauri/                   COQUE (back-end Rust)
│  ├─ src/
│  │  ├─ lib.rs                 les commandes Rust (largement commentées)
│  │  └─ main.rs                point d'entrée
│  ├─ forms/                    gabarits officiels embarqués dans le binaire
│  │  ├─ cerfa_15476-04.pdf
│  │  └─ form_r5-uas-derog_v4.pdf
│  ├─ icons/                    icônes de l'application
│  ├─ capabilities/default.json permissions accordées à l'interface
│  ├─ tauri.conf.json           configuration (nom, fenêtre, updater, version)
│  ├─ Cargo.toml                dépendances Rust
│  └─ build.rs
│
├─ .github/workflows/
│  ├─ build.yml                 compilation de test à chaque push sur main
│  └─ release.yml               release signée à chaque tag v*  (mise à jour auto)
├─ docs/DOSSIER_TECHNIQUE.md    ce document
├─ README.md
└─ .gitignore
```

Le fichier central pour vous est `src/index.html` : toute l'interface et toute la logique métier. Le Rust est court et stable ; on y touche pour ajouter une capacité système (stockage, réseau, mise à jour). Les deux fichiers de code, `lib.rs` et `index.html`, sont **abondamment commentés en français** (voir section 19).

Trois emplacements gardent des données, **hors du programme**, dans le dossier de configuration de l'application propre à chaque poste : `pin.hash` (empreinte du code PIN), `donnees.json` (toutes vos saisies), et le sous-dossier `documents/` (vos justificatifs importés). Ils survivent aux mises à jour et aux réinstallations.

---

## 4. La coque Rust (`src-tauri/src/lib.rs`)

Le Rust expose **dix-huit commandes**. Chacune est une fonction précédée de l'étiquette `#[tauri::command]`, ce qui la rend appelable depuis l'interface par `invoke('nom', {arguments})`.

| Commande | Rôle |
|---|---|
| `pin_status` / `set_pin` / `verify_pin` | Verrou par code PIN (empreinte SHA-256, jamais en clair). |
| `data_get` / `data_set` | Lit / enregistre toutes les données (JSON), écriture atomique. |
| `form_template` | Renvoie un gabarit PDF officiel embarqué (base64). |
| `export_file` | Écrit un fichier généré (PDF, dossier, lettre) dans `Documents\Assistant Vol Drone\`. |
| `open_file` | Ouvre un fichier avec l'application par défaut du système (non restreint à un dossier). |
| `save_bytes` | Écrit des octets à l'emplacement choisi par l'utilisateur (« Enregistrer sous »). |
| `http_get` | Requête météo restreinte à aviationweather.gov (METAR/TAF). |
| `check_update` / `apply_update` | Vérifie et applique une mise à jour, puis redémarre. |
| `import_doc` / `list_docs` / `doc_path` / `read_doc` / `export_doc` / `delete_doc` | Gestion des justificatifs (import, liste, chemin, lecture intégrée, export, suppression). |

Le PIN est stocké en empreinte SHA-256 salée : le fichier `pin.hash` ne contient jamais le code. Les gabarits Cerfa et dérogation sont incorporés au binaire (`include_bytes!`), donc toujours disponibles hors ligne. `http_get` refuse toute URL hors du domaine météo autorisé, pour ne pas faire de l'application un relais réseau ouvert. Deux commandes séparent volontairement deux besoins d'écriture : `export_file` écrit à un emplacement imposé (`Documents\Assistant Vol Drone\`) et l'ouvre, tandis que `save_bytes` écrit à l'emplacement exact que l'utilisateur choisit dans la boîte « Enregistrer sous ». Les documents importés sont copiés dans le sous-dossier `documents/` du dossier de config ; `read_doc` en renvoie le contenu pour l'afficher dans le lecteur PDF intégré, `export_doc` les recopie dans `Documents\Assistant Vol Drone\Justificatifs\`.

Au lancement, `run()` branche les composants opener, updater et dialog, déclare les dix-huit commandes, et démarre. `main.rs` ne fait qu'appeler `run()`.

---

## 5. L'interface (`src/index.html`)

Tout est dans un seul fichier, sans framework : du JavaScript natif. Un bloc d'en-tête au début du `<script>` résume l'architecture et sert de point d'entrée à la lecture. Le fichier se lit en trois couches.

### 5.1 Les données

L'état complet tient dans un objet `store` : `exploitant`, `pilotes` (chacun avec mentions cochées et habilitations datées), `referent`, `dossiers`, le `logo`, et le dossier courant. Un **dossier** (une mission) contient l'intitulé, le client, le lieu, les dates, l'appareil, le **régime** et la sous-catégorie, l'**analyse** (GRC/ARC pour le SORA), le **site** (adresse, coordonnées, aérodrome, météo relevée), la **check-list pré-vol**, le **journal**, et les paramètres des formulaires.

À chaque modification, l'interface met à jour `store` puis appelle une sauvegarde différée (`scheduleSave`, qui écrit via `data_set` après une courte pause pour ne pas enregistrer à chaque frappe). Au démarrage, `loadStore()` lit le JSON via `data_get`. Une installation neuve démarre **vierge** : aucune donnée personnelle n'est embarquée dans le programme. La fonction `normalize()` garantit qu'un `store` ancien ou incomplet est ramené à la forme attendue (compatibilité ascendante et migrations automatiques).

### 5.2 La navigation et les cartes repliables

Sans framework : `renderView()` reconstruit la zone principale avec de petites fonctions utilitaires (`el`, `card`, `field`). Le bandeau de gauche donne accès à : Exploitant, Télépilotes, Dossiers de vol, Check-list pré-vol, Documents / MANEX, Liens & contacts. Ouvrir un dossier affiche un écran à onglets dont le contenu **s'adapte au régime** choisi.

Les sections de référence (Exploitant, Image de marque, Documents / MANEX / Sauvegarde, Liens, et les fiches télépilotes) s'affichent en **accordéon** : un bandeau cliquable qui déroule ou replie son contenu. C'est le rôle de `collCard()`, la version repliable de `card()`, qui mémorise l'état ouvert/fermé de chaque section pendant la session. Les écrans de saisie guidée d'un dossier (Mission, Site, Régime, Conformité, Check-list, Journal, SORA) restent en cartes fixes : on y travaille de haut en bas, replier une étape en cours desservirait.

### 5.3 La logique métier

Trois blocs de connaissance sont codés dans l'interface : les tables SORA 2.5, la base DJI, et les correspondances de champs des formulaires. Ils sont détaillés ci-après.

---

## 6. Régimes d'exploitation et recommandation

À la création d'un dossier, vous renseignez quelques infos de base (classe C du drone, type de vol VLOS/BVLOS, environnement survolé, distance aux tiers, hauteur). L'onglet **Régime** applique alors une logique de recommandation (`recommendRegime`) qui propose le cadre le plus adapté, avec sa justification, et vous laisse le choix final :

- **Catégorie ouverte (A1/A2/A3)** : pas de SORA. Drone C0-C4, vol en vue, ≤ 120 m, hors rassemblement. Le module vérifie la cohérence sous-catégorie / classe / distances (`openConformite`).
- **Catégorie spécifique — STS** (déclaration) : STS-01 (VLOS, drone C5, zone au sol contrôlée) ou STS-02 (BVLOS avec observateurs, drone C6, zone peu peuplée).
- **Catégorie spécifique — PDRA** (autorisation allégée) : scénarios de risque prédéfinis (S01, S02, G01 à G03).
- **Catégorie spécifique — SORA** (autorisation) : analyse de risque complète, moteur détaillé à la section 7.

La recommandation est une aide au dégrossissage : les conditions exactes d'un STS ou d'un PDRA priment, à confronter à la fiche officielle. Les scénarios nationaux S-1/S-2/S-3 sont caducs depuis le 1er janvier 2026 et ne figurent plus dans l'outil.

---

## 7. Le moteur SORA 2.5

Actif quand le régime retenu est SORA. Le calcul (`computeSora`) suit le guide DGAC.

**Risque au sol (GRC).** L'iGRC croise la colonne de l'appareil (la plus pénalisante entre dimension et vitesse) et la ligne de densité de population. La Note 1 (moins de 250 g et ≤ 25 m/s) donne iGRC 1. Les atténuations M1(A), M1(B), M1(C) et M2 réduisent l'iGRC jusqu'à un plancher, donnant le GRC final.

**Risque air (ARC).** Un arbre de décision (`computeARC`) détermine l'ARC initial (a à d) selon l'espace aérien ; une réduction justifiée peut donner l'ARC final, qui fixe le niveau d'atténuation tactique requis.

**SAIL et OSO.** Le SAIL (I à VI) résulte du croisement GRC × ARC ; un GRC > 7 bascule en catégorie certifiée. Le SAIL fixe le niveau de robustesse exigé (Faible/Moyen/Haut) pour chaque OSO. Le moteur a été vérifié sur plusieurs cas connus. La fonction `computeSora` est commentée étape par étape dans le code.

---

## 8. La base de référence DJI

`DJI_DB` liste les modèles courants (Mini, Air, Mavic 3, Mavic 3 Enterprise, Matrice, FlyCart, Agras) avec, pour chacun : dimension caractéristique (m), vitesse max (m/s), masse (g) et **classe C** usuelle. Les valeurs viennent des fiches constructeur. À la sélection, ces données remplissent le dossier ; il ne reste que le numéro de série. La classe C pré-remplie est indicative : le marquage réel dépend de l'exemplaire, le champ est modifiable. Ajouter un modèle = ajouter une ligne dans `DJI_DB`.

---

## 9. Repérage du site : adresse, carte, météo

Vous localisez le site sans connaître les coordonnées : soit en saisissant **adresse / code postal / ville** puis « Localiser » (géocodage via OpenStreetMap Nominatim, qui remplit latitude et longitude), soit en ouvrant une **carte interactive** (Leaflet + tuiles OpenStreetMap) où vous cherchez une adresse ou cliquez directement le point de vol. Le champ « Lieu / site » se remplit alors automatiquement.

Le bouton « Carte des restrictions ici » ouvre Géoportail centré sur le point. Le bouton « Relever la météo » interroge aviationweather.gov (via la commande Rust `http_get`) pour l'aérodrome dont vous donnez le code OACI, et rapatrie le METAR/TAF (vent, température, visibilité, catégorie VFR/IFR) dans le dossier et le PDF.

Ces trois fonctions nécessitent Internet ; hors ligne, l'application le signale et vous pouvez saisir les coordonnées à la main.

---

## 10. Préparation, journal et dossier de vol PDF

Chaque dossier comporte, quel que soit le régime, une **check-list pré-vol** (NOTAM, météo, zones, périmètre, matériel, batteries, assurance, documents, urgences, briefing) et un **journal de vol** (horaires, nombre de vols, incidents par session). La check-list est aussi consultable seule depuis le bandeau de gauche, sans monter de dossier, et imprimable vierge.

Le bouton **« Générer le dossier de vol »** produit un document PDF complet rassemblant exploitant, télépilote, appareil, mission, météo, régime et conformité, check-list et journal, prêt à présenter au client ou en contrôle. Il s'ouvre dans le navigateur pour impression ou enregistrement en PDF.

---

## 11. Les formulaires officiels

Dans un dossier, l'onglet documents produit trois pièces pré-remplies depuis l'exploitant, le télépilote référent et la mission. Le **Cerfa 15476*04** et la **dérogation R5-UAS-DEROG** sont de vrais PDF à champs, remplis par pdf-lib (gabarits embarqués dans le binaire, noms de champs relevés et vérifiés dans les formulaires officiels), puis écrits par `export_file` et ouverts automatiquement. L'**AOT** est une lettre générée (le Cerfa 14023 voirie n'étant pas interactif). Les documents restent à compléter et signer.

---

## 12. MANEX, rapport de mission et image de marque

L'écran **Documents / MANEX** propose un **générateur de trame de MANEX** (`genManex`) : un manuel d'exploitation pré-rempli à partir de votre référentiel (exploitant, télépilotes, matériel, contacts), suivant le plan standard parties A à E. C'est un point de départ à relire et adapter, pas un MANEX validé. Une fois votre MANEX abouti, vous l'importez comme justificatif pour le garder à portée en contrôle.

Sur un dossier, le bouton **rapport de mission client** (`genRapport`) produit un document orienté client — en-tête à votre logo, prestation réalisée, moyens, conditions, déroulé — à envoyer après la mission.

Ces documents portent votre **image de marque** : sur l'écran Exploitant, la section « Image de marque » permet d'importer votre **logo** (PNG/JPG), stocké sur le poste dans `store.logo` et repris automatiquement en en-tête du MANEX et des rapports.

---

## 13. Documents / MANEX et sauvegarde

L'espace **Documents / MANEX** (bandeau de gauche) permet d'importer vos justificatifs (MANEX, attestation d'assurance, attestations de formation…), stockés localement, puis de les **lire** (dans un lecteur PDF intégré à l'application, sans logiciel externe), **ouvrir** (dans le logiciel par défaut du système), **exporter** (« Enregistrer sous » à l'emplacement de votre choix, ou copie dans `Documents\Assistant Vol Drone\Justificatifs\`) ou supprimer. C'est l'appui du scénario de contrôle sur zone : tout se montre depuis l'application.

Le même écran propose la **sauvegarde des données** : « Exporter » écrit un fichier JSON reprenant l'ensemble de vos données (exploitant, télépilotes, dossiers), via la boîte « Enregistrer sous » ; « Importer » restaure ce fichier (avec confirmation) — utile comme sauvegarde ou pour transférer sur un autre poste.

---

## 14. Sécurité et données

Accès verrouillé par **code PIN** (empreinte SHA-256, jamais en clair). Toutes les données restent **sur le poste**, dans le dossier de configuration de l'application ; rien n'est envoyé sur Internet en dehors des appels explicites de localisation et de météo. Une installation neuve démarre vierge : le programme distribué ne contient aucune donnée personnelle.

Point d'attention : `donnees.json` est en clair. Le PIN protège l'ouverture de l'application, pas le fichier. Pour un poste partagé, comptez sur la session Windows et le chiffrement du disque. La sauvegarde JSON (section 13) est le filet de sécurité recommandé avant toute manipulation importante.

---

## 15. Mise à jour automatique

L'application vérifie au démarrage s'il existe une version plus récente et, le cas échéant, affiche une bannière « Mise à jour disponible → Installer & redémarrer » qui télécharge, installe et relance en un clic (commandes Rust `check_update` / `apply_update`). Si aucune mise à jour n'est disponible ou si le poste est hors ligne, rien ne s'affiche : le mécanisme est silencieux et ne bloque jamais l'application.

Le mécanisme repose sur trois pièces : une **clé de signature** (la clé publique est dans la configuration, la clé privée est un secret GitHub jamais dans le code) ; un **endpoint** pointant sur les *releases* GitHub du dépôt ; et le workflow **release.yml**, déclenché par un tag `vX.Y.Z`, qui compile en signé, publie une release et son manifeste `latest.json`. Les mises à jour **ne touchent pas** vos données (elles vivent hors du binaire). L'endpoint étant public, le dépôt doit rester **public** pour que la vérification fonctionne.

Cycle pour publier une version : bump du numéro dans `tauri.conf.json`, `git push`, puis `git tag vX.Y.Z && git push origin vX.Y.Z`. La release se construit, et les applications installées la proposent au lancement suivant.

---

## 16. Installation, compilation et CI

Le code vit sur GitHub. Deux workflows :

- **build.yml** : à chaque `git push` sur `main`, compile Windows + Linux et publie les installateurs comme artefacts (onglet Actions). Sert à tester une itération.
- **release.yml** : à chaque tag `v*`, compile en **signé** et publie une **release** avec les installateurs et le `latest.json` de mise à jour.

Les deux récupèrent pdf-lib et Leaflet avant de compiler, et signent le build : le secret GitHub `TAURI_SIGNING_PRIVATE_KEY` doit être présent, sinon la compilation échoue. Vous n'avez rien à compiler vous-même ; les installateurs sont téléchargeables dans Actions (build) ou dans Releases (versions). Le partage par clé USB reste possible : l'installateur est autonome et fonctionne hors ligne.

---

## 17. Dépannage

**La mise à jour ne se propose pas.** Il faut qu'une **release** (pas seulement un build) existe, avec un numéro supérieur à la version installée, visible dans l'onglet Releases, le dépôt **public**, et le poste connecté à Internet. Sans release plus récente, c'est normal.

**Le build échoue sur « Build (Tauri) » avec une erreur de signature.** Le secret `TAURI_SIGNING_PRIVATE_KEY` manque ou est mal collé : recréez-le puis relancez le workflow.

**Un avertissement « Node.js 20 obsolète » apparaît dans Actions.** C'est cosmétique et n'empêche pas le build. Il disparaît en passant les actions GitHub à leur version récente (ex. `actions/checkout@v5`).

**Un lien, un formulaire, la météo ou la carte ne répondent pas.** Vérifiez la connexion Internet (carte, météo, localisation en ont besoin). Pour un formulaire, reprenez la dernière version compilée.

**Le PIN est oublié.** Pas de récupération : supprimez `pin.hash` dans le dossier de config (les données restent). L'application redemandera d'en créer un.

**Crainte de perdre des données.** Les données sont hors du programme et survivent aux mises à jour ; en complément, faites un export JSON régulier (section 13).

---

## 18. Accès au code source

L'application est ouverte : tout son code est public sur GitHub, et accessible directement depuis l'application, écran **Liens & contacts**, section « Code source & documentation ». Trois accès :

- **Code source** : `https://github.com/paul-jnn/assistant-vol-drone` — l'intégralité du code (interface et coque).
- **Versions & installateurs** : `.../releases` — pour télécharger une version Windows ou Linux.
- **Dossier technique** : `.../blob/main/docs/DOSSIER_TECHNIQUE.md` — ce document.

---

## 19. Lire et modifier le code

Les deux fichiers de code sont **commentés en français**, pensés pour une lecture par quelqu'un qui a les bases de Python et de CSS, sans connaître Rust.

**`src-tauri/src/lib.rs` (la coque Rust).** Un en-tête explique les tournures Rust utiles (`fn`, `let`, `Result`, `?`, `match`, `#[tauri::command]`) avec leurs équivalents Python. Chaque commande est précédée d'un commentaire disant ce qu'elle fait et pourquoi. Pour **ajouter une capacité système** : écrire une fonction avec l'étiquette `#[tauri::command]`, puis ajouter son nom dans la liste `generate_handler![ ... ]` en bas du fichier.

**`src/index.html` (l'interface).** Un bloc d'en-tête au début du `<script>` donne l'architecture : le pont `invoke` vers le Rust, l'objet `store` (l'état complet), et `renderView()` (redessine l'écran). Le CSS est en haut du fichier, dans `<style>`. Les fonctions complexes (moteur SORA, recommandation de régime, couche données, cartes repliables) sont commentées sur place. La plupart des évolutions se font ici, sans toucher au Rust.

**Règle d'or.** Le Rust ne fait que le travail système (disque, réseau, mise à jour) ; toute la logique métier et l'affichage vivent dans le HTML/JS. Une nouvelle fonctionnalité qui ne demande pas d'accès système nouveau se code entièrement côté interface.

---

## 20. Évolutions envisagées

Bascule de **langue FR/EN** (petit drapeau / « FR » / « EN »). **Import PDF** enrichi des attestations télépilote. **Clé de licence** en complément du PIN. Suivi des **appels d'offres** enregistrés avec dates limites. Cartographie enrichie (couche restrictions drone préchargée).

---

## 21. Glossaire

- **Tauri** : cadre combinant interface web et coque native Rust.
- **`invoke`** : appel par lequel l'interface demande un service à la coque.
- **CI / GitHub Actions** : compilation automatique sur les serveurs de GitHub.
- **AcroForm / pdf-lib** : champs remplissables d'un PDF et la bibliothèque qui les remplit.
- **Leaflet / OpenStreetMap / Nominatim** : carte, fond cartographique et service de géocodage adresse ↔ coordonnées.
- **METAR / TAF** : observation et prévision météo aéronautiques d'un aérodrome.
- **MANEX** : manuel d'exploitation de l'exploitant d'aéronefs.
- **Régimes** : Ouverte (A1/A2/A3), STS, PDRA, SORA — voir sections 6 et 7.
- **SAIL / OSO** : niveau d'assurance de l'opération et objectifs de sécurité associés.
- **Updater / release / latest.json** : mécanisme de mise à jour automatique (section 15).

---

*Document généré pour M.G.I. — Maintenance Générale Industrielle. Aide à la préparation de vol ; ne se substitue pas à la réglementation applicable.*
