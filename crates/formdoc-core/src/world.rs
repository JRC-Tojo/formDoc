//! Typst の World 実装。
//!
//! - プロジェクトのファイルはメモリ上（GUIの生成結果、またはコードモードのフォルダ内容）
//! - パッケージは埋め込みライブラリのみ（@local/formdoc, @preview/cetz など同梱分）。ネットワークは使わない
//! - フォントは同梱分のみ（システムフォントは使わない）
//! これにより Web版・デスクトップ版・誰のPCでも同じPDFになる。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use typst::diag::{FileError, FileResult, PackageError};
use typst::foundations::{Bytes, Datetime, Dict, IntoValue};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};

/// 同梱フォント（プロセス内で1回だけ読み込む）。
struct FontStore {
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn font_store() -> &'static FontStore {
    static STORE: OnceLock<FontStore> = OnceLock::new();
    STORE.get_or_init(|| {
        let mut fonts = Vec::new();
        for data in typst_assets::fonts().chain(formdoc_library::fonts()) {
            fonts.extend(Font::iter(Bytes::new(data)));
        }
        let book = FontBook::from_fonts(&fonts);
        FontStore { book: LazyHash::new(book), fonts }
    })
}

/// 同梱フォントのファミリー名一覧（テンプレート開発時の確認用）。
pub fn font_families() -> Vec<String> {
    let mut v: Vec<String> = font_store().fonts.iter().map(|f| f.info().family.clone()).collect();
    v.sort();
    v.dedup();
    v
}

pub fn main_id() -> FileId {
    project_id("/main.typ")
}

pub fn project_id(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).unwrap_or_else(|_| VirtualPath::new("/main.typ").unwrap());
    RootedPath::new(VirtualRoot::Project, vpath).intern()
}

pub struct FormdocWorld {
    library: LazyHash<Library>,
    main: FileId,
    /// プロジェクトのファイル（パスは "/main.typ" のように先頭スラッシュ付き）
    files: HashMap<String, Bytes>,
    /// 解析済みソースの再利用（再コンパイルの高速化）
    sources: Mutex<HashMap<FileId, Source>>,
    today: Option<Datetime>,
}

impl FormdocWorld {
    pub fn new() -> Self {
        Self {
            library: LazyHash::new(Library::default()),
            main: main_id(),
            files: HashMap::new(),
            sources: Mutex::new(HashMap::new()),
            today: None,
        }
    }

    /// `sys.inputs` に渡す値を設定する。
    pub fn set_inputs(&mut self, inputs: &[(&str, &str)]) {
        let mut d = Dict::new();
        for (k, v) in inputs {
            d.insert((*k).into(), (*v).into_value());
        }
        self.library = LazyHash::new(Library::builder().with_inputs(d).build());
    }

    /// 文書の日付を固定する（PDFの再現性のため、実行日時は使わない）。
    pub fn set_today(&mut self, ymd: Option<(i32, u8, u8)>) {
        self.today = ymd.and_then(|(y, m, d)| Datetime::from_ymd(y, m, d));
    }

    /// プロジェクトのファイル一式を置き換える。テキストが変わらないソースは解析結果を再利用する。
    pub fn set_files(&mut self, files: impl IntoIterator<Item = (String, Vec<u8>)>) {
        self.files = files
            .into_iter()
            .map(|(p, b)| (if p.starts_with('/') { p } else { format!("/{p}") }, Bytes::new(b)))
            .collect();
        let mut sources = self.sources.lock().unwrap();
        sources.retain(|id, _| match id.get().root() {
            VirtualRoot::Project => self.files.contains_key(id.get().vpath().get_with_slash()),
            _ => true,
        });
    }

    pub fn set_main(&mut self, path: &str) {
        self.main = project_id(path);
    }

    fn read(&self, id: FileId) -> FileResult<Bytes> {
        let rooted = id.get();
        let vpath = rooted.vpath().get_without_slash().to_string();
        match rooted.root() {
            VirtualRoot::Project => self
                .files
                .get(rooted.vpath().get_with_slash())
                .cloned()
                .ok_or_else(|| FileError::NotFound(vpath.into())),
            VirtualRoot::Package(spec) => {
                let base = match (spec.namespace.as_str(), spec.name.as_str()) {
                    ("local", "formdoc") => format!("typst/formdoc/{}", spec.version),
                    ("preview", name) => format!("vendor/preview/{name}/{}", spec.version),
                    _ => return Err(FileError::Package(PackageError::NotFound(spec.clone()))),
                };
                if formdoc_library::get(&format!("{base}/typst.toml")).is_none() {
                    return Err(FileError::Package(PackageError::NotFound(spec.clone())));
                }
                formdoc_library::get(&format!("{base}/{vpath}"))
                    .map(Bytes::new)
                    .ok_or_else(|| FileError::NotFound(vpath.into()))
            }
        }
    }
}

impl Default for FormdocWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl World for FormdocWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &font_store().book
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        let bytes = self.read(id)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| FileError::InvalidUtf8)?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut sources = self.sources.lock().unwrap();
        match sources.get_mut(&id) {
            Some(src) => {
                if src.text() != text {
                    src.replace(text);
                }
                Ok(src.clone())
            }
            None => {
                let src = Source::new(id, text.to_string());
                sources.insert(id, src.clone());
                Ok(src)
            }
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        self.read(id)
    }

    fn font(&self, index: usize) -> Option<Font> {
        font_store().fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<typst::foundations::Duration>) -> Option<Datetime> {
        self.today
    }
}
