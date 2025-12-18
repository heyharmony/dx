// TODO: Fix clippy warnings for better code quality
#![allow(clippy::collapsible_if)] // TODO: Simplify nested if statements

use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct MenuItem {
    pub name: String,
    #[serde(default, alias = "description")]
    pub desc: Option<String>,
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub aliases: Option<Vec<String>>, // multiple aliases
    pub cmd: Option<String>,
    pub file: Option<String>,
    #[serde(default, alias = "children")]
    pub items: Vec<MenuItem>,
    #[serde(default)]
    #[allow(dead_code)]
    pub capture: Option<bool>,
    #[serde(default)]
    pub external: Option<bool>,
    #[serde(default)]
    pub enhanced_terminal: Option<bool>,
    #[serde(default)]
    pub form: Option<FormSpec>,
    #[serde(default)]
    pub plugin_list: bool,
}

#[derive(Debug, Deserialize)]
pub struct MenuConfig {
    #[serde(default)]
    pub items: Vec<MenuItem>,
}

// == NEW UNIFIED FORMAT ==
#[derive(Debug, Deserialize)]
pub struct DxFile {
    #[serde(default)]
    #[allow(dead_code)]
    pub config: Option<serde_json::Value>, // TODO: Implement proper config parsing instead of JSON Value
    #[serde(default)]
    pub menu: Vec<MenuItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FormSpec {
    #[serde(default)]
    pub title: Option<String>,
    pub fields: Vec<FormField>,
    #[serde(default)]
    pub submit: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FormField {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub r#type: Option<String>, // input | select
    #[serde(default)]
    pub options: Option<Vec<String>>, // for select
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub required: Option<bool>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub help: Option<String>,
}

#[derive(Debug)]
pub struct MenuState {
    pub items: Vec<MenuItem>,
    pub selected_index: usize,
    pub path: Vec<usize>,
}

#[must_use]
pub fn submenu_at<'a>(root: &'a [MenuItem], path: &[usize]) -> &'a [MenuItem] {
    let mut items = root;
    for &idx in path {
        if let Some(mi) = items.get(idx) {
            items = &mi.items;
        } else {
            break;
        }
    }
    items
}

#[must_use]
pub fn find_item_by_alias<'a>(root: &'a [MenuItem], alias: &str) -> Option<&'a MenuItem> {
    // First try explicit aliases (legacy behavior)
    for item in root {
        if let Some(a) = &item.alias {
            if a == alias {
                return Some(item);
            }
        }
        if let Some(list) = &item.aliases {
            if list.iter().any(|s| s == alias) {
                return Some(item);
            }
        }
        if let Some(found) = find_item_by_alias(&item.items, alias) {
            return Some(found);
        }
    }

    // Then try nested path-based aliases with colons
    if alias.contains(':') {
        return find_item_by_nested_path(root, alias);
    }

    None
}

fn find_item_by_nested_path<'a>(root: &'a [MenuItem], path: &str) -> Option<&'a MenuItem> {
    let parts: Vec<&str> = path.split(':').collect();
    if parts.is_empty() {
        return None;
    }

    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn find_nested<'a>(
        items: &'a [MenuItem],
        parts: &[&str],
        current_depth: usize,
    ) -> Option<&'a MenuItem> {
        if current_depth >= parts.len() {
            return None;
        }

        let target_part = parts[current_depth];

        for item in items {
            // Use explicit alias if provided, otherwise convert name (same logic as collect_aliases)
            let alias_part = if let Some(ref explicit_alias) = item.alias {
                explicit_alias.clone()
            } else {
                item.name
                    .to_lowercase()
                    .replace([' ', '-'], "_")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>()
            };

            if alias_part == target_part {
                // If this is the last part and item has cmd/file, return it
                if current_depth == parts.len() - 1 && (item.cmd.is_some() || item.file.is_some()) {
                    return Some(item);
                }
                // Otherwise recurse into children
                if !item.items.is_empty() {
                    if let Some(found) = find_nested(&item.items, parts, current_depth + 1) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    find_nested(root, &parts, 0)
}

#[must_use]
pub fn collect_aliases(root: &[MenuItem]) -> Vec<(String, String, Option<String>, Option<String>)> {
    let mut out: Vec<(String, String, Option<String>, Option<String>)> = Vec::new();

    // First collect explicit aliases (legacy behavior)
    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn walk_explicit(
        acc: &mut Vec<(String, String, Option<String>, Option<String>)>,
        items: &[MenuItem],
    ) {
        for it in items {
            if let Some(a) = &it.alias {
                acc.push((a.clone(), it.name.clone(), it.cmd.clone(), it.file.clone()));
            }
            if let Some(list) = &it.aliases {
                for a in list {
                    acc.push((a.clone(), it.name.clone(), it.cmd.clone(), it.file.clone()));
                }
            }
            if !it.items.is_empty() {
                walk_explicit(acc, &it.items);
            }
        }
    }

    // Then collect nested path-based aliases with colons
    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn walk_nested(
        acc: &mut Vec<(String, String, Option<String>, Option<String>)>,
        items: &[MenuItem],
        path: &[String],
    ) {
        for it in items {
            let mut current_path = path.to_owned();
            // Use explicit alias if provided, otherwise convert name to snake_case-like format
            let alias_part = if let Some(ref explicit_alias) = it.alias {
                explicit_alias.clone()
            } else {
                it.name
                    .to_lowercase()
                    .replace([' ', '-'], "_")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>()
            };
            current_path.push(alias_part);

            // If this item has a command or file, generate alias
            if (it.cmd.is_some() || it.file.is_some()) && !current_path.is_empty() {
                let nested_alias = current_path.join(":");
                // Only add if not already exists (avoid duplicates with explicit aliases)
                if !acc.iter().any(|(a, _, _, _)| a == &nested_alias) {
                    acc.push((
                        nested_alias,
                        it.name.clone(),
                        it.cmd.clone(),
                        it.file.clone(),
                    ));
                }
            }

            // Recurse into children
            if !it.items.is_empty() {
                walk_nested(acc, &it.items, &current_path);
            }
        }
    }

    walk_explicit(&mut out, root);
    walk_nested(&mut out, root, &Vec::new());
    out
}

/// Build terminal alias for a specific menu item given its path in the menu hierarchy  
#[must_use]
pub fn build_terminal_alias(root: &[MenuItem], menu_path: &[usize], item_index: usize) -> Option<String> {
    // Build path of alias parts from menu hierarchy
    let mut alias_parts: Vec<String> = Vec::new();
    let mut items = root;
    
    // Walk through menu path to build parent alias parts
    for &idx in menu_path {
        if let Some(mi) = items.get(idx) {
            let alias_part = if let Some(ref explicit_alias) = mi.alias {
                explicit_alias.clone()
            } else {
                mi.name
                    .to_lowercase()
                    .replace([' ', '-'], "_")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>()
            };
            alias_parts.push(alias_part);
            items = &mi.items;
        } else {
            return None;
        }
    }
    
    // Add final item alias part
    if let Some(final_item) = items.get(item_index) {
        if final_item.cmd.is_some() || final_item.file.is_some() {
            let final_alias_part = if let Some(ref explicit_alias) = final_item.alias {
                explicit_alias.clone()
            } else {
                final_item.name
                    .to_lowercase()
                    .replace([' ', '-'], "_")
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>()
            };
            alias_parts.push(final_alias_part);
            
            if !alias_parts.is_empty() {
                return Some(format!("dx {}", alias_parts.join(":")));
            }
        }
    }
    
    None
}

#[must_use]
pub fn collect_unaliased_commands(
    root: &[MenuItem],
) -> Vec<(String, Option<String>, Option<String>)> {
    let mut out: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn walk(acc: &mut Vec<(String, Option<String>, Option<String>)>, items: &[MenuItem]) {
        for it in items {
            let has_action = it.cmd.is_some() || it.file.is_some();
            let has_alias = it.alias.as_ref().is_some_and(|s| !s.is_empty())
                || it.aliases.as_ref().is_some_and(|v| !v.is_empty());
            if it.items.is_empty() && has_action && !has_alias {
                acc.push((it.name.clone(), it.cmd.clone(), it.file.clone()));
            }
            if !it.items.is_empty() {
                walk(acc, &it.items);
            }
        }
    }
    walk(&mut out, root);
    out
}

/// Validate menu structure and semantics. Returns a list of human-readable issues.
#[must_use]
pub fn validate_menu(root: &[MenuItem]) -> Vec<String> {
    use std::collections::HashSet;
    let mut issues: Vec<String> = Vec::new();
    let mut seen_aliases: HashSet<String> = HashSet::new();
    let mut dup_aliases: HashSet<String> = HashSet::new();

    #[allow(clippy::items_after_statements)] // Helper function logically placed here
    fn walk(
        items: &[MenuItem],
        path: &mut Vec<String>,
        seen: &mut std::collections::HashSet<String>,
        dups: &mut std::collections::HashSet<String>,
        out: &mut Vec<String>,
    ) {
        for it in items {
            path.push(it.name.clone());
            let here = path.join(" > ");

            // Structural checks
            let has_items = !it.items.is_empty();
            let has_cmd = it
                .cmd
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());
            let has_file = it
                .file
                .as_ref()
                .is_some_and(|s| !s.trim().is_empty());

            if has_items && (has_cmd || has_file) {
                out.push(format!(
                    "Menu item '{here}' cannot have 'items' together with 'cmd' or 'file'"
                ));
            }
            if has_cmd && has_file {
                out.push(format!(
                    "Menu item '{here}' cannot specify both 'cmd' and 'file'"
                ));
            }
            if !has_items && !has_cmd && !has_file {
                out.push(format!(
                    "Menu item '{here}' has no action ('cmd'/'file') and no 'items'"
                ));
            }

            // Alias checks (single and multi)
            if let Some(a) = it.alias.as_ref().filter(|a| !a.trim().is_empty()) {
                let key = a.trim().to_string();
                if !seen.insert(key.clone()) {
                    dups.insert(key);
                }
            }
            if let Some(list) = &it.aliases {
                for a in list.iter().filter(|a| !a.trim().is_empty()) {
                    let key = a.trim().to_string();
                    if !seen.insert(key.clone()) {
                        dups.insert(key);
                    }
                }
            }

            if !it.items.is_empty() {
                walk(&it.items, path, seen, dups, out);
            }
            path.pop();
        }
    }

    let mut path: Vec<String> = Vec::new();
    walk(
        root,
        &mut path,
        &mut seen_aliases,
        &mut dup_aliases,
        &mut issues,
    );

    if !dup_aliases.is_empty() {
        let mut v: Vec<String> = dup_aliases.into_iter().collect();
        v.sort_unstable();
        issues.push(format!("Duplicate aliases: {}", v.join(", ")));
    }

    issues
}

/// Loads menu configuration from the specified path.
/// 
/// # Errors
/// Returns error if file reading or TOML parsing fails.
pub fn load_menu(path: &Path) -> Result<MenuState> {
    let dx_file = load_dx_file(path)?;
    Ok(MenuState {
        items: dx_file.menu,
        selected_index: 0,
        path: Vec::new(),
    })
}

pub fn prepend_readme_item(menu: &mut MenuState) {
    if Path::new("README.md").exists() {
        let readme_item = MenuItem {
            name: "README.md".to_string(),
            desc: Some("View project documentation".to_string()),
            alias: None,
            aliases: None,
            cmd: None,
            file: Some("README.md".to_string()),
            items: Vec::new(),
            capture: None,
            external: None,
            enhanced_terminal: None,
            form: None,
            plugin_list: false,
        };
        menu.items.insert(0, readme_item);
        menu.selected_index = 0;
    }
}

#[allow(dead_code)]
pub fn append_configuration_item(menu: &mut MenuState) {
    let config_item = MenuItem {
        name: "Configuration".to_string(),
        desc: Some("Edit dx settings".to_string()),
        alias: Some("config".to_string()),
        aliases: Some(vec!["cfg".to_string()]),
        cmd: None,
        file: None,
        items: Vec::new(),
        capture: None,
        external: None,
        enhanced_terminal: None,
        form: None,
        plugin_list: false,
    };
    menu.items.push(config_item);
}

#[allow(dead_code)]
/// Loads menu configuration with extra validation and processing.
/// 
/// # Errors
/// Returns error if file reading, TOML parsing, or validation fails.
pub fn load_menu_with_extras(path: &Path) -> Result<MenuState> {
    let mut m = load_menu(path)?;
    prepend_readme_item(&mut m);
    append_configuration_item(&mut m);
    Ok(m)
}

#[allow(dead_code)]
pub fn append_dx_menu(menu: &mut MenuState) {
    let dx_children: Vec<MenuItem> = vec![
        MenuItem {
            name: "Doctor (quick)".to_string(),
            desc: Some("Validate config in current directory".to_string()),
            alias: Some("dx.doctor".to_string()),
            aliases: None,
            cmd: Some("dx doctor".to_string()),
            file: None,
            items: Vec::new(),
            capture: None,
            external: Some(false),
            enhanced_terminal: None,
            form: None,
            plugin_list: false,
        },
        MenuItem {
            name: "Doctor (full)".to_string(),
            desc: Some("Full diagnostics: config, plugin paths, effective settings".to_string()),
            alias: Some("dx.doctor.full".to_string()),
            aliases: None,
            cmd: Some("dx doctor --full".to_string()),
            file: None,
            items: Vec::new(),
            capture: None,
            external: Some(false),
            enhanced_terminal: None,
            form: None,
            plugin_list: false,
        },
    ];
    let dx_folder = MenuItem {
        name: "DX".to_string(),
        desc: Some("DX built-in tools".to_string()),
        alias: Some("dx".to_string()),
        aliases: None,
        cmd: None,
        file: None,
        items: dx_children,
        capture: None,
        external: None,
        enhanced_terminal: None,
        form: None,
        plugin_list: false,
    };
    menu.items.push(dx_folder);
}

// == NEW UNIFIED FORMAT LOADING ==
/// Loads DX configuration file from the specified path.
/// 
/// # Errors
/// Returns error if file reading or parsing fails.
pub fn load_dx_file(path: &Path) -> Result<DxFile> {
    let contents = fs::read_to_string(path)?;

    // Try to parse as unified format first
    if path.extension().and_then(|s| s.to_str()) == Some("yaml")
        || path.extension().and_then(|s| s.to_str()) == Some("yml")
    {
        match serde_yaml::from_str::<DxFile>(&contents) {
            Ok(dx_file) => return Ok(dx_file),
            Err(_) => {
                // If unified format fails, try old menu format as fallback
                match serde_yaml::from_str::<MenuConfig>(&contents) {
                    Ok(menu_config) => {
                        return Ok(DxFile {
                            config: None,
                            menu: menu_config.items,
                        });
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
        match toml::from_str::<DxFile>(&contents) {
            Ok(dx_file) => return Ok(dx_file),
            Err(_) => {
                // If unified format fails, try old menu format as fallback
                match toml::from_str::<MenuConfig>(&contents) {
                    Ok(menu_config) => {
                        return Ok(DxFile {
                            config: None,
                            menu: menu_config.items,
                        });
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
        match serde_json::from_str::<DxFile>(&contents) {
            Ok(dx_file) => return Ok(dx_file),
            Err(_) => {
                // If unified format fails, try old menu format as fallback
                match serde_json::from_str::<MenuConfig>(&contents) {
                    Ok(menu_config) => {
                        return Ok(DxFile {
                            config: None,
                            menu: menu_config.items,
                        });
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }

    Err(anyhow::anyhow!("Unsupported file format"))
}
