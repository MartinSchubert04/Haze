// La interfaz no llama al plugin directo: todo pasa por los comandos de la app.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).android_path("android").build();
}
