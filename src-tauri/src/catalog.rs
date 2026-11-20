use crate::config::LaunchifyConfig;
use crate::model::{IndexItem, ItemKind};

/// Returns synonym keywords for well-known application or system titles.
pub fn synonym_keywords(title: &str) -> Vec<String> {
    let lower = title.to_lowercase();
    let mut out = Vec::new();

    if lower.contains("visual studio code") || lower == "code" {
        out.extend(["vscode", "vsc", "editor"].into_iter().map(String::from));
    }
    if lower.contains("windows terminal") || lower == "terminal" {
        out.extend(["wt", "cmd", "shell", "powershell"].into_iter().map(String::from));
    }
    if lower.contains("task manager") {
        out.extend(["taskmgr", "processes", "cpu", "ram"].into_iter().map(String::from));
    }
    if lower.contains("control panel") {
        out.extend(["ctrl", "settings"].into_iter().map(String::from));
    }
    if lower.contains("file explorer") || lower == "explorer" {
        out.extend(["files", "folders", "dir"].into_iter().map(String::from));
    }
    if lower.contains("chrome") {
        out.extend(["browser", "web"].into_iter().map(String::from));
    }
    if lower.contains("edge") {
        out.extend(["browser", "web"].into_iter().map(String::from));
    }
    if lower.contains("firefox") {
        out.extend(["browser", "web"].into_iter().map(String::from));
    }
    if lower.contains("snipping tool") {
        out.extend(["screenshot", "snip", "capture"].into_iter().map(String::from));
    }
    if lower == "settings" {
        out.extend(["preferences", "config"].into_iter().map(String::from));
    }

    out
}

/// Built-in Windows Settings (`ms-settings:*`) deep-links and Power actions.
pub fn builtin_catalog() -> Vec<IndexItem> {
    let settings: &[(&str, &str, &str, &[&str])] = &[
        ("display", "Display Settings", "ms-settings:display", &["monitor", "resolution", "brightness", "scale"]),
        ("sound", "Sound Settings", "ms-settings:sound", &["audio", "volume", "speaker", "microphone"]),
        ("wifi", "Wi-Fi Settings", "ms-settings:network-wifi", &["network", "internet", "wireless"]),
        ("bluetooth", "Bluetooth & Devices", "ms-settings:bluetooth", &["devices", "wireless", "pair"]),
        ("apps", "Apps & Features", "ms-settings:appsfeatures", &["uninstall", "programs", "installed"]),
        ("update", "Windows Update", "ms-settings:windowsupdate", &["updates", "patch", "os"]),
        ("privacy", "Privacy & Security", "ms-settings:privacy", &["security", "permissions", "defender"]),
        ("personalization", "Personalization", "ms-settings:personalization", &["theme", "wallpaper", "colors", "dark"]),
        ("storage", "Storage Settings", "ms-settings:storagesense", &["disk", "space", "cleanup"]),
        ("taskbar", "Taskbar Settings", "ms-settings:taskbar", &["start", "tray", "pin"]),
        ("defaultapps", "Default Apps", "ms-settings:defaultapps", &["browser", "associations"]),
        ("startup", "Startup Apps", "ms-settings:startupapps", &["boot", "autostart"]),
        ("about", "About This PC", "ms-settings:about", &["system", "specs", "version", "ram"]),
        ("datetime", "Date & Time", "ms-settings:dateandtime", &["clock", "timezone"]),
        ("notifications", "Notifications", "ms-settings:notifications", &["alerts", "focus", "dnd"]),
        ("power", "Power & Battery", "ms-settings:powersleep", &["sleep", "battery", "energy"]),
    ];

    let mut items = Vec::with_capacity(settings.len() + 6);
    for (id, title, target, kws) in settings {
        items.push(IndexItem {
            id: format!("setting:{id}"),
            title: (*title).to_string(),
            subtitle: format!("Windows Settings · {target}"),
            keywords: kws.iter().map(|s| (*s).to_string()).collect(),
            kind: ItemKind::SystemSetting,
            target: (*target).to_string(),
            icon_key: None,
        });
    }

    let power_actions: &[(&str, &str, &str, &[&str])] = &[
        ("lock", "Lock PC", "power:lock", &["lock", "screen", "win+l"]),
        ("sleep", "Sleep", "power:sleep", &["suspend", "standby"]),
        ("restart", "Restart PC", "power:restart", &["reboot", "reset"]),
        ("shutdown", "Shut Down PC", "power:shutdown", &["power off", "halt", "turn off"]),
        ("signout", "Sign Out", "power:signout", &["logout", "logoff"]),
        ("hibernate", "Hibernate", "power:hibernate", &["deep sleep"]),
        ("empty_recycle_bin", "Empty Recycle Bin", "power:empty_recycle_bin", &["recycle", "trash", "bin", "clean"]),
    ];

    for (id, title, target, kws) in power_actions {
        items.push(IndexItem {
            id: format!("power:{id}"),
            title: (*title).to_string(),
            subtitle: "System Power Action".to_string(),
            keywords: kws.iter().map(|s| (*s).to_string()).collect(),
            kind: ItemKind::PowerAction,
            target: (*target).to_string(),
            icon_key: None,
        });
    }

    items
}

/// Converts user-configured custom workflows and web shortcuts into `IndexItem`s.
pub fn config_items(cfg: &LaunchifyConfig) -> Vec<IndexItem> {
    let mut items = Vec::with_capacity(cfg.custom_workflows.len() + cfg.web_shortcuts.len());

    for wf in &cfg.custom_workflows {
        items.push(IndexItem {
            id: format!("workflow:{}", wf.keyword),
            title: wf.name.clone(),
            subtitle: format!("Workflow · {}", wf.keyword),
            keywords: vec![wf.keyword.clone()],
            kind: ItemKind::Workflow,
            target: wf.command.clone(),
            icon_key: None,
        });
    }

    for ws in &cfg.web_shortcuts {
        items.push(IndexItem {
            id: format!("web:{}", ws.keyword),
            title: ws.name.clone(),
            subtitle: format!("Web Shortcut · {}", ws.keyword),
            keywords: vec![ws.keyword.clone()],
            kind: ItemKind::WebShortcut,
            target: ws.url_template.clone(),
            icon_key: None,
        });
    }

    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{WebShortcutConfig, WorkflowConfig};

    #[test]
    fn builtin_catalog_has_settings_and_power_actions() {
        let items = builtin_catalog();
        assert!(items.iter().any(|i| i.kind == ItemKind::SystemSetting && i.target == "ms-settings:display"));
        assert!(items.iter().any(|i| i.kind == ItemKind::PowerAction && i.target == "power:lock"));
        assert!(items.iter().any(|i| i.kind == ItemKind::PowerAction && i.target == "power:shutdown"));
        assert!(items.iter().any(|i| i.kind == ItemKind::PowerAction && i.target == "power:hibernate"));
        assert!(items.iter().any(|i| i.kind == ItemKind::PowerAction && i.target == "power:signout"));
        assert!(items.iter().any(|i| i.kind == ItemKind::PowerAction && i.target == "power:empty_recycle_bin"));
    }

    #[test]
    fn synonyms_include_vscode_and_terminal() {
        let vsc = synonym_keywords("Visual Studio Code");
        assert!(vsc.contains(&"vscode".to_string()));
        let wt = synonym_keywords("Windows Terminal");
        assert!(wt.contains(&"wt".to_string()));
    }

    #[test]
    fn config_items_maps_workflows_and_web_shortcuts() {
        let mut cfg = LaunchifyConfig::default();
        cfg.custom_workflows.push(WorkflowConfig {
            name: "Dev Server".into(),
            keyword: "dev".into(),
            command: "npm run dev".into(),
            args: vec![],
            run_in_terminal: true,
        });
        cfg.web_shortcuts.push(WebShortcutConfig {
            keyword: "g".into(),
            name: "Google Search".into(),
            url_template: "https://google.com/search?q={query}".into(),
        });

        let items = config_items(&cfg);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, ItemKind::Workflow);
        assert_eq!(items[1].kind, ItemKind::WebShortcut);
    }
}
