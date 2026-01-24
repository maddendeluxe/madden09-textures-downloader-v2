use crate::config::{REPO_NAME, REPO_OWNER};
use reqwest::Client;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Custom deserializer that accepts both strings and numbers, converting to string
fn string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected string or number")),
    }
}

/// Installer data from the mod repository
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerData {
    /// Minimum required version of this downloader app
    #[serde(deserialize_with = "string_or_number")]
    pub min_downloader_app_version: String,
    /// Total size of the texture pack (e.g., "8.5 GB" or just "22.5")
    #[serde(deserialize_with = "string_or_number")]
    pub total_size: String,
}

/// Result of fetching installer data
#[derive(Debug, Clone, Serialize)]
pub struct InstallerDataResult {
    pub data: Option<InstallerData>,
    pub error: Option<String>,
}

/// Get the app version from Cargo.toml/tauri.conf.json
#[tauri::command]
pub fn get_app_version(app_handle: tauri::AppHandle) -> String {
    app_handle
        .package_info()
        .version
        .to_string()
}

/// Fetch installer-data.json from the mod repository
#[tauri::command]
pub async fn fetch_installer_data() -> InstallerDataResult {
    let client = Client::new();
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/main/installer-data.json",
        REPO_OWNER, REPO_NAME
    );

    match client
        .get(&url)
        .header("User-Agent", "PS2-Textures-Downloader")
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
                return InstallerDataResult {
                    data: None,
                    error: Some(format!(
                        "Failed to fetch installer data: HTTP {}",
                        response.status()
                    )),
                };
            }

            match response.json::<InstallerData>().await {
                Ok(data) => InstallerDataResult {
                    data: Some(data),
                    error: None,
                },
                Err(e) => InstallerDataResult {
                    data: None,
                    error: Some(format!("Failed to parse installer data: {}", e)),
                },
            }
        }
        Err(e) => InstallerDataResult {
            data: None,
            error: Some(format!("Network error: {}", e)),
        },
    }
}

/// Compare two semver version strings
/// Returns: -1 if v1 < v2, 0 if equal, 1 if v1 > v2
#[tauri::command]
pub fn compare_versions(v1: String, v2: String) -> i32 {
    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };

    let v1_parts = parse_version(&v1);
    let v2_parts = parse_version(&v2);

    // Compare each part
    let max_len = v1_parts.len().max(v2_parts.len());
    for i in 0..max_len {
        let p1 = v1_parts.get(i).copied().unwrap_or(0);
        let p2 = v2_parts.get(i).copied().unwrap_or(0);

        if p1 < p2 {
            return -1;
        }
        if p1 > p2 {
            return 1;
        }
    }

    0 // Equal
}
