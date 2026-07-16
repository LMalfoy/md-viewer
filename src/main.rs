use std::ffi::OsString;
use std::process::ExitCode;

use md_viewer::app::ViewerApp;
use md_viewer::args::{HELP, LaunchAction, parse_args};

fn main() -> ExitCode {
    let action = match parse_args(std::env::args_os().skip(1).collect::<Vec<OsString>>()) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("md-viewer: {error}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };

    let initial_path = match action {
        LaunchAction::Help => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        LaunchAction::Version => {
            println!("md-viewer {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        LaunchAction::Run { file } => file,
    };

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("MD Viewer")
            .with_app_id("md-viewer")
            .with_inner_size([1024.0, 760.0])
            .with_min_inner_size([480.0, 320.0])
            .with_resizable(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    match eframe::run_native(
        "MD Viewer",
        options,
        Box::new(move |creation_context| {
            Ok(Box::new(ViewerApp::new(creation_context, initial_path)))
        }),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("md-viewer: failed to start graphical interface: {error}");
            ExitCode::FAILURE
        }
    }
}
