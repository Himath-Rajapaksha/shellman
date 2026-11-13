use crate::N_;

pub const MIN_ROWS: i32 = 3;
pub const MIN_FILES_HEIGHT: i32 = 120;
pub const SCROLLBACK_LINES: i64 = 10_000;
pub const BUTTON_ICON: &str = "utilities-terminal-symbolic";
pub const ACTIONS: &[(&str, &str)] = &[
    ("toggle", N_!("Show or Hide the Terminal")),
    ("copy", N_!("Copy")),
    ("paste", N_!("Paste")),
    ("insert", N_!("Insert Selected Files")),
    ("split-right", N_!("Split Right")),
    ("split-down", N_!("Split Down")),
    ("split-auto", N_!("Split Automatically")),
    ("close", N_!("Close Terminal")),
    ("goto-tab-dir", N_!("Go to Tab Folder")),
    ("goto-selected-folder", N_!("Go to Selected Folder")),
    ("focus-left", N_!("Go to Left Terminal")),
    ("focus-right", N_!("Go to Right Terminal")),
    ("focus-up", N_!("Go to Terminal Above")),
    ("focus-down", N_!("Go to Terminal Below")),
];

pub const DEFAULT_SHORTCUTS: &[(&str, &str)] = &[
    ("F4", "toggle"),
    ("<Control><Shift>c", "copy"),
    ("<Control><Shift>v", "paste"),
    ("<Control><Shift>s", "insert"),
    ("<Alt><Shift>equal", "split-right"),
    ("<Alt><Shift>KP_Add", "split-right"),
    ("<Alt><Shift>minus", "split-down"),
    ("<Alt><Shift>KP_Subtract", "split-down"),
    ("<Alt><Shift>d", "split-auto"),
    ("<Control><Shift>w", "close"),
    ("<Control><Shift>g", "goto-tab-dir"),
    ("<Alt><Shift>Left", "focus-left"),
    ("<Alt><Shift>Right", "focus-right"),
    ("<Alt><Shift>Up", "focus-up"),
    ("<Alt><Shift>Down", "focus-down"),
];
pub const MIN_PANE_WIDTH: i32 = 160;

pub const TESTED_NAUTILUS: &[u32] = &[46, 47, 48, 50, 51];
