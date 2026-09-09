use base64::Engine;
use tauri::Manager;

// Gabarits officiels embarqués dans le binaire (100% hors ligne).
const TPL_CERFA: &[u8] = include_bytes!("../forms/cerfa_15476-04.pdf");
const TPL_DEROG: &[u8] = include_bytes!("../forms/form_r5-uas-derog_v4.pdf");

fn config_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app.path().app_config_dir().expect("dossier de configuration introuvable");
    std::fs::create_dir_all(&dir).ok();
    dir
}
fn pin_file(app: &tauri::AppHandle) -> std::path::PathBuf { config_dir(app).join("pin.hash") }
fn data_file(app: &tauri::AppHandle) -> std::path::PathBuf { config_dir(app).join("donnees.json") }

fn hash_pin(pin: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"mgi-assistant-vol-drone-v1");
    h.update(pin.as_bytes());
    format!("{:x}", h.finalize())
}

#[tauri::command]
fn pin_status(app: tauri::AppHandle) -> bool { pin_file(&app).exists() }

#[tauri::command]
fn set_pin(app: tauri::AppHandle, pin: String) -> Result<(), String> {
    if pin.len() < 4 { return Err("Le code PIN doit comporter au moins 4 chiffres.".into()); }
    std::fs::write(pin_file(&app), hash_pin(&pin)).map_err(|e| e.to_string())
}

#[tauri::command]
fn verify_pin(app: tauri::AppHandle, pin: String) -> bool {
    match std::fs::read_to_string(pin_file(&app)) {
        Ok(stored) => stored.trim() == hash_pin(&pin),
        Err(_) => false,
    }
}

#[tauri::command]
fn data_get(app: tauri::AppHandle) -> String {
    std::fs::read_to_string(data_file(&app)).unwrap_or_else(|_| "{}".to_string())
}

#[tauri::command]
fn data_set(app: tauri::AppHandle, json: String) -> Result<(), String> {
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("JSON invalide : {e}"))?;
    let path = data_file(&app);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// Renvoie un gabarit PDF officiel encodé en base64.
#[tauri::command]
fn form_template(name: String) -> Result<String, String> {
    let bytes: &[u8] = match name.as_str() {
        "cerfa" => TPL_CERFA,
        "derog" => TPL_DEROG,
        _ => return Err("gabarit inconnu".into()),
    };
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn sanitize(s: &str) -> String {
    let s: String = s.chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() { "Sans_titre".into() } else { s }
}

fn export_root(app: &tauri::AppHandle) -> std::path::PathBuf {
    let base = app.path().document_dir()
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| config_dir(app));
    base.join("Assistant Vol Drone")
}

// Écrit un fichier (PDF, HTML…) fourni en base64 dans Documents/Assistant Vol Drone/<sous-dossier>/.
// Renvoie le chemin complet du fichier écrit.
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            pin_status, set_pin, verify_pin, data_get, data_set, form_template, export_file
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
