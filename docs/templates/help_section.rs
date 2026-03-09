//! help_section.rs
//!
//! Reusable, collapsible help panel widget for iced daemon applications.
//!
//! Features:
//! - Structured help content (steps, notes, tips, warnings)
//! - Collapsible sections with toggle buttons
//! - Help registry for multiple sections
//! - Step-by-step guide builder
//!
//! # Example
//!
//! ```rust
//! use help_section::{HelpSection, HelpItem, HelpPanelState, view_help_panel, HelpMsg};
//!
//! // Build state once
//! let state = HelpPanelState::from_registry(&HelpRegistry::default());
//!
//! // In your view():
//! let help = view_help_panel(&state);
//! // map HelpMsg into your top-level Message
//! ```

use iced::widget::{button, column, container, row, text, Column};
use iced::{Alignment, Element, Length, Padding};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Help item types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum HelpItemType {
    Step { number: usize },
    Note,
    Tip,
    Warning,
    Info,
}

/// Individual help item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpItem {
    /// Item type (step, note, tip, etc.)
    #[serde(flatten)]
    pub item_type: HelpItemType,
    /// Item title/heading
    pub title: String,
    /// Item description/content
    pub content: String,
    /// Optional icon prefix (text, not emoji)
    pub icon: Option<String>,
}

impl HelpItem {
    /// Create a numbered step item
    pub fn step(number: usize, content: impl Into<String>) -> Self {
        Self {
            item_type: HelpItemType::Step { number },
            title: format!("Step {}", number),
            content: content.into(),
            icon: None,
        }
    }

    /// Create a note item
    pub fn note(content: impl Into<String>) -> Self {
        Self {
            item_type: HelpItemType::Note,
            title: "Note".to_string(),
            content: content.into(),
            icon: Some("[i]".to_string()),
        }
    }

    /// Create a tip item
    pub fn tip(content: impl Into<String>) -> Self {
        Self {
            item_type: HelpItemType::Tip,
            title: "Tip".to_string(),
            content: content.into(),
            icon: Some("[*]".to_string()),
        }
    }

    /// Create a warning item
    pub fn warning(content: impl Into<String>) -> Self {
        Self {
            item_type: HelpItemType::Warning,
            title: "Warning".to_string(),
            content: content.into(),
            icon: Some("[!]".to_string()),
        }
    }

    /// Create an info item
    pub fn info(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            item_type: HelpItemType::Info,
            title: title.into(),
            content: content.into(),
            icon: Some("[i]".to_string()),
        }
    }

    /// Set custom icon prefix
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Help section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelpSection {
    /// Section ID
    pub id: String,
    /// Section title
    pub title: String,
    /// Help items in this section
    pub items: Vec<HelpItem>,
    /// Whether section is collapsible
    pub collapsible: bool,
    /// Whether section starts expanded
    pub initially_expanded: bool,
}

impl HelpSection {
    /// Create a new help section
    pub fn new(title: impl Into<String>) -> Self {
        let title_str = title.into();
        let id = title_str
            .to_lowercase()
            .replace(' ', "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();

        Self {
            id,
            title: title_str,
            items: Vec::new(),
            collapsible: true,
            initially_expanded: false,
        }
    }

    /// Add a help item
    pub fn add_item(mut self, item: HelpItem) -> Self {
        self.items.push(item);
        self
    }

    /// Add multiple items
    pub fn add_items(mut self, items: Vec<HelpItem>) -> Self {
        self.items.extend(items);
        self
    }

    /// Set collapsible state
    pub fn collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Set initially expanded state
    pub fn initially_expanded(mut self, expanded: bool) -> Self {
        self.initially_expanded = expanded;
        self
    }
}

// =============================================================================
// HELP REGISTRY
// =============================================================================

/// Help content registry
pub struct HelpRegistry {
    sections: HashMap<String, HelpSection>,
}

impl HelpRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            sections: HashMap::new(),
        }
    }

    /// Register a help section
    pub fn register(&mut self, section: HelpSection) {
        self.sections.insert(section.id.clone(), section);
    }

    /// Get a help section by ID
    pub fn get(&self, id: &str) -> Option<&HelpSection> {
        self.sections.get(id)
    }

    /// Get all sections (ordered by ID for deterministic rendering)
    pub fn all_sections(&self) -> Vec<&HelpSection> {
        let mut sections: Vec<_> = self.sections.values().collect();
        sections.sort_by_key(|s| &s.id);
        sections
    }

    /// Get all section IDs
    pub fn list_sections(&self) -> Vec<String> {
        self.sections.keys().cloned().collect()
    }
}

impl Default for HelpRegistry {
    fn default() -> Self {
        let mut registry = Self::new();
        registry.register(create_file_processor_help());
        registry.register(create_settings_help());
        registry
    }
}

// =============================================================================
// DEFAULT HELP SECTIONS
// =============================================================================

/// Create help for file processor
fn create_file_processor_help() -> HelpSection {
    HelpSection::new("File Processor")
        .add_item(HelpItem::step(1, "Select your input folder containing files to process"))
        .add_item(HelpItem::step(2, "Configure processing options as needed"))
        .add_item(HelpItem::step(3, "Click Start to begin processing"))
        .add_item(HelpItem::note("Processing may take several minutes depending on file count"))
        .add_item(HelpItem::tip("Use the progress bar to monitor processing status"))
        .initially_expanded(false)
}

/// Create help for settings
fn create_settings_help() -> HelpSection {
    HelpSection::new("Settings")
        .add_item(HelpItem::info(
            "General Settings",
            "Configure application behavior and appearance",
        ))
        .add_item(HelpItem::info(
            "Advanced Settings",
            "Configure advanced options (use with caution)",
        ))
        .add_item(HelpItem::warning(
            "Changes take effect immediately and are saved automatically",
        ))
        .initially_expanded(true)
}

// =============================================================================
// ICED WIDGET STATE & MESSAGES
// =============================================================================

/// Messages emitted by the help panel
#[derive(Debug, Clone)]
pub enum HelpMsg {
    /// Toggle a section's expanded/collapsed state
    ToggleSection(String),
}

/// Runtime state for the help panel (tracks which sections are expanded)
#[derive(Debug, Clone)]
pub struct HelpPanelState {
    /// Ordered list of sections to display
    pub sections: Vec<HelpSection>,
    /// Expanded state per section ID
    pub expanded: HashMap<String, bool>,
}

impl HelpPanelState {
    /// Build panel state from a registry, respecting `initially_expanded`
    pub fn from_registry(registry: &HelpRegistry) -> Self {
        let sections: Vec<HelpSection> = registry.all_sections().into_iter().cloned().collect();
        let expanded = sections
            .iter()
            .map(|s| (s.id.clone(), s.initially_expanded))
            .collect();

        Self { sections, expanded }
    }

    /// Handle a `HelpMsg` by mutating state in place
    pub fn update(&mut self, msg: HelpMsg) {
        match msg {
            HelpMsg::ToggleSection(id) => {
                if let Some(val) = self.expanded.get_mut(&id) {
                    *val = !*val;
                }
            }
        }
    }
}

// =============================================================================
// VIEW
// =============================================================================

/// Render the full help panel as a collapsible set of sections.
///
/// # Returns
///
/// An `Element<HelpMsg>` to embed in any window's layout.
pub fn view_help_panel<'a>(state: &HelpPanelState) -> Element<'a, HelpMsg> {
    let mut content = Column::new().spacing(8).padding(12).width(Length::Fill);

    for section in &state.sections {
        let is_expanded = state.expanded.get(&section.id).copied().unwrap_or(false);
        content = content.push(view_section(section, is_expanded));
    }

    container(content)
        .width(Length::Fill)
        .into()
}

/// Render a single help section with a toggle header
fn view_section<'a>(section: &HelpSection, expanded: bool) -> Element<'a, HelpMsg> {
    let toggle_label = if expanded { "[-]" } else { "[+]" };
    let section_id = section.id.clone();

    let header = if section.collapsible {
        let btn: Element<'a, HelpMsg> = button(
            row![
                text(toggle_label).size(14),
                text(&section.title).size(15),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .on_press(HelpMsg::ToggleSection(section_id))
        .padding(Padding::from([4, 8]))
        .into();
        btn
    } else {
        text(&section.title).size(15).into()
    };

    let mut col = Column::new().spacing(4).push(header);

    if expanded || !section.collapsible {
        for item in &section.items {
            col = col.push(view_help_item(item));
        }
    }

    container(col)
        .width(Length::Fill)
        .padding(Padding::from([4, 0]))
        .into()
}

/// Render a single help item
fn view_help_item<'a>(item: &HelpItem) -> Element<'a, HelpMsg> {
    let prefix = match &item.item_type {
        HelpItemType::Step { number } => format!("  {}.", number),
        _ => {
            if let Some(ref icon) = item.icon {
                format!("  {}", icon)
            } else {
                "  -".to_string()
            }
        }
    };

    let label = format!("{} {} -- {}", prefix, item.title, item.content);

    text(label).size(13).into()
}

// =============================================================================
// BUILDER PATTERN FOR STEP-BY-STEP GUIDES
// =============================================================================

/// Builder for creating step-by-step guides
pub struct GuideBuilder {
    title: String,
    steps: Vec<String>,
    notes: Vec<String>,
}

impl GuideBuilder {
    /// Create a new guide builder
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            steps: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Add a step
    pub fn add_step(mut self, step: impl Into<String>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// Add a note
    pub fn add_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Build the help section
    pub fn build(self) -> HelpSection {
        let mut section = HelpSection::new(self.title);

        for (i, step) in self.steps.iter().enumerate() {
            section = section.add_item(HelpItem::step(i + 1, step));
        }

        for note in self.notes {
            section = section.add_item(HelpItem::note(note));
        }

        section
    }
}

// =============================================================================
// DEPENDENCIES
// =============================================================================

// Add to Cargo.toml:
// [dependencies]
// iced = { version = "0.14", features = ["multi-window"] }
// serde = { version = "1.0", features = ["derive"] }

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_item_creation() {
        let step = HelpItem::step(1, "First step");
        assert_eq!(step.title, "Step 1");
        assert_eq!(step.content, "First step");

        let note = HelpItem::note("Important note");
        assert_eq!(note.title, "Note");
        assert!(note.icon.is_some());
    }

    #[test]
    fn test_help_section_builder() {
        let section = HelpSection::new("Test Section")
            .add_item(HelpItem::step(1, "Step one"))
            .add_item(HelpItem::note("A note"))
            .collapsible(true)
            .initially_expanded(false);

        assert_eq!(section.title, "Test Section");
        assert_eq!(section.items.len(), 2);
        assert!(section.collapsible);
        assert!(!section.initially_expanded);
    }

    #[test]
    fn test_guide_builder() {
        let guide = GuideBuilder::new("My Guide")
            .add_step("First step")
            .add_step("Second step")
            .add_note("Important note")
            .build();

        assert_eq!(guide.items.len(), 3);
    }

    #[test]
    fn test_help_registry() {
        let mut registry = HelpRegistry::new();
        let section = HelpSection::new("Test");
        let id = section.id.clone();

        registry.register(section);
        assert!(registry.get(&id).is_some());
        assert_eq!(registry.list_sections().len(), 1);
    }

    #[test]
    fn test_panel_state_toggle() {
        let registry = HelpRegistry::default();
        let mut state = HelpPanelState::from_registry(&registry);

        // Settings section starts expanded
        assert_eq!(state.expanded.get("settings"), Some(&true));

        // Toggle it
        state.update(HelpMsg::ToggleSection("settings".to_string()));
        assert_eq!(state.expanded.get("settings"), Some(&false));

        // Toggle back
        state.update(HelpMsg::ToggleSection("settings".to_string()));
        assert_eq!(state.expanded.get("settings"), Some(&true));
    }
}
