//! Typst の World 実装。
//!
//! - プロジェクトのファイルはメモリ上（GUIの生成結果、またはコードモードのフォルダ内容）
//! - パッケージは埋め込みライブラリのみ（@local/formdoc, @preview/cetz など同梱分）。ネットワークは使わない
//! - フォントは同梱分のみ（システムフォントは使わない）。Web版は同梱フォントを実行時に受け取る（[`install_fonts`]）
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

static FONT_STORE: OnceLock<FontStore> = OnceLock::new();

impl FontStore {
    /// 埋め込みフォントのあとに、実行時に渡されたフォントを並べる。
    /// 並び順はフォントの番号になるため、どちらも library/fonts のファイル名順にする。
    fn new(extra: Vec<Vec<u8>>) -> Self {
        let mut fonts = Vec::new();
        for data in formdoc_library::fonts() {
            fonts.extend(Font::iter(Bytes::new(data)));
        }
        for data in extra {
            fonts.extend(Font::iter(Bytes::new(data)));
        }
        let book = FontBook::from_fonts(&fonts);
        FontStore { book: LazyHash::new(book), fonts }
    }
}

fn font_store() -> &'static FontStore {
    FONT_STORE.get_or_init(|| FontStore::new(Vec::new()))
}

/// フォントを埋め込んでいない版（Web版）で、実行時に取得した同梱フォントを渡す。
///
/// `fonts` は [`formdoc_library::font_files`] と同じ順に並べた中身。最初の組版より前に1回だけ呼ぶ
/// （組版で一度使われたフォントの一覧は差し替えられない。同じ文書が同じPDFになることを守るため）。
/// 既にフォントを使い始めていた場合や、渡されたフォントが同梱フォントの一覧（数・大きさ）と合わない場合はエラーを返す
/// （フォントの番号がずれると、同じ文書でもPDFが変わるため）。
pub fn install_fonts(fonts: Vec<Vec<u8>>) -> Result<(), String> {
    if FONT_STORE.get().is_some() {
        return Err("フォントは既に読み込まれています（最初の組版より前に渡してください）".into());
    }
    let list: Vec<_> = formdoc_library::font_files().iter().filter(|f| f.data.is_none()).collect();
    if fonts.len() != list.len() || fonts.iter().zip(&list).any(|(d, f)| d.len() != f.size) {
        return Err(format!("渡されたフォント（{} 個）が同梱フォントの一覧（{} 個）と合いません", fonts.len(), list.len()));
    }
    FONT_STORE
        .set(FontStore::new(fonts))
        .map_err(|_| "フォントは既に読み込まれています（最初の組版より前に渡してください）".to_string())
}

/// 同梱フォントのファミリー名一覧（文書テンプレート開発時の確認用）。
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
