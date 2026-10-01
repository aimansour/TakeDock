use std::collections::HashMap;

#[derive(Default)]
pub struct Windows {
    names: HashMap<String, String>,
    focused: Vec<String>,
    next: u64,
}
impl Windows {
    pub fn add(&mut self, label: &str, language: &str) -> String {
        self.next += 1;
        let name = format!(
            "{} {}",
            if language == "ar" {
                "نافذة"
            } else {
                "Window"
            },
            self.next
        );
        self.names.insert(label.into(), name.clone());
        self.focus(label);
        name
    }
    pub fn name(&self, label: &str) -> String {
        self.names.get(label).cloned().unwrap_or_default()
    }
    pub fn rename(&mut self, label: &str, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err("invalid_window_name".into());
        }
        let current = self.names.get_mut(label).ok_or("window_closed")?;
        *current = name.into();
        Ok(name.into())
    }
    pub fn focus(&mut self, label: &str) {
        if self.names.contains_key(label) {
            self.focused.retain(|item| item != label);
            self.focused.push(label.into());
        }
    }
    pub fn remove(&mut self, label: &str) {
        self.names.remove(label);
        self.focused.retain(|item| item != label);
    }
    pub fn active(&self) -> Option<String> {
        self.focused.last().cloned()
    }
}
pub fn title(name: &str) -> String {
    format!("TakeDock — {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relaunch_targets_last_focused_window_and_closed_window_falls_back() {
        let mut windows = Windows::default();
        windows.add("main", "en");
        windows.add("window-two", "en");
        windows.focus("main");
        assert_eq!(windows.active().as_deref(), Some("main"));
        windows.focus("window-two");
        assert_eq!(windows.active().as_deref(), Some("window-two"));
        windows.remove("window-two");
        assert_eq!(windows.active().as_deref(), Some("main"));
        windows.remove("main");
        assert_eq!(windows.active(), None);
    }
    #[test]
    fn names_are_window_local_unicode_trimmed_and_validated() {
        let mut windows = Windows::default();
        windows.add("main", "ar");
        windows.add("window-two", "ar");
        assert_eq!(
            windows.rename("window-two", "  مقابلة  ").unwrap(),
            "مقابلة"
        );
        assert_eq!(windows.name("main"), "نافذة 1");
        assert_eq!(title(&windows.name("window-two")), "TakeDock — مقابلة");
        for name in ["", "  ", "bad\nname", "bad\0name"] {
            assert!(windows.rename("main", name).is_err());
        }
        assert!(windows.rename("main", &"x".repeat(81)).is_err());
        assert!(windows.rename("missing", "Valid").is_err());
    }
}
