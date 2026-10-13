fn main() {
    // Embed the app icon in hangar.exe so Explorer, the taskbar and the Start Menu shortcut show it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/hangar.ico");
        res.compile().expect("embedding the Windows icon");
    }
}
