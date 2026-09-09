# Assistant Vol Drone — Dossier technique

**Éditeur :** M.G.I. — Maintenance Générale Industrielle
**Version du document :** 1.0 (couvre l'application jusqu'au commit `8b29061`)
**Application :** Assistant Vol Drone — outil de préparation de vol drone (référentiel, SORA 2.5, formulaires officiels)
**Public visé :** l'utilisateur-mainteneur de l'application (vous), pour comprendre, exploiter, faire évoluer et dépanner l'outil.

---

## 1. À quoi sert l'application

L'Assistant Vol Drone est une application de bureau qui aide à préparer un vol drone professionnel du début à la fin :

- il conserve une fois pour toutes les informations de l'**exploitant** (M.G.I.) et des **télépilotes** ;
- il gère des **dossiers de vol**, une fiche par mission, chacune avec son appareil et son analyse de risque ;
- il déroule la méthode **SORA 2.5** (le calcul de risque au sol iGRC/GRC, de risque air ARC, du niveau SAIL et des objectifs de sécurité OSO) ;
- il embarque une **base de drones DJI** qui pré-remplit les caractéristiques techniques à la sélection d'un modèle ;
- il **pré-remplit les formulaires officiels** (Cerfa 15476*04, dérogation préfectorale R5-UAS-DEROG, lettre de demande d'AOT).

L'application fonctionne **entièrement hors ligne**. Toutes les données restent sur le poste. L'accès est protégé par un **code PIN**.

Elle ne remplace pas la réglementation ni le jugement du télépilote : c'est une aide à la saisie et au calcul, à vérifier avant tout dépôt officiel.

---

## 2. Sur quoi l'application est construite

L'application est un logiciel **Tauri**. Il faut retenir une idée simple : une application Tauri est faite de deux moitiés qui coopèrent.

**La coque (back-end), écrite en Rust.** C'est le programme natif Windows/Linux. Il ouvre la fenêtre, lit et écrit les fichiers sur le disque, calcule l'empreinte du code PIN, et détient les gabarits PDF. Le Rust est compilé en un exécutable ; c'est lui qui a le droit de toucher au disque et au système.

**L'interface (front-end), écrite en HTML/CSS/JavaScript.** C'est tout ce que vous voyez : la barre latérale, les onglets, les formulaires, les tableaux SORA. Elle tourne dans une vue web intégrée à la fenêtre (WebView2 sur Windows, WebKitGTK sur Linux). Elle ne touche jamais directement au disque : quand elle a besoin d'enregistrer ou de lire, elle **demande à la coque Rust** via un appel appelé `invoke`.

Pourquoi ce choix plutôt qu'une « vraie » application web hébergée sur un serveur ? Parce que vous vouliez un outil qui s'installe par clé USB, marche sans Internet, et garde les données sur le poste. Tauri donne exactement cela : la légèreté d'une interface web, la robustesse et l'accès disque d'un logiciel natif, le tout dans un seul exécutable de quelques mégaoctets.

Le pont entre les deux moitiés : l'interface appelle `invoke('nom_de_commande', {arguments})`, et la coque Rust répond. C'est le seul canal. Tout ce que l'interface ne peut pas faire seule (écrire un PDF, lire le code PIN enregistré) passe par une commande Rust.

---

## 3. Arborescence du projet

```
assistant-vol/
├─ src/                         INTERFACE (front-end)
│  ├─ index.html                toute l'application web : écran, données, calculs
│  └─ vendor/
│     └─ pdf-lib.min.js         bibliothèque de remplissage PDF (récupérée au build)
│
├─ src-tauri/                   COQUE (back-end Rust)
│  ├─ src/
│  │  ├─ lib.rs                 les commandes Rust (PIN, données, formulaires…)
│  │  └─ main.rs                point d'entrée : lance l'application
│  ├─ forms/
│  │  ├─ cerfa_15476-04.pdf     gabarit officiel vierge (embarqué)
│  │  └─ form_r5-uas-derog_v4.pdf
│  ├─ icons/                    icônes de l'application
│  ├─ capabilities/default.json permissions accordées à l'interface
│  ├─ tauri.conf.json           configuration de l'application (nom, fenêtre, bundle)
│  ├─ Cargo.toml                dépendances Rust
│  └─ build.rs                  script de compilation Tauri
│
├─ .github/workflows/build.yml  compilation automatique Windows + Linux (CI)
├─ README.md
└─ .gitignore
```

Le fichier de loin le plus important pour vous est `src/index.html` : il contient **toute** l'interface et toute la logique métier (le calcul SORA, la base DJI, les formulaires). Le Rust, lui, est court et stable ; on y touche seulement pour ajouter une nouvelle capacité système (par exemple, plus tard, une clé de licence).

---

## 4. La coque Rust (`src-tauri/src/lib.rs`)

Le Rust expose **sept commandes** que l'interface peut appeler. Chacune est une fonction marquée `#[tauri::command]`.

| Commande | Rôle |
|---|---|
| `pin_status()` | Renvoie vrai si un code PIN a déjà été défini sur ce poste. |
| `set_pin(pin)` | Enregistre (ou remplace) le code PIN. |
| `verify_pin(pin)` | Vérifie un code saisi. |
| `data_get()` | Renvoie toutes les données de l'application (JSON). |
| `data_set(json)` | Enregistre toutes les données de l'application. |
| `form_template(name)` | Renvoie un gabarit PDF officiel (encodé en base64). |
| `export_file(subdir, filename, data_b64)` | Écrit un fichier généré (PDF, lettre) dans `Documents\Assistant Vol Drone\`. |

**Où sont stockées les données ?** Dans le dossier de configuration de l'application, propre à chaque poste et à chaque utilisateur Windows :

- le code PIN : dans un fichier `pin.hash` ;
- toutes vos saisies (exploitant, télépilotes, dossiers de vol) : dans un fichier `donnees.json`.

Ces fichiers ne sont **jamais** supprimés par une réinstallation de l'application : ils vivent à côté du programme, pas dedans. C'est pourquoi, après une mise à jour, votre PIN et vos dossiers sont toujours là.

**Le code PIN n'est pas stocké en clair.** La fonction `hash_pin` calcule une empreinte SHA-256 du code (avec un « sel » fixe). Seule cette empreinte est écrite sur le disque. À la saisie, on recalcule l'empreinte et on la compare. Quelqu'un qui ouvrirait le fichier `pin.hash` ne verrait pas votre code, seulement une suite de caractères inexploitable. C'est un verrou local mono-poste, suffisant pour empêcher un accès occasionnel ; ce n'est pas un coffre-fort chiffré (les données `donnees.json`, elles, sont en clair sur le poste).

**Les gabarits PDF sont dans l'exécutable.** Les deux formulaires vierges (`cerfa_15476-04.pdf`, `form_r5-uas-derog_v4.pdf`) sont incorporés au binaire à la compilation par l'instruction `include_bytes!`. Conséquence : ils sont toujours disponibles, sans Internet, sans fichier externe à installer. La commande `form_template` les ressort à la demande.

**L'écriture des fichiers générés.** `export_file` reçoit le contenu d'un PDF ou d'une lettre déjà rempli par l'interface, le décode, et l'écrit dans `Documents\Assistant Vol Drone\<nom de la mission>\`. Le nom de dossier est « nettoyé » des caractères interdits par Windows. La fonction renvoie le chemin complet, que l'interface fait ensuite ouvrir dans le lecteur PDF ou le navigateur par défaut.

**Le lancement.** La fonction `run()` construit l'application, branche le plugin `opener` (qui sait ouvrir une URL ou un fichier avec l'application par défaut du système), déclare la liste des sept commandes, et démarre. Le fichier `main.rs` ne fait qu'appeler `run()`.

---

## 5. L'interface (`src/index.html`)

Tout est dans un seul fichier, volontairement : pas de serveur, pas de dépendance externe (sauf pdf-lib), facile à embarquer et à lire. Le fichier se lit en trois couches.

### 5.1 Les données

L'état complet de l'application tient dans un objet JavaScript appelé `store`, de cette forme :

```
store = {
  exploitant : { raison, siret, numUAS, adresse, cp, ville, ... },
  pilotes    : [ { id, prenom, nom, tel, mail, mentions, ... }, ... ],
  referent   : "id du télépilote par défaut",
  dossiers   : [ dossier, dossier, ... ],
  currentId  : "id du dossier actuellement ouvert"
}
```

Un **dossier** (une mission) contient son intitulé, son lieu, ses dates, l'appareil utilisé, l'analyse SORA (`grc` et `arc`) et les paramètres des formulaires (`forms`).

À chaque modification d'un champ, l'interface met à jour `store` puis appelle `scheduleSave()`, qui attend une fraction de seconde (pour ne pas écrire à chaque frappe) puis envoie tout le `store` à la commande Rust `data_set`. Au démarrage, `loadStore()` lit le JSON via `data_get`. Si le poste est vierge, l'application se pré-remplit avec les coordonnées de M.G.I. (fonction `seededStore`).

Une fonction importante est `normalize()` : elle garantit qu'un `store` lu depuis le disque, même ancien ou incomplet, est ramené à la forme attendue (tous les champs présents). C'est elle qui assure la **compatibilité ascendante** : quand une version ajoute un champ, les anciens dossiers se complètent automatiquement avec une valeur par défaut. Elle gère aussi la migration de l'ancien format (un SORA unique) vers le nouveau (une liste de dossiers).

### 5.2 La navigation et le rendu

L'interface n'utilise aucun framework (pas de React, pas de Vue) : uniquement du JavaScript natif. Le principe est simple et se répète partout : une fonction `renderView()` regarde quel écran est demandé (`current`) et vide puis reconstruit la zone principale avec de petites fonctions utilitaires (`el` crée un élément, `card` crée une carte, `field` crée un champ de saisie relié à une donnée). Chaque champ, quand on le modifie, écrit dans `store` et déclenche la sauvegarde. Ouvrir un dossier bascule sur un écran de détail à onglets internes : *Mission & appareil*, *Risque sol*, *Risque air*, *SAIL & OSO*, *Formulaires*.

Ce style (tout redessiner à chaque changement) est volontairement simple : il n'y a pas d'état caché, ce qui est à l'écran est toujours le reflet exact de `store`.

### 5.3 La logique métier

Trois blocs de connaissance sont codés en dur dans l'interface : les **tables SORA 2.5**, la **base DJI**, et les **correspondances de champs des formulaires**. Ils sont détaillés dans les sections suivantes.

---

## 6. Le moteur de calcul SORA 2.5

C'est le cœur réglementaire de l'outil. Le calcul est fait par la fonction `computeSora(dossier)`, qui recalcule tout à chaque affichage à partir des saisies. Il suit fidèlement la méthode du guide DGAC (SORA 2.5).

### 6.1 Le risque au sol (GRC)

**iGRC (risque au sol intrinsèque).** On croise deux entrées : la « colonne » de l'appareil et la « ligne » de densité de population.

La colonne est choisie sur la dimension caractéristique **et** la vitesse max, en retenant la **plus pénalisante** des deux (c'est la règle conservatrice de SORA). Les seuils de colonnes sont 1 m / 3 m / 8 m / 20 m / 40 m en dimension, et 25 / 35 / 75 / 120 / 200 m/s en vitesse.

La ligne est la densité de population survolée, de « zone contrôlée au sol » (tiers exclus) jusqu'à « rassemblement de personnes ».

La table iGRC (valeurs 1 à 10) est celle du guide. Exemple : un drone de 2 m à 20 m/s au-dessus d'une zone à moins de 50 hab/km² donne iGRC 4.

**Note 1 (mini-drones).** Un drone de moins de 250 g **et** à 25 m/s ou moins donne un iGRC de 1 (sauf survol de rassemblement). L'application coche ce cas automatiquement quand vous sélectionnez un Mini DJI.

**GRC final.** On applique les atténuations à l'iGRC :
- **M1(A)** refuge/sheltering (0, −1 ou −2),
- **M1(B)** restrictions opérationnelles (0, −1 ou −2),
- **M1(C)** observation au sol VLOS (0 ou −1),
- **M2** réduction des effets de l'impact au sol (0, −1 ou −2).

Le GRC final est l'iGRC augmenté de ces réductions (négatives), sans jamais descendre sous un **plancher** (le GRC minimal de la colonne, celui de la zone contrôlée). L'application signale aussi le cas de cumul déconseillé M1(A) moyenne + M1(B).

### 6.2 Le risque air (ARC)

L'ARC initial est déterminé par un **arbre de décision** (fonction `computeARC`) qui pose, dans l'ordre, les questions du guide : espace atypique ? au-dessus du FL600 ? zone aéroportuaire (et classe A-D) ? au-dessus de 500 ft ? zone ADS-B/TMZ ? espace contrôlé ? zone urbaine ? Le résultat va de **ARC-a** (le plus faible) à **ARC-d**.

L'étape 5 permet de retenir un **ARC final** réduit (avec justification). L'étape 6 en déduit le niveau d'atténuation tactique requis (de « aucun » à « haut »).

### 6.3 SAIL et OSO

Le **SAIL** (I à VI) résulte du croisement GRC final × ARC final, via la matrice du guide. Un GRC supérieur à 7 bascule en **catégorie certifiée** (hors périmètre du SORA spécifique standard), ce que l'application indique.

Le SAIL fixe ensuite, pour chacun des **OSO** (objectifs de sécurité opérationnels), le niveau de robustesse exigé : Faible (L), Moyen (M) ou Haut (H), ou non requis. L'application affiche la liste des OSO applicables avec leur niveau, classés par catégorie (Technique, Procédures, Équipage).

Le moteur a été **vérifié** sur plusieurs cas connus (FlyCart 30 → SAIL IV, Matrice 350 → iGRC 2, Mini 4 Pro → iGRC 1), cohérents avec les tables du guide.

---

## 7. La base de référence DJI

La constante `DJI_DB` liste 17 modèles (Mini, Air, Mavic 3, Mavic 3 Enterprise, Matrice 30/300/350, FlyCart 30/100, Agras T40/T50). Pour chacun : la dimension caractéristique (m), la vitesse horizontale max (m/s) et la masse au décollage (g). Ces valeurs proviennent des **fiches techniques officielles du constructeur**.

Quand vous choisissez un modèle dans un dossier, la fonction `applyModel` recopie ces trois valeurs dans le dossier (elles alimentent directement le calcul GRC), pose la marque et le modèle, coche automatiquement la case « moins de 250 g » si le modèle le justifie, et vous laisse **seulement le numéro de série à saisir**. Une option « saisie manuelle » reste disponible pour tout autre appareil.

Pour ajouter un modèle : il suffit d'ajouter une ligne dans `DJI_DB` (clé, catégorie, modèle, dimension, vitesse, masse). Aucune autre modification n'est nécessaire, le sélecteur se met à jour tout seul.

---

## 8. Le module Formulaires

Chaque dossier a un onglet *Formulaires* qui produit trois documents pré-remplis à partir de l'exploitant, du télépilote référent et de la mission.

**Cerfa 15476*04 et dérogation R5-UAS-DEROG** sont de vrais PDF à champs (AcroForm). Le remplissage se fait ainsi :
1. l'interface demande le gabarit vierge à la coque Rust (`form_template`) ;
2. la bibliothèque **pdf-lib** ouvre le PDF, écrit chaque champ d'après une table de correspondance (`cerfaMapping`, `derogMapping`) et coche les cases voulues ;
3. le PDF rempli est renvoyé à la coque Rust (`export_file`), qui l'écrit dans `Documents\Assistant Vol Drone\<mission>\` ;
4. le fichier s'ouvre automatiquement dans votre lecteur PDF.

Les tables de correspondance associent un nom de champ du PDF (par exemple `Télépilote 1Nom`, ou le générique `Zone de texte 1`) à une donnée. Ces noms ont été **relevés et vérifiés** directement dans les formulaires officiels (191 champs pour le Cerfa, 48 pour la dérogation).

**AOT (occupation du domaine public).** Le Cerfa 14023*01 (voirie) n'étant pas un formulaire interactif et visant les travaux routiers, l'application génère à la place une **lettre de demande d'AOT** adaptée au drone (fonction `aotHTML`), reprenant l'exploitant, le site, les dates, la hauteur et l'objet. Elle s'ouvre dans le navigateur : Fichier → Imprimer → « Enregistrer au format PDF ».

Dans tous les cas, le document reste **à compléter et à signer** (horaires détaillés, espaces aériens pénétrés, justifications, lieu et date). L'assistant fait gagner la saisie répétitive, pas la relecture.

**Sur pdf-lib.** La bibliothèque de remplissage PDF n'est pas incluse dans le dépôt : elle est **téléchargée pendant la compilation** par la CI (les serveurs de GitHub ont Internet) puis intégrée à l'application. C'est le seul composant récupéré au build ; les gabarits, eux, sont dans le code.

---

## 9. Sécurité et données

L'accès est verrouillé par un **code PIN** (au moins 4 chiffres), stocké sous forme d'empreinte SHA-256, jamais en clair. Le verrou évoluera vers une **clé de licence** si le besoin se confirme : l'architecture est prête pour cela (il suffira d'ajouter des commandes Rust de validation, sans toucher au reste).

Toutes vos données restent **sur le poste**, dans le dossier de configuration de l'application. Rien n'est envoyé sur Internet — l'application n'ouvre aucune connexion. Les documents générés vont dans `Documents\Assistant Vol Drone\`.

Point d'attention : le fichier `donnees.json` est en clair. Le PIN protège l'ouverture de l'application, pas le fichier lui-même. Pour un poste partagé, comptez sur la session Windows et, à terme, sur le chiffrement du disque.

---

## 10. Installation, compilation et mise à jour

### 10.1 La chaîne de compilation (CI)

Le code vit sur GitHub (`paul-jnn/assistant-vol-drone`). À chaque envoi de code (`git push`), **GitHub Actions** compile automatiquement l'application, en parallèle, pour **Windows et Linux**, sur les serveurs de GitHub. C'est le fichier `.github/workflows/build.yml` qui décrit cette recette : installer Rust, récupérer pdf-lib, puis lancer la compilation Tauri, et publier les installateurs.

Vous n'avez donc **rien à compiler vous-même**. Les installateurs prêts à l'emploi (`.msi`/`.exe` pour Windows, `.deb`/`.AppImage` pour Linux) sont téléchargeables dans l'onglet **Actions** du dépôt, en bas de chaque exécution réussie, dans la section *Artifacts*.

### 10.2 Cycle de travail type

1. on modifie le code (l'interface, le plus souvent) ;
2. `git push` envoie le code sur GitHub ;
3. la CI compile (5 à 10 minutes) ;
4. vous téléchargez l'installateur de la dernière exécution verte ;
5. vous fermez l'application, relancez l'installateur : il remplace la version installée.

Vos données et votre PIN sont conservés d'une version à l'autre.

### 10.3 Compilation locale (facultatif)

Pour compiler sur votre poste (utile seulement pour tester une modification sans passer par la CI), il faut Rust, les outils de build Visual C++, et la commande `cargo tauri build`. En pratique, la CI suffit et évite d'installer cet environnement.

### 10.4 Partage par clé USB

L'installateur produit est un fichier autonome : vous pouvez le copier sur une clé USB et l'installer sur un autre poste, sans Internet. L'application fonctionnera hors ligne ; chaque poste aura son propre code PIN et ses propres données.

---

## 11. Dépannage

**Un lien ne s'ouvre pas.** Vérifié et corrigé : les liens passent par le module d'ouverture de Tauri (une simple balise de lien ne suffit pas dans une fenêtre d'application).

**Un formulaire ne se génère pas / message « Bibliothèque PDF non disponible ».** L'application installée n'a pas embarqué pdf-lib au build. Reprenez la dernière version compilée par la CI.

**Le PIN est oublié.** Il n'y a pas de récupération (c'est le principe d'une empreinte). Il faut supprimer le fichier `pin.hash` dans le dossier de configuration de l'application ; l'application redemandera alors d'en créer un. Attention, cela n'efface pas les données (`donnees.json` reste).

**La compilation échoue sur GitHub.** Ouvrez l'exécution en échec dans l'onglet Actions : le journal indique l'étape fautive (souvent une dépendance ou une faute de frappe dans le code). Corrigez, `git push`, la CI relance.

**Après réinstallation, les données ont disparu.** Cela ne devrait pas arriver : les données sont hors du programme. Vérifiez que vous n'avez pas changé d'utilisateur Windows.

---

## 12. Évolutions prévues et pistes

- **Import PDF** des attestations télépilote pour pré-remplir automatiquement les fiches (présent dans l'ancienne version web, à ré-intégrer en version hors ligne).
- **Clé de licence** en remplacement/complément du PIN.
- **Mise à jour automatique** (l'application vérifie GitHub au démarrage et se met à jour seule), pour ne plus réinstaller à la main.
- **Export d'un dossier SORA complet en PDF** (au-delà des formulaires).
- **Cartographie** : coordonnées du site et liens directs vers Geoportail / restrictions drone.

---

## 13. Glossaire express

- **Tauri** : cadre logiciel qui combine une interface web et une coque native (Rust). Produit une application de bureau légère.
- **Rust** : langage du back-end natif (fenêtre, disque, sécurité).
- **WebView2 / WebKitGTK** : le composant système qui affiche l'interface web dans la fenêtre (Windows / Linux).
- **`invoke`** : l'appel par lequel l'interface demande un service à la coque Rust.
- **CI / GitHub Actions** : la compilation automatique sur les serveurs de GitHub à chaque `git push`.
- **AcroForm** : les champs remplissables d'un PDF.
- **pdf-lib** : la bibliothèque qui remplit ces champs.
- **SORA / iGRC / GRC / ARC / SAIL / OSO** : la méthode d'analyse de risque drone et ses indicateurs (voir section 6).

---

*Document généré pour M.G.I. — Maintenance Générale Industrielle. Aide à la préparation de vol ; ne se substitue pas à la réglementation applicable.*
