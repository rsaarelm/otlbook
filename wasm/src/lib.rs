#![no_std]
mod utils;

use cfg_if::cfg_if;
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

#[wasm_bindgen]
pub fn parse(input: &str) -> usize {
    // TODO: Put something useful here.
    // Now there's just a code that forces the parser to get compiled so I can track the generated
    // wasm size.
    parser::Lexer::new(input).count()
}
