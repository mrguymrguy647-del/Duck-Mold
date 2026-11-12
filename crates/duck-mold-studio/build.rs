//! Build script: Cargo runs this before compiling the Studio.
//!
//! On Windows it puts the duck icon and the program's name into
//! DuckMoldStudio.exe, so File Explorer, the taskbar and desktop
//! shortcuts show them. On other systems it does nothing.

fn main() {
    // Only run again when the icon or the version number changes.
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.toml");

    // Only Windows programs carry an icon like this.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/icon.ico");
    resource.set("ProductName", "Duck Mold Studio");
    resource.set("FileDescription", "Duck Mold Studio");
    if let Err(error) = resource.compile() {
        // On your own PC a missing icon is not worth failing the whole
        // build over. On GitHub's build machines (which set CI), it is:
        // the download must have its icon.
        if std::env::var_os("CI").is_some() {
            panic!("Could not add the icon to DuckMoldStudio.exe: {error}");
        }
        println!("cargo:warning=Could not add the icon to DuckMoldStudio.exe: {error}");
    }
}
