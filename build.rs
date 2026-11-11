use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=po");
    println!("cargo:rerun-if-env-changed=SHELLMAN_LOCALEDIR");
    let installed = std::env::var("SHELLMAN_LOCALEDIR").ok();
    let localedir = installed.clone().unwrap_or_else(|| "/usr/share/locale".into());
    println!("cargo:rustc-env=SHELLMAN_LOCALEDIR={localedir}");

    // packaged build: translations only from LOCALEDIR, never from the build folder
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("locale");
    let build_dir = if installed.is_some() { String::new() } else { out.display().to_string() };
    println!("cargo:rustc-env=SHELLMAN_BUILD_LOCALEDIR={build_dir}");
    let Ok(linguas) = std::fs::read_to_string("po/LINGUAS") else { return };
    for lang in linguas.split_whitespace().filter(|l| !l.starts_with('#')) {
        let dir = out.join(lang).join("LC_MESSAGES");
        std::fs::create_dir_all(&dir).unwrap();
        let status = Command::new("msgfmt")
            .args(["--check", "-o"])
            .arg(dir.join("shellman.mo"))
            .arg(format!("po/{lang}.po"))
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => println!("cargo:warning=msgfmt failed for po/{lang}.po: interface stays in English"),
        }
    }
}
