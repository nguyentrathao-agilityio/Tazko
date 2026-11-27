#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind {
    pub mime: &'static str,
    pub image: bool,
}

struct Format {
    magic: fn(&[u8]) -> bool,
    exts: &'static [&'static str],
    mime: &'static str,
    image: bool,
    what: &'static str,
}

fn iso_media(b: &[u8]) -> bool {
    b.len() >= 12 && &b[4..8] == b"ftyp"
}

fn webm(b: &[u8]) -> bool {
    b.starts_with(b"\x1a\x45\xdf\xa3")
}

const FORMATS: &[Format] = &[
    Format {
        magic: |b| b.starts_with(b"\x89PNG\r\n\x1a\n"),
        exts: &["png"],
        mime: "image/png",
        image: true,
        what: "a PNG image",
    },
    Format {
        magic: |b| b.starts_with(b"\xff\xd8\xff"),
        exts: &["jpg", "jpeg"],
        mime: "image/jpeg",
        image: true,
        what: "a JPEG image",
    },
    Format {
        magic: |b| b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a"),
        exts: &["gif"],
        mime: "image/gif",
        image: true,
        what: "a GIF image",
    },
    Format {
        magic: |b| b.len() >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP",
        exts: &["webp"],
        mime: "image/webp",
        image: true,
        what: "a WebP image",
    },
    Format {
        magic: |b| b.starts_with(b"%PDF-"),
        exts: &["pdf"],
        mime: "application/pdf",
        image: false,
        what: "a PDF",
    },
    Format {
        magic: |b| b.starts_with(b"PK\x03\x04") || b.starts_with(b"PK\x05\x06"),
        exts: &["zip", "docx", "xlsx", "pptx", "odt", "ods", "odp", "epub"],
        mime: "application/zip",
        image: false,
        what: "a zip archive",
    },
    Format {
        magic: |b| b.starts_with(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1"),
        exts: &["doc", "xls", "ppt"],
        mime: "application/octet-stream",
        image: false,
        what: "an office document",
    },
    Format {
        magic: iso_media,
        exts: &["mp4", "m4v", "mov"],
        mime: "video/mp4",
        image: false,
        what: "a video",
    },
    Format {
        magic: iso_media,
        exts: &["m4a"],
        mime: "audio/mp4",
        image: false,
        what: "a sound recording",
    },
    Format {
        magic: webm,
        exts: &["webm"],
        mime: "video/webm",
        image: false,
        what: "a WebM video",
    },
];

const PLAIN: &str = "text/plain; charset=utf-8";

const TEXT: &[(&str, &str)] = &[
    ("txt", PLAIN),
    ("md", PLAIN),
    ("log", PLAIN),
    ("csv", "text/csv; charset=utf-8"),
    ("tsv", "text/tab-separated-values; charset=utf-8"),
    ("json", "application/json"),
    ("yaml", PLAIN),
    ("yml", PLAIN),
];

const IMAGE_NAMES: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff", "heic", "avif", "ico",
];

const NEVER: &[&str] = &["svg", "svgz", "html", "htm", "xhtml", "xml", "js", "mjs"];

pub fn is_media(mime: &str) -> bool {
    mime.starts_with("video/") || mime.starts_with("audio/")
}

fn media_format(name: &str) -> Option<&'static Format> {
    let ext = extension(name)?;
    FORMATS
        .iter()
        .find(|f| is_media(f.mime) && f.exts.contains(&ext.as_str()))
}

pub fn is_media_name(name: &str) -> bool {
    media_format(name).is_some()
}

pub fn could_be_media(name: &str, head: &[u8]) -> bool {
    media_format(name).is_none_or(|f| head.len() < 12 || (f.magic)(head))
}

pub fn extension(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() {
        return None;
    }

    Some(ext.to_ascii_lowercase())
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
}

pub fn check(name: &str, bytes: &[u8]) -> Result<Kind, String> {
    let ext = extension(name);
    let ext = ext.as_deref();
    if let Some(e) = ext.filter(|e| NEVER.contains(e)) {
        return Err(format!(
            ".{e} files cannot be uploaded; attach them as a zip archive"
        ));
    }

    let matching: Vec<&Format> = FORMATS.iter().filter(|f| (f.magic)(bytes)).collect();
    let named = matching
        .iter()
        .find(|f| ext.is_some_and(|e| f.exts.contains(&e)));
    if let Some(f) = named.or(matching.first()) {
        return match ext {
            Some(e) if !f.exts.contains(&e) => Err(format!(
                "this file is named .{e} but it is really {}; rename it or upload another file",
                f.what
            )),
            _ => Ok(Kind {
                mime: f.mime,
                image: f.image,
            }),
        };
    }

    match ext {
        Some(e) if IMAGE_NAMES.contains(&e) => Err(format!(
            "this file is named .{e} but it is not an image Tazko can show; upload a PNG, JPEG, GIF or WebP image"
        )),
        Some(e) => match TEXT.iter().find(|(t, _)| *t == e) {
            Some((_, mime)) if is_text(bytes) => Ok(Kind { mime, image: false }),
            Some(_) => Err(format!("this file is named .{e} but it is not plain text")),
            None => Err(format!(".{e} files cannot be uploaded")),
        },
        None if is_text(bytes) => Ok(Kind {
            mime: PLAIN,
            image: false,
        }),
        None => Err("this type of file cannot be uploaded".into()),
    }
}

pub fn clean_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '"' | '\u{202e}' | '\u{202d}'))
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').trim();
    let short: String = trimmed.chars().take(200).collect();
    if short.is_empty() {
        "file".into()
    } else {
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn the_real_type_comes_from_the_bytes() {
        assert!(check("shot.png", PNG).unwrap().image);
        assert_eq!(check("shot", PNG).unwrap().mime, "image/png");
        assert_eq!(
            check("spec.pdf", b"%PDF-1.7\n").unwrap().mime,
            "application/pdf"
        );
        assert!(!check("notes.md", "# Héllo".as_bytes()).unwrap().image);
        assert!(check("README", b"plain words").is_ok());
    }

    #[test]
    fn a_file_renamed_to_look_like_an_image_is_refused() {
        let e = check("photo.png", b"MZ\x90\0this is a program").unwrap_err();
        assert!(e.contains("not an image"), "{e}");
        let e = check("photo.jpg", b"%PDF-1.4").unwrap_err();
        assert!(e.contains("really a PDF"), "{e}");
        let e = check("photo.gif", PNG).unwrap_err();
        assert!(e.contains("really a PNG"), "{e}");
    }

    #[test]
    fn web_pages_scripts_and_programs_are_refused() {
        for name in ["x.svg", "x.html", "x.HTM", "x.js", "x.xml"] {
            assert!(check(name, b"<svg onload=alert(1)>").is_err(), "{name}");
        }
        assert!(check("setup.exe", b"MZ\x90\0").is_err());
        assert!(check("data.csv", b"a,b\0c").is_err());
        assert!(check("blob", b"\0\x01\x02").is_err());
    }

    #[test]
    fn videos_and_sound_get_a_type_browsers_can_play() {
        let mp4 = b"\0\0\0\x18ftypisom\0\0\x02\0";
        assert_eq!(check("clip.mp4", mp4).unwrap().mime, "video/mp4");
        assert_eq!(check("clip.MOV", mp4).unwrap().mime, "video/mp4");
        assert_eq!(check("talk.m4a", mp4).unwrap().mime, "audio/mp4");
        assert_eq!(
            check("clip.webm", b"\x1a\x45\xdf\xa3\x9f\x42\x86\x81")
                .unwrap()
                .mime,
            "video/webm"
        );
        let e = check("clip.webm", mp4).unwrap_err();
        assert!(e.contains("really a video"), "{e}");
        let e = check("clip.mp4", PNG).unwrap_err();
        assert!(e.contains("really a PNG"), "{e}");
        assert!(is_media_name("Demo.MP4") && !is_media_name("demo.png"));
        assert!(could_be_media("a.mp4", mp4) && !could_be_media("a.mp4", &[b'x'; 64]));
        assert!(could_be_media("a.mp4", b"short") && could_be_media("a.png", b"anything"));
    }

    #[test]
    fn names_lose_folders_and_control_characters() {
        assert_eq!(clean_name("../../etc/passwd"), "passwd");
        assert_eq!(clean_name("C:\\Users\\me\\a.png"), "a.png");
        assert_eq!(clean_name("..\u{0}"), "file");
        assert_eq!(clean_name("re\u{202e}gnp.exe"), "regnp.exe");
        assert_eq!(clean_name(".hidden.txt"), "hidden.txt");
        assert_eq!(clean_name(&"a".repeat(300)).len(), 200);
    }
}
