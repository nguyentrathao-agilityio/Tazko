use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const PLACEHOLDER: &str = "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Tazko</title></head><body><p>The web app was not built into this binary.</p></body></html>";

fn main() {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let web = root.join("web");
    for p in [
        "src",
        "public",
        "index.html",
        "package.json",
        "pnpm-lock.yaml",
        "vite.config.ts",
        "tsconfig.app.json",
    ] {
        println!("cargo:rerun-if-changed=web/{p}");
    }
    println!("cargo:rerun-if-changed=design/tokens.css");
    println!("cargo:rerun-if-env-changed=TAZKO_SKIP_WEB_BUILD");

    if env::var_os("TAZKO_SKIP_WEB_BUILD").is_none() && web.join("package.json").exists() {
        build_web(&web);
    }

    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let blobs = out.join("web");
    let _ = fs::remove_dir_all(&blobs);
    fs::create_dir_all(&blobs).unwrap();

    let mut files = Vec::new();
    let dist = web.join("dist");
    if dist.join("index.html").exists() {
        collect(&dist, &dist, &mut files);
    } else {
        println!("cargo:warning=web/dist is missing; embedding a placeholder page");
        files.push(("/index.html".to_string(), PLACEHOLDER.as_bytes().to_vec()));
    }
    files.sort();

    let mut table = String::from("pub static ASSETS: &[Asset] = &[\n");
    for (i, (path, bytes)) in files.iter().enumerate() {
        let blob = blobs.join(format!("{i}.br"));
        fs::write(&blob, compress(bytes)).unwrap();
        writeln!(
            table,
            "    Asset {{ path: {path:?}, mime: {:?}, br: include_bytes!({:?}) }},",
            mime(path),
            blob.display().to_string()
        )
        .unwrap();
    }
    table.push_str("];\n");
    fs::write(out.join("assets.rs"), table).unwrap();
}

fn build_web(web: &Path) {
    if !web.join("node_modules").exists() {
        run(web, &["install", "--frozen-lockfile"]);
    }
    run(web, &["run", "build"]);
}

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("pnpm")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "could not run pnpm {}: {e}; install pnpm or set TAZKO_SKIP_WEB_BUILD=1",
                args.join(" ")
            )
        });
    assert!(status.success(), "pnpm {} failed in web/", args.join(" "));
}

fn collect(base: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect(base, &path, files);
        } else {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            files.push((format!("/{rel}"), fs::read(&path).unwrap()));
        }
    }
}

fn compress(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
        w.write_all(bytes).unwrap();
    }
    out
}

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
