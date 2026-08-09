use std::{env, path::PathBuf};

fn main() {
    println!("cargo::rerun-if-changed=wrapper.h");

    let libpci = pkg_config::Config::new()
        .probe("libpci")
        .expect("failed to find libpci via pkg-config");

    let mut builder = bindgen::Builder::default()
        .header("wrapper.h")
        .allowlist_type("pci_access")
        .allowlist_type("pci_dev")
        .allowlist_type("pci_lookup_mode")
        .allowlist_function(
            "pci_(alloc|init|cleanup|scan_bus|fill_info|lookup_name|get_string_property)",
        )
        .allowlist_var("PCI_FILL_.*")
        .allowlist_var("PCI_LOOKUP_VENDOR")
        .allowlist_var("PCI_LOOKUP_DEVICE")
        .allowlist_var("PCI_LOOKUP_CLASS")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()));

    for include_path in libpci.include_paths {
        builder = builder.clang_arg(format!("-I{}", include_path.display()));
    }

    let bindings = builder
        .generate()
        .expect("failed to generate libpci bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("failed to write generated bindings");
}
