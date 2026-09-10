// =============================================================================
//  Assistant Vol Drone — COQUE NATIVE (back-end Rust)
//  Fichier : src-tauri/src/lib.rs
// -----------------------------------------------------------------------------
//  À QUOI SERT CE FICHIER
//  L'application a deux moitiés :
//    - l'INTERFACE, écrite en HTML/CSS/JavaScript (src/index.html) : tout ce que
//      vous voyez à l'écran ;
//    - la COQUE, ce fichier Rust : la partie qui a le droit de toucher au disque,
//      au réseau et au système. L'interface ne peut RIEN faire de tout ça
//      directement ; elle « demande » à la coque via un appel nommé `invoke`.
//
//  COMMENT LIRE CE FICHIER SI VOUS NE CONNAISSEZ PAS RUST (mais un peu Python)
//    - `fn nom(...) -> Type { ... }`  = `def nom(...):` en Python. Le `-> Type`
//      annonce ce que la fonction renvoie.
//    - `let x = ...;`                 = affectation. Chaque instruction finit par `;`.
//    - `&`  devant un type/argument   = « emprunt » : on prête la valeur au lieu de
//      la copier (détail de performance/sécurité propre à Rust ; à ignorer pour
//      comprendre la logique).
//    - `Result<Ok, Err>`             = une valeur qui est SOIT un succès `Ok(...)`
//      SOIT une erreur `Err(...)`. C'est le type de retour des opérations qui
//      peuvent échouer (lire un fichier, décoder du base64…).
//    - `?`  à la fin d'une ligne      = « si erreur, arrête et renvoie l'erreur ;
//      sinon continue ». C'est le raccourci Rust du try/except.
//    - `match valeur { motif => ... }`= un `switch` / série de `if` sur les cas
//      possibles d'une valeur.
//    - `.map_err(|e| e.to_string())`  = transforme une erreur technique en simple
//      texte, pour la renvoyer proprement à l'interface.
//    - `#[tauri::command]`            = ÉTIQUETTE posée juste au-dessus d'une
//      fonction pour dire « cette fonction est appelable depuis l'interface par
//      invoke('nom_de_la_fonction', {arguments}) ». C'est le pont entre les deux
//      moitiés. Toute fonction sans cette étiquette est un utilitaire interne.
//
//  RÈGLE DE SÉCURITÉ SUIVIE ICI : la coque n'expose que le strict nécessaire.
//  Le réseau est limité au seul serveur météo ; les données restent sur le poste.
// =============================================================================

// --- Imports : on « apporte » des outils fournis par des bibliothèques ---------
use base64::Engine;   // pour encoder/décoder le base64 (façon de transporter des
                      // octets bruts — un PDF — sous forme de simple texte).
use tauri::Manager;   // donne accès aux chemins standard de l'app (dossier de
                      // configuration, Documents…) via `app.path()`.

// --- Gabarits PDF officiels EMBARQUÉS dans le programme -------------------------
// `include_bytes!` recopie le contenu du fichier PDF DANS le binaire au moment de
// la compilation. Résultat : ces formulaires sont toujours disponibles, même hors
// ligne, sans fichier externe à installer. `&[u8]` = « une suite d'octets ».
const TPL_CERFA: &[u8] = include_bytes!("../forms/cerfa_15476-04.pdf");
const TPL_DEROG: &[u8] = include_bytes!("../forms/form_r5-uas-derog_v4.pdf");

// -----------------------------------------------------------------------------
//  EMPLACEMENTS DE STOCKAGE (utilitaires internes, pas exposés à l'interface)
// -----------------------------------------------------------------------------

// Renvoie le dossier de CONFIGURATION de l'application, propre à chaque poste
// (ex. sous Windows : C:\Users\<vous>\AppData\Roaming\fr.mgi.assistantvoldrone).
// C'est ici que vivent le PIN, les données et les justificatifs — HORS du
// programme, donc ils survivent aux mises à jour et réinstallations.
fn config_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app.path().app_config_dir().expect("dossier de configuration introuvable");
    std::fs::create_dir_all(&dir).ok(); // crée le dossier s'il manque ; `.ok()` = « ignore l'échec »
    dir
}
// Chemins des deux fichiers de base, construits à partir du dossier de config.
// `.join("x")` = ajoute « /x » au chemin (équivalent de os.path.join en Python).
fn pin_file(app: &tauri::AppHandle) -> std::path::PathBuf { config_dir(app).join("pin.hash") }
fn data_file(app: &tauri::AppHandle) -> std::path::PathBuf { config_dir(app).join("donnees.json") }

// -----------------------------------------------------------------------------
//  VERROU PAR CODE PIN
// -----------------------------------------------------------------------------

// Calcule l'EMPREINTE (hash SHA-256) d'un code PIN. Une empreinte est un résumé
// irréversible : on peut vérifier qu'un PIN correspond, mais on ne peut pas
// retrouver le PIN à partir de l'empreinte. Le fichier pin.hash ne contient donc
// JAMAIS le code en clair. Le `b"..."` ajouté est un « sel » : un préfixe fixe qui
// renforce l'empreinte.
fn hash_pin(pin: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"mgi-assistant-vol-drone-v1"); // sel
    h.update(pin.as_bytes());                // + le PIN saisi
    format!("{:x}", h.finalize())            // renvoie l'empreinte en hexadécimal
}

// L'interface demande : « un PIN a-t-il déjà été défini sur ce poste ? »
// (true = oui, on affiche l'écran de saisie ; false = non, on propose d'en créer un).
#[tauri::command]
fn pin_status(app: tauri::AppHandle) -> bool { pin_file(&app).exists() }

// Enregistre un nouveau PIN (on stocke seulement son empreinte). Refuse un code de
// moins de 4 chiffres. Renvoie `Ok` (rien) en cas de succès, ou `Err(texte)`.
#[tauri::command]
fn set_pin(app: tauri::AppHandle, pin: String) -> Result<(), String> {
    if pin.len() < 4 { return Err("Le code PIN doit comporter au moins 4 chiffres.".into()); }
    std::fs::write(pin_file(&app), hash_pin(&pin)).map_err(|e| e.to_string())
}

// Vérifie un PIN saisi : on recalcule son empreinte et on la compare à celle
// stockée. Renvoie true si elles correspondent. `match` gère les deux cas de la
// lecture du fichier : réussie (`Ok`) ou impossible (`Err`, => false).
#[tauri::command]
fn verify_pin(app: tauri::AppHandle, pin: String) -> bool {
    match std::fs::read_to_string(pin_file(&app)) {
        Ok(stored) => stored.trim() == hash_pin(&pin),
        Err(_) => false,
    }
}

// -----------------------------------------------------------------------------
//  DONNÉES DE L'APPLICATION (le gros fichier donnees.json)
// -----------------------------------------------------------------------------

// Renvoie à l'interface tout le contenu de donnees.json (exploitant, télépilotes,
// dossiers…). Si le fichier n'existe pas encore (première utilisation), renvoie un
// objet JSON vide "{}" — d'où le démarrage « vierge » d'une installation neuve.
#[tauri::command]
fn data_get(app: tauri::AppHandle) -> String {
    std::fs::read_to_string(data_file(&app)).unwrap_or_else(|_| "{}".to_string())
}

// Enregistre les données envoyées par l'interface. ÉCRITURE ATOMIQUE en 2 temps
// pour ne jamais corrompre le fichier si l'app est coupée en plein enregistrement :
//   1) on vérifie que le texte reçu est bien du JSON valide ;
//   2) on écrit d'abord dans un fichier temporaire « .tmp » ;
//   3) on RENOMME ce temporaire par-dessus le vrai fichier (opération instantanée).
// Ainsi donnees.json est toujours soit l'ancienne version complète, soit la
// nouvelle version complète — jamais un mélange à moitié écrit.
#[tauri::command]
fn data_set(app: tauri::AppHandle, json: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("JSON invalide : {e}"))?;
    let path = data_file(&app);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
//  GABARITS DE FORMULAIRES OFFICIELS
// -----------------------------------------------------------------------------

// Renvoie un des PDF officiels embarqués, encodé en base64 (texte) pour le
// transporter jusqu'à l'interface, qui le remplira avec pdf-lib. `match` choisit
// le bon gabarit selon le nom demandé ; tout autre nom => erreur.
#[tauri::command]
fn form_template(name: String) -> Result<String, String> {
    let bytes: &[u8] = match name.as_str() {
        "cerfa" => TPL_CERFA,
        "derog" => TPL_DEROG,
        _ => return Err("gabarit inconnu".into()),
    };
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

// -----------------------------------------------------------------------------
//  EXPORT DE FICHIERS GÉNÉRÉS (PDF, dossiers, lettres)
// -----------------------------------------------------------------------------

// Nettoie un texte pour en faire un nom de fichier valide : remplace les caractères
// interdits par Windows ( \ / : * ? " < > | ) par « _ », enlève espaces et points
// en trop. Si le résultat est vide, renvoie « Sans_titre ». Évite les noms de
// fichiers cassés à partir d'un intitulé de mission libre.
fn sanitize(s: &str) -> String {
    let s: String = s.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "Sans_titre".into() } else { s }
}

// Dossier racine des exports, visible par l'utilisateur : « Documents\Assistant Vol
// Drone\ ». Si le dossier Documents est introuvable, on se rabat sur le dossier
// personnel, puis en dernier recours sur le dossier de config. Le `.or_else(...)`
// enchaîne ces solutions de repli.
fn export_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    let base = app.path().document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| config_dir(app));
    base.join("Assistant Vol Drone")
}

// Écrit un fichier généré par l'interface (reçu en base64) dans un sous-dossier de
// « Documents\Assistant Vol Drone\ », et renvoie le chemin complet du fichier créé
// (que l'interface peut ensuite ouvrir). Décode le base64, crée le sous-dossier au
// besoin, puis écrit les octets.
#[tauri::command]
fn export_file(app: tauri::AppHandle, subdir: String, filename: String, data_b64: String) -> Result<String, String> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_b64.as_bytes()).map_err(|e| format!("base64 invalide : {e}"))?;
    let dir = export_root(&app).join(sanitize(&subdir));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(sanitize(&filename));
    std::fs::write(&path, &data).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}

// -----------------------------------------------------------------------------
//  OUVRIR / ENREGISTRER via le système
// -----------------------------------------------------------------------------

// Ouvre un fichier avec l'application par défaut du système (ex. un PDF dans le
// lecteur PDF de Windows). Passe par le composant « opener » de Tauri. Non
// restreint à un dossier précis : c'est pourquoi l'interface passe par CETTE
// commande plutôt que par l'ouverture directe (qui, elle, est limitée à des
// dossiers autorisés et bloquait le dossier de config).
#[tauri::command]
fn open_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_path(path, None::<&str>).map_err(|e| e.to_string())
}

// Écrit des octets (reçus en base64) à un emplacement EXACT choisi par
// l'utilisateur. C'est ce qui alimente la boîte « Enregistrer sous… » : l'interface
// ouvre le sélecteur de fichier, récupère le chemin choisi (`dest`) et le confie
// ici. Sépare volontairement « exporter » (chemin imposé, ci-dessus) et
// « enregistrer sous » (chemin libre, ici).
#[tauri::command]
fn save_bytes(dest: String, data_b64: String) -> Result<(), String> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(data_b64.as_bytes()).map_err(|e| format!("base64 invalide : {e}"))?;
    std::fs::write(&dest, &data).map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
//  MÉTÉO AÉRONAUTIQUE (seul accès réseau de toute l'application)
// -----------------------------------------------------------------------------

// Va chercher le METAR/TAF sur aviationweather.gov et renvoie le texte brut.
// `async` = fonction qui peut « attendre » une réponse réseau sans figer l'app.
// GARDE-FOU DE SÉCURITÉ : toute URL qui ne commence pas exactement par l'adresse
// du serveur météo est REFUSÉE. L'application ne peut donc pas être détournée pour
// contacter un autre site — elle n'est pas un relais réseau ouvert.
#[tauri::command]
async fn http_get(url: String) -> Result<String, String> {
    if !url.starts_with("https://aviationweather.gov/") {
        return Err("URL non autorisée".into());
    }
    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "AssistantVolDrone/1.0")
        .send().await.map_err(|e| e.to_string())?; // `.await` = « attends la réponse »
    resp.text().await.map_err(|e| e.to_string())
}

// -----------------------------------------------------------------------------
//  MISE À JOUR AUTOMATIQUE
// -----------------------------------------------------------------------------

// Vérifie s'il existe une version plus récente publiée sur GitHub. Renvoie le
// numéro de la nouvelle version si oui (`Some("0.2.7")`), ou `None` si l'app est à
// jour ou hors ligne. Volontairement SILENCIEUX : toute erreur (pas de réseau…)
// est convertie en `None`, jamais en message bloquant.
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = match app.updater() { Ok(u) => u, Err(_) => return Ok(None) };
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(update.version.clone())),
        Ok(None) => Ok(None),
        Err(_) => Ok(None),
    }
}

// Télécharge et installe la mise à jour, puis redémarre l'application. Appelée
// quand l'utilisateur clique « Installer & redémarrer » sur la bannière. Les deux
// `|...| {}` sont des fonctions de suivi de progression laissées vides (on ne se
// sert pas de la barre de progression ici).
#[tauri::command]
async fn apply_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    if let Some(update) = updater.check().await.map_err(|e| e.to_string())? {
        update.download_and_install(|_c, _t| {}, || {}).await.map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}

// -----------------------------------------------------------------------------
//  JUSTIFICATIFS IMPORTÉS (MANEX, assurance, attestations…)
// -----------------------------------------------------------------------------

// Sous-dossier « documents/ » du dossier de config, où l'on range les pièces
// importées par l'utilisateur. Créé au besoin.
fn docs_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    let d = config_dir(app).join("documents");
    std::fs::create_dir_all(&d).ok();
    d
}

// Importe un document (reçu en base64) : le décode et l'écrit dans documents/.
// Renvoie le nom de fichier retenu (nettoyé).
#[tauri::command]
fn import_doc(app: tauri::AppHandle, filename: String, data_b64: String) -> Result<String, String> {
    let data = base64::engine::general_purpose::STANDARD.decode(data_b64.as_bytes()).map_err(|e| e.to_string())?;
    let name = sanitize(&filename);
    std::fs::write(docs_dir(&app).join(&name), &data).map_err(|e| e.to_string())?;
    Ok(name)
}

// Liste les documents importés, avec leur nom et leur taille en octets. Construit
// une liste d'objets JSON `{"name": ..., "size": ...}` que l'interface affichera.
// On ne garde que les vrais fichiers, et on trie par nom à la fin.
#[tauri::command]
fn list_docs(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let mut out = vec![]; // `vec![]` = liste vide, comme [] en Python. `mut` = modifiable.
    if let Ok(rd) = std::fs::read_dir(docs_dir(&app)) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() {
                if md.is_file() {
                    out.push(serde_json::json!({"name": e.file_name().to_string_lossy(), "size": md.len()}));
                }
            }
        }
    }
    out.sort_by(|a, b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
    out
}

// Renvoie le chemin complet d'un document importé (pour l'ouvrir avec open_file).
// Erreur « introuvable » si le fichier n'existe pas.
#[tauri::command]
fn doc_path(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let p = docs_dir(&app).join(sanitize(&filename));
    if p.exists() { Ok(p.to_string_lossy().to_string()) } else { Err("introuvable".into()) }
}

// Renvoie le CONTENU d'un document importé en base64, pour l'afficher DANS
// l'application (lecteur PDF intégré) sans passer par un logiciel externe.
#[tauri::command]
fn read_doc(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let p = docs_dir(&app).join(sanitize(&filename));
    let data = std::fs::read(&p).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(data))
}

// Supprime un document importé.
#[tauri::command]
fn delete_doc(app: tauri::AppHandle, filename: String) -> Result<(), String> {
    std::fs::remove_file(docs_dir(&app).join(sanitize(&filename))).map_err(|e| e.to_string())
}

// Exporte (recopie) un document importé vers « Documents\Assistant Vol Drone\
// Justificatifs\ », emplacement visible par l'utilisateur. Renvoie le chemin créé.
#[tauri::command]
fn export_doc(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let name = sanitize(&filename);
    let dir = export_root(&app).join("Justificatifs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dst = dir.join(&name);
    std::fs::copy(docs_dir(&app).join(&name), &dst).map_err(|e| e.to_string())?;
    Ok(dst.to_string_lossy().to_string())
}

// =============================================================================
//  POINT DE DÉMARRAGE DE LA COQUE
// -----------------------------------------------------------------------------
//  `run()` est appelée par main.rs au lancement. Elle :
//    1) branche les COMPOSANTS (« plugins ») utilisés : opener (ouvrir fichiers/
//       URLs), updater (mise à jour auto), dialog (boîtes « Enregistrer sous ») ;
//    2) DÉCLARE la liste des commandes appelables depuis l'interface — toute
//       fonction #[tauri::command] doit être listée ici, sinon invoke ne la
//       trouve pas ;
//    3) démarre l'application (ouvre la fenêtre).
//  Pour AJOUTER une capacité côté Rust : écrire la fonction avec #[tauri::command],
//  puis ajouter son nom dans generate_handler![ ... ] ci-dessous.
// =============================================================================
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            pin_status, set_pin, verify_pin, data_get, data_set,
            form_template, export_file, open_file, save_bytes, http_get, check_update, apply_update,
            import_doc, list_docs, doc_path, read_doc, delete_doc, export_doc
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
