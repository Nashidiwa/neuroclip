fn main() {
    tauri_build::build();

    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        match std::env::var_os("OUT_DIR") {
            Some(out_dir) => {
                let resource_lib = std::path::PathBuf::from(out_dir).join("resource.lib");
                if resource_lib.is_file() {
                    println!("cargo:rustc-link-arg-tests={}", resource_lib.display());
                } else {
                    println!(
                        "cargo:warning=Missing Windows test manifest resource library at {}",
                        resource_lib.display()
                    );
                }
            }
            None => {
                println!(
                    "cargo:warning=OUT_DIR is not set; skipping Windows test resource linking"
                );
            }
        }
    }
}
