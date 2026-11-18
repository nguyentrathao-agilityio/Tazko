use std::io::Read;

pub struct Asset {
    pub path: &'static str,
    pub mime: &'static str,
    pub br: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub fn find(path: &str) -> Option<&'static Asset> {
    ASSETS.iter().find(|a| a.path == path)
}

pub fn index() -> &'static Asset {
    find("/index.html").expect("index.html is always embedded")
}

pub fn decompress(asset: &Asset) -> Vec<u8> {
    let mut out = Vec::new();
    brotli::Decompressor::new(asset.br, 4096)
        .read_to_end(&mut out)
        .expect("embedded asset is valid brotli");
    out
}
