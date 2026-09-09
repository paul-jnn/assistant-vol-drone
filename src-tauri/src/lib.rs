use base64::Engine;
use tauri::Manager;

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

// Requête HTTP GET restreinte (météo aviationweather.gov). Renvoie le corps texte.
#[tauri::command]
async fn http_get(url: String) -> Result<String, String> {
    if !url.starts_with("https://aviationweather.gov/") {
        return Err("URL non autorisée".into());
    }
    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "AssistantVolDrone/1.0")
        .send().await.map_err(|e| e.to_string())?;
    resp.text().await.map_err(|e| e.to_string())
}

// Vérifie s'il existe une mise à jour ; renvoie la version, ou None (silencieux si indisponible).
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

// Télécharge et installe la mise à jour, puis redémarre l'application.
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


fn docs_dir(app: &tauri::AppHandle) -> std::path::PathBuf {
    let d = config_dir(app).join("documents");
    std::fs::create_dir_all(&d).ok();
    d
}

#[tauri::command]
fn import_doc(app: tauri::AppHandle, filename: String, data_b64: String) -> Result<String, String> {
    let data = base64::engine::general_purpose::STANDARD.decode(data_b64.as_bytes()).map_err(|e| e.to_string())?;
    let name = sanitize(&filename);
    std::fs::write(docs_dir(&app).join(&name), &data).map_err(|e| e.to_string())?;
    Ok(name)
}

#[tauri::command]
fn list_docs(app: tauri::AppHandle) -> Vec<serde_json::Value> {
    let mut out = vec![];
    if let Ok(rd) = std::fs::read_dir(docs_dir(&app)) {
        for e in rd.flatten() {
            if let Ok(md) = e.metadata() {
                if md.is_file() {
                    out.push(serde_json::json!({"name": e.file_name().to_string_lossy(), "size": md.len()}));
                }
            }
        }
    }
    out.sort_by(|a,b| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")));
    out
}

#[tauri::command]
fn doc_path(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let p = docs_dir(&app).join(sanitize(&filename));
    if p.exists() { Ok(p.to_string_lossy().to_string()) } else { Err("introuvable".into()) }
}

#[tauri::command]
fn delete_doc(app: tauri::AppHandle, filename: String) -> Result<(), String> {
    std::fs::remove_file(docs_dir(&app).join(sanitize(&filename))).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_doc(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    let name = sanitize(&filename);
    let dir = export_root(&app).join("Justificatifs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dst = dir.join(&name);
    std::fs::copy(docs_dir(&app).join(&name), &dst).map_err(|e| e.to_string())?;
    Ok(dst.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            pin_status, set_pin, verify_pin, data_get, data_set,
            form_template, export_file, http_get, check_update, apply_update,
            import_doc, list_docs, doc_path, delete_doc, export_doc
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de l'application Tauri");
}
