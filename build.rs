//! Embeds the flower pictures (`assets/flowers/*.png`) and models
//! (`assets/flowers/*.glb`) into the binary, like every other asset of the
//! game, so a release needs no assets folder. Adding or changing one
//! triggers a rebuild.

use std::{env, fs, path::Path};

fn main() {
    let root = env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let dir = Path::new(&root).join("assets/flowers");
    println!("cargo:rerun-if-changed={}", dir.display());

    let mut out = String::new();
    for (name, extension, what) in [
        ("FILES", "png", "Painted flower pictures"),
        ("MODELS", "glb", "Flower models"),
    ] {
        let mut files: Vec<_> = fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .is_some_and(|x| x.eq_ignore_ascii_case(extension))
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        out += &format!(
            "/// {what} by file name (without `.{extension}`), embedded at build time.\n\
             pub static {name}: &[(&str, &[u8])] = &[\n"
        );
        for path in files {
            let stem = path
                .file_stem()
                .expect("a file name")
                .to_string_lossy()
                .to_lowercase();
            out += &format!(
                "    ({stem:?}, include_bytes!({:?})),\n",
                path.display().to_string()
            );
        }
        out += "];\n";
    }
    let target = Path::new(&env::var("OUT_DIR").expect("set by cargo")).join("flower_sprites.rs");
    fs::write(target, out).expect("write generated sprite list");
}
