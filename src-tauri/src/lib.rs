use tauri::Manager;

/// Dossier de configuration de l'application (créé si absent).
fn config_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("dossier de configuration introuvable");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// Fichier stockant l'empreinte du code PIN.
fn pin_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    config_dir(app).join("pin.hash")
}

/// Fichier de données de l'application (référentiel, dossiers, etc.).
fn data_file(app: &tauri::AppHandle) -> std::path::PathBuf {
    config_dir(app).join("donnees.json")
}

/// Empreinte SHA-256 du PIN, avec un sel fixe (suffisant pour un verrou local mono-poste).
fn hash_pin(pin: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"mgi-assistant-vol-drone-v1");
    h.update(pin.as_bytes());
    format!("{:x}", h.finalize())
}

/// Un PIN a-t-il déjà été défini sur ce poste ?
#[tauri::command]
fn pin_status(app: tauri::AppHandle) -> bool {
    pin_file(&app).exists()
}

/// Définit (ou remplace) le code PIN.
#[tauri::command]
fn set_pin(app: tauri::AppHandle, pin: String) -> Result<(), String> {
    if pin.len() < 4 {
        return Err("Le code PIN doit comporter au moins 4 chiffres.".into());
    }
    std::fs::write(pin_file(&app), hash_pin(&pin)).map_err(|e| e.to_string())
}

/// Vérifie un code PIN.
#[tauri::command]
fn verify_pin(app: tauri::AppHandle, pin: String) -> bool {
    match std::fs::read_to_string(pin_file(&app)) {
        Ok(stored) => stored.trim() == hash_pin(&pin),
        Err(_) => false,
    }
}

/// Lit l'ensemble des données applicatives (JSON brut). Renvoie "{}" si aucun fichier.
#[tauri::command]
fn data_get(app: tauri::AppHandle) -> String {
    std::fs::read_to_string(data_file(&app)).unwrap_or_else(|_| "{}".to_string())
}

/// Écrit l'ensemble des données applicatives (JSON brut), de façon atomique.
#[tauri::command]
fn data_set(app: tauri::AppHandle, json: String) -> Result<(), String> {
    // Validation minimale : le contenu doit être un JSON valide.
    serde_json::from_str::<serde_json::Value>(&json).map_err(|e| format!("JSON invalide : {e}"))?;
    let path = data_file(&app);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json.as_bytes()).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            pin_status, set_pin, verify_pin, data_get, data_set
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
