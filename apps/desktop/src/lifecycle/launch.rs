use crate::{Workspace, evaluation, home, latency, live, notifications, spec};

pub(crate) fn run() {
    let mut fixtures = false;
    let mut evaluation = false;
    let mut design = false;
    let mut project = None;
    let mut native: Option<caelum_core::remote::native::NativeTarget> = None;
    let mut latency_workload = None;
    let mut args = Vec::new();
    let mut input = std::env::args().skip(1);
    while let Some(arg) = input.next() {
        match arg.as_str() {
            #[cfg(target_os = "linux")]
            "--install" => {
                super::install::run().unwrap_or_else(|error| fail(&error));
                return;
            }
            "--version" | "-V" => {
                println!("Caelum {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--fixtures" => fixtures = true,
            "--evaluation-workload" => evaluation = true,
            "--design-fixture" => design = true,
            "--boat-workspace" => {
                let target: caelum_core::remote::native::NativeTarget = input
                    .next()
                    .and_then(|v| serde_json::from_str(&v).ok())
                    .unwrap_or_else(|| fail("Expected a Boat workspace identity"));
                target.root().unwrap_or_else(|e| fail(&e));
                native = Some(target);
            }
            "--latency-workload" => {
                latency_workload = input.next().as_deref().and_then(latency::Scenario::parse);
                if latency_workload.is_none() {
                    fail(
                        "--latency-workload requires small, streaming, scroll or scroll-streaming",
                    );
                }
            }
            "--project" | "--open-url" => {
                let Some(value) = input.next() else {
                    fail("--project/--open-url requires a directory or project link");
                };
                project = Some(project_argument(&value).unwrap_or_else(|error| fail(&error)));
            }
            _ if args.is_empty() && project.is_none() && !arg.starts_with('-') => {
                project = Some(project_argument(&arg).unwrap_or_else(|error| fail(&error)));
            }
            _ => args.push(arg),
        }
    }
    let _capture = std::env::var_os("CAELUM_LATENCY_TRACE")
        .filter(|path| !path.is_empty())
        .and_then(|path| {
            match caelum_core::diagnostics::Capture::start(std::path::Path::new(&path)) {
                Ok(capture) => Some(capture),
                Err(error) => {
                    eprintln!("Cannot start latency trace: {error}");
                    None
                }
            }
        });
    #[cfg(target_os = "linux")]
    rune::set_app_id("dev.caelum.desktop");
    let app = super::configure(
        rune::App::new(spec())
            .compositor(home::art::Compositor::default())
            .font_smoothing(rune::FontSmoothing::Off)
            .on_frame_drawn(live::frame_drawn),
    );
    let app = if caelum_core::diagnostics::enabled() {
        app.on_frame_encoded(latency::encoded)
    } else {
        app
    };
    let handle = app.handle();
    let production = !fixtures && !evaluation && !design && latency_workload.is_none();
    #[cfg(target_os = "linux")]
    let _instance = if production {
        let local = caelum_core::application::Store::default_path();
        let store = native
            .as_ref()
            .map(|n| n.store_path(&local))
            .unwrap_or(local);
        match super::linux::start_for_state(handle.clone(), project.clone(), &store) {
            Ok(super::linux::Launch::Primary(instance)) => Some(instance),
            Ok(super::linux::Launch::Forwarded) => return,
            Err(error) => fail(&error),
        }
    } else {
        None
    };
    let notifications = notifications::State::connect(handle.clone(), production);
    let restart = crate::updates::RestartSlot::default();
    let restart_slot = restart.clone();
    app.init(move || {
        let mut state = if let Some(scenario) = latency_workload {
            latency::workspace(scenario)
        } else if design {
            evaluation::design()
        } else if evaluation {
            evaluation::workspace()
        } else if fixtures {
            Workspace::new()
        } else if let Some(target) = native {
            crate::remote::native::workspace(target)
        } else {
            home::workspace(project)
        };
        if let Some(live) = &mut state.live {
            live.notifications = notifications;
            live.updates.restart = restart_slot;
        }
        state.handle = Some(handle);
        state
    })
    .args(args)
    .chrome(super::chrome())
    .run();
    #[cfg(target_os = "linux")]
    drop(_instance);
    crate::updates::relaunch(restart);
}

fn project_argument(value: &str) -> Result<String, String> {
    let path = if value.starts_with("caelum:") {
        caelum_core::desktop::project_from_url(value)?
    } else {
        std::path::absolute(value).map_err(|error| error.to_string())?
    };
    path.to_str()
        .filter(|value| !value.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| "Expected a UTF-8 project path without NUL characters".into())
}

fn fail(message: &str) -> ! {
    eprintln!("Caelum: {message}");
    std::process::exit(2);
}

#[cfg(test)]
mod tests;
