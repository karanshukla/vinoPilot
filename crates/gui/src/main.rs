use eframe::egui;
use vino_ui::theme;
use vinopilot_gui::{Pilot, VERSION, desktop};

fn main() -> eframe::Result {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("vinopilot-gui {VERSION}");
            return Ok(());
        }
        // The app menu entry, for installs that aren't install.sh's.
        Some("--install-desktop") => {
            let exe = std::env::current_exe().expect("this binary's path");
            match desktop::install(&exe) {
                Ok(path) => println!("Added vinoPilot to the app menu: {}", path.display()),
                Err(e) => {
                    eprintln!("✗ couldn't add the app menu entry: {e}");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        Some("--uninstall-desktop") => {
            if let Err(e) = desktop::uninstall() {
                eprintln!("✗ couldn't remove the app menu entry: {e}");
                std::process::exit(1);
            }
            return Ok(());
        }
        Some("-h" | "--help") => {
            println!("vinopilot-gui [--install-desktop | --uninstall-desktop | --version]");
            return Ok(());
        }
        _ => {}
    }
    let app_title_bar = vino_ui::window::app_title_bar();
    let options = eframe::NativeOptions {
        viewport: vino_ui::window::viewport(
            egui::ViewportBuilder::default()
                .with_title("vinoPilot")
                .with_app_id("io.github.karanshukla.vinopilot")
                .with_inner_size([1000.0, 720.0])
                .with_min_inner_size([360.0, 360.0]),
            app_title_bar,
        ),
        ..Default::default()
    };
    eframe::run_native(
        "vinoPilot",
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            Ok(Box::new(
                Pilot::new(&cc.egui_ctx).with_app_title_bar(app_title_bar),
            ))
        }),
    )
}
