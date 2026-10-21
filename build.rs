// Mengompilasi `ui/main.slint` menjadi kode Rust yang dimuat oleh `slint::include_modules!()` di `src/main.rs`.
fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
