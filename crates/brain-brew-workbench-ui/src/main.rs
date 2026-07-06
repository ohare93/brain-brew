#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(|| leptos::view! { <brain_brew_workbench_ui::App /> });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
