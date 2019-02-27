#![no_std]
mod utils;

use cfg_if::cfg_if;
use parser::OutlineWriter;
use wasm_bindgen::prelude::*;

cfg_if! {
    // When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
    // allocator.
    if #[cfg(feature = "wee_alloc")] {
        extern crate wee_alloc;
        #[global_allocator]
        static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
    }
}

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
    fn outlineLine(text: &str);
}

#[wasm_bindgen]
pub fn greet() {
    alert("Hello from Rust!");
}

struct JsWriter;

impl OutlineWriter for JsWriter {
    fn start_line(&mut self, depth: i32) {}

    fn text(&mut self, text: &str) {
        outlineLine(text);
    }
}

#[wasm_bindgen]
pub fn parse(input: &str) {
    JsWriter.parse(input);
}
