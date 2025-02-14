use std::env;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    cbindgen::Builder::new()
      .with_src("src/c_exports.rs")
	  .with_config(cbindgen::Config::from_root_or_default(crate_dir))
      .generate()
      .expect("Unable to generate bindings")
      .write_to_file("target/rusty_platinum_utils.h");
}
