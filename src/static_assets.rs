#[cfg(not(debug_assertions))]
mod embed {

    pub struct EmbeddedFile {
        name: &'static str,
        contents: &'static str,
    }

    impl EmbeddedFile {
        fn name(&self) -> &str {
            &self.name
        }
        fn load(&self) -> &str {
            &self.contents
        }
    }
    static FILES: &'static [EmbeddedFile] = &[
        EmbeddedFile {
            name: "/static/index.html",
            contents: include_str!("../static/index.html"),
        },
        EmbeddedFile {
            name: "/static/modern-normalize.min.css",
            contents: include_str!("../static/modern-normalize.min.css"),
        },
        EmbeddedFile {
            name: "/static/bundle.js",
            contents: include_str!("../static/bundle.js"),
        },
    ];

    pub fn load_file(name: &str) -> Option<&'static str> {
        for file in FILES {
            if file.name() == name {
                return Some(file.load());
            }
        }
        None
    }
}

#[cfg(debug_assertions)]
mod external {
    use std::path::PathBuf;
    use urlencoding::decode;

    fn validate_path(p: &str) -> Option<PathBuf> {
        let p = match decode(p) {
            Ok(p) => p,
            Err(_) => return None,
        };
        let mut buf = std::env::current_dir().unwrap();

        for seg in p.split('/') {
            if seg.starts_with("..") || seg.contains('\\') {
                return None;
            } else {
                buf.push(seg);
            }
        }
        Some(buf)
    }

    pub async fn load_file(name: &str) -> Option<String> {
        if let Some(p) = validate_path(name) {
            tokio::fs::read_to_string(&p).await.ok()
        } else {
            None
        }
    }
}

#[cfg(not(debug_assertions))]
pub async fn load_file(name: &str) -> Option<String> {
    embed::load_file(name).map(String::from)
}
#[cfg(debug_assertions)]
pub async fn load_file(name: &str) -> Option<String> {
    external::load_file(name).await
}
