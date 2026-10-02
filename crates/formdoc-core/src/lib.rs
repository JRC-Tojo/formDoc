//! formDoc のコア。IOを持たず、Web版（wasm）とデスクトップ版（Tauri）の両方から同じコードで使う。

pub mod api;
pub mod codegen;
pub mod compile;
pub mod evaluate;
pub mod lint;
pub mod model;
pub mod template;
pub mod world;

pub use api::Session;
pub use compile::{Compiled, Diagnostic, compile, render_pdf, render_svgs};
pub use model::Document;
pub use world::{FormdocWorld, font_families};
