use std::{env, path::PathBuf, process::Command};

fn main() {
  if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
    build_webkit_youtube_extension();
  }
  tauri_build::build()
}

fn build_webkit_youtube_extension() {
  let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"));
  let extension = output_directory.join("libbuitenzorg_youtube_referer.so");
  println!("cargo:rerun-if-changed=src/youtube_referer_extension.c");

  let pkg_config = Command::new("pkg-config")
    .args(["--cflags", "--libs", "webkit2gtk-web-extension-4.1"])
    .output()
    .expect("pkg-config and WebKitGTK 4.1 development files are required on Linux");
  assert!(pkg_config.status.success(), "WebKitGTK 4.1 WebExtension development files are required on Linux");
  let flags = String::from_utf8(pkg_config.stdout).expect("pkg-config output is valid UTF-8");

  let compiler = env::var("CC").unwrap_or_else(|_| "cc".to_owned());
  let status = Command::new(compiler)
    .arg("-shared")
    .arg("-fPIC")
    .arg("-o")
    .arg(&extension)
    .arg("src/youtube_referer_extension.c")
    .args(flags.split_whitespace())
    .status()
    .expect("a C compiler is required to build the Linux WebKitGTK extension");
  assert!(status.success(), "failed to build the Linux YouTube referer extension");
  println!("cargo:rustc-env=YOUTUBE_REFERER_EXTENSION_PATH={}", extension.display());
}
