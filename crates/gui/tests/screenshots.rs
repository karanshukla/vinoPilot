//! The README's pictures, from a fixed machine state with wgpu:
//! `cargo test -p vinopilot-gui --test screenshots -- --ignored`, then commit
//! docs/images.

use std::collections::HashMap;

use egui_kittest::Harness;
use vino_ui::{App, egui, theme};
use vinopilot_core::Config;
use vinopilot_core::catalog::{Channel, Id};
use vinopilot_core::detect::{Checkout, Installed, Local, SystemPackage};
use vinopilot_core::remote::Latest;
use vinopilot_gui::Pilot;

const SCALE: f32 = 1.5;
const MARGIN: f32 = 8.0;

fn installed(channel: Channel, version: &str) -> Option<Installed> {
    Some(Installed {
        channel,
        version: Some(version.into()),
    })
}

fn demo() -> Pilot {
    let root = std::path::PathBuf::from("/home/you/Development");
    let checkout = |name: &str, describe: &str| {
        Some(Checkout {
            path: root.join(name),
            describe: describe.into(),
        })
    };
    let local = HashMap::from([
        (
            Id::App(App::Whisper),
            Local {
                installed: installed(Channel::Release, "1.3.0"),
                checkout: None,
                system_package: None,
            },
        ),
        (
            Id::App(App::AuthFace),
            Local {
                installed: installed(Channel::Release, "12"),
                checkout: None,
                system_package: None,
            },
        ),
        (
            Id::App(App::OpenEffects),
            Local {
                installed: installed(Channel::Source, "4"),
                checkout: checkout("vinoOpenEffects", "v5"),
                system_package: Some(SystemPackage {
                    kind: "rpm",
                    packages: vec!["vinoopeneffects".into()],
                    remove: "sudo dnf remove vinoopeneffects".into(),
                }),
            },
        ),
        (
            Id::App(App::Pop),
            Local {
                installed: installed(Channel::Source, "1.4.0"),
                checkout: checkout("vinoPop", "v1.4.0-13-g2f1c9e0"),
                system_package: None,
            },
        ),
        (
            Id::App(App::Speak),
            Local {
                installed: installed(Channel::Package, "0.5.1"),
                checkout: None,
                system_package: None,
            },
        ),
        (
            Id::App(App::Search),
            Local {
                installed: installed(Channel::Release, "8.0.0"),
                checkout: None,
                system_package: None,
            },
        ),
        (
            Id::Ovfetch,
            Local {
                installed: installed(Channel::Package, "0.6.0"),
                checkout: None,
                system_package: None,
            },
        ),
        (
            Id::Pilot,
            Local {
                installed: installed(Channel::Release, vinopilot_gui::VERSION),
                checkout: None,
                system_package: None,
            },
        ),
    ]);
    let release = |tag: &str| Some(Ok(tag.to_string()));
    let latest = HashMap::from([
        (
            Id::App(App::Whisper),
            Latest {
                release: release("v1.3.1"),
                ..Default::default()
            },
        ),
        (
            Id::App(App::AuthFace),
            Latest {
                release: release("v12"),
                ..Default::default()
            },
        ),
        (
            Id::App(App::Pop),
            Latest {
                behind: Some(Ok(13)),
                ..Default::default()
            },
        ),
        (
            Id::App(App::Speak),
            Latest {
                package: Some(Ok("0.5.1".into())),
                ..Default::default()
            },
        ),
        (
            Id::App(App::Search),
            Latest {
                release: release("v8"),
                ..Default::default()
            },
        ),
        (
            Id::Ovfetch,
            Latest {
                package: Some(Ok("0.6.0".into())),
                ..Default::default()
            },
        ),
        (
            Id::Pilot,
            Latest {
                release: release(&format!("v{}", vinopilot_gui::VERSION)),
                ..Default::default()
            },
        ),
    ]);
    Pilot::with(Config { source_root: root }, local, latest)
}

fn render(dark: bool, width: f32, height: f32) -> image::RgbaImage {
    let mut h = Harness::builder()
        .with_size((width + 2.0 * MARGIN, height + 2.0 * MARGIN))
        .with_pixels_per_point(SCALE)
        .wgpu()
        .build_ui_state(
            move |ui, (pilot, styled): &mut (Pilot, bool)| {
                if !*styled {
                    theme::apply(ui.ctx());
                    ui.ctx().set_theme(if dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    ui.ctx().all_styles_mut(|style| style.animation_time = 0.0);
                    *styled = true;
                    ui.ctx().request_repaint();
                    return;
                }
                pilot.ui(ui);
            },
            (demo(), false),
        );
    h.run_steps(4);
    let full = h.render().expect("wgpu renders");
    let m = (MARGIN * SCALE) as u32;
    image::imageops::crop_imm(&full, m, m, full.width() - 2 * m, full.height() - 2 * m).to_image()
}

fn save(image: &image::RgbaImage, name: &str) {
    let path = format!(
        "{}/../../docs/images/{name}.png",
        env!("CARGO_MANIFEST_DIR")
    );
    image.save(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
}

#[test]
#[ignore = "needs a GPU or lavapipe; regenerates docs/images"]
fn screenshots() {
    save(&render(true, 1000.0, 720.0), "window-dark");
    save(&render(false, 1000.0, 720.0), "window-light");
    save(&render(true, 420.0, 720.0), "narrow-dark");
}
