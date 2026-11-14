//! Measure real review tasks in a fresh process, with an explicitly isolated store.
use caelum_core::application::{Command, Event, Reply, Runtime, Store, Workspace};
use serde_json::json;
use std::{
    collections::HashSet,
    sync::mpsc,
    time::{Duration, Instant},
};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: review_load PROJECT PR_URL ISOLATED_WORKSPACE_JSON".into());
    }
    let (wake, ready) = mpsc::channel();
    let mut runtime = Runtime::new(move || {
        let _ = wake.send(());
    });
    runtime.submit(0, Command::Load(Store((&args[2]).into())))?;
    loop {
        ready
            .recv_timeout(Duration::from_secs(30))
            .map_err(|e| e.to_string())?;
        if let Some(result) = runtime.drain().into_iter().find_map(|event| {
            if let Event::Completed(0, result) = event {
                Some(result)
            } else {
                None
            }
        }) {
            result?;
            break;
        }
    }
    let mut app = Workspace::default();
    let owner = (args[0].clone(), args[1].clone());
    let started = Instant::now();
    app.open_review(owner.0.clone(), owner.1.clone());
    let mut pending = HashSet::new();
    let mut overview = None;
    let mut files = None;
    let mut fresh = None;
    let mut cached = false;
    loop {
        for (id, command) in app.commands() {
            pending.insert(id);
            runtime.submit(id, command)?;
        }
        if pending.is_empty() {
            break;
        }
        ready
            .recv_timeout(Duration::from_secs(30))
            .map_err(|e| e.to_string())?;
        for event in runtime.drain() {
            if let Event::Completed(id, result) = &event {
                pending.remove(id);
                if let Err(error) = result {
                    return Err(error.clone());
                }
                if matches!(result, Ok(Reply::Task(_))) {
                    app.apply(event);
                }
            }
            let review = &app.services.reviews[&owner];
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            if review.details.is_some() && overview.is_none() {
                overview = Some(ms);
                cached = review.cached();
            }
            if review.files_loaded && files.is_none() {
                files = Some(ms);
            }
            if review.details.is_some() && !review.cached() && fresh.is_none() {
                fresh = Some(ms);
            }
        }
    }
    let review = &app.services.reviews[&owner];
    println!(
        "{}",
        json!({
            "overview_ms": overview, "files_ms": files, "fresh_details_ms": fresh,
            "all_first_pages_ms": started.elapsed().as_secs_f64() * 1000.0,
            "cache_shown": cached, "loaded_files": review.files.len(),
        })
    );
    Ok(())
}
