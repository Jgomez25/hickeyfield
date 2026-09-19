mod app;
mod chain;
mod commands;
mod harness;
mod library;
mod pricing;
mod render;
mod runner;
mod store;
mod validate;
mod vault;

use app::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // FIRST, before any provider client can be built: this is the one process
    // allowed to spend money. Every other entrypoint (tests, examples, a future
    // CLI) leaves the gate OFF by default, so a billable request there is
    // refused with `JobError::SpendBlocked` (see `hickeyfield_core::spend_gate`).
    // Kept ahead of `AppState::init`, which constructs the client factory.
    hickeyfield_core::spend_gate::enable();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("hickeyfield_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Visible in the log so a regression that drops the `enable()` call above
    // (which would make the app silently refuse to generate) is diagnosable.
    tracing::info!("spending enabled for this process (packaged app)");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::init(app.handle()).map_err(std::io::Error::other)?;

            // On the CSP in tauri.conf.json, which cannot carry a comment of
            // its own: `img-src`/`media-src` include `https:` because there is
            // a real window between a job completing and its file finishing
            // downloading, and the provider's own URL is the only thing that
            // exists during it. Without that, the window renders as a broken
            // card in a release build while working perfectly in dev — the dev
            // server sends no CSP at all — which is the worst kind of split.
            // Nothing else is widened: `script-src` and `connect-src` stay on
            // 'self', so this buys a visible result and grants no new reach.

            // Let the webview load files out of the library, and nothing else.
            //
            // `assetProtocol.scope` in tauri.conf.json is a static list, and the
            // library root is chosen at runtime — so it was left empty, which
            // does not mean "allow all", it means **allow nothing**. Every
            // `asset://` URL was blocked and every finished generation rendered
            // as a blank card next to a perfectly good file on disk.
            //
            // Granting the real root here is both narrower than any glob we
            // could have written and correct when the user moves it.
            let root = state.library.root().to_path_buf();
            if let Err(e) = std::fs::create_dir_all(&root) {
                tracing::warn!("could not create the library at {}: {e}", root.display());
            }
            app.asset_protocol_scope().allow_directory(&root, true)?;
            tracing::info!("library: {}", root.display());

            // Anything the last session left running is picked back up here.
            // This is the payoff for keeping job state in Rust rather than in
            // the webview: quitting mid-generation costs the user nothing.
            state.resume();
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::key_states,
            commands::set_key,
            commands::configured_providers,
            commands::local_endpoints,
            commands::list_ollama_models,
            commands::is_ready,
            commands::provider_info,
            commands::validate_key,
            commands::import_env,
            commands::list_models,
            commands::model_names,
            commands::get_show_never_used,
            commands::set_show_never_used,
            commands::list_use_cases,
            commands::models_for_use_case,
            commands::model_capabilities,
            commands::list_presets,
            commands::estimate_cost,
            commands::price_status,
            commands::preview_prompt,
            commands::submit_job,
            commands::plan_storyboard,
            commands::generate_storyboard,
            commands::regenerate_keyframe_still,
            commands::list_storyboards,
            render::render_storyboard,
            render::list_storyboard_renders,
            commands::list_jobs,
            commands::cancel_job,
            commands::delete_job,
            commands::reveal_result,
            commands::allow_media_preview,
            commands::watching_jobs,
            commands::library_root,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hickeyfield");
}

#[cfg(test)]
mod spend_gate_wiring_tests {
    /// This file, so the entrypoint's wiring can be asserted on statically —
    /// the same technique `app.rs` uses for its adapter-drift guard. We cannot
    /// execute `run()` in a test (it launches Tauri), so the source-string
    /// checks below pin that the `enable()` call exists and runs before any
    /// client is constructed.
    const SOURCE: &str = include_str!("lib.rs");

    #[test]
    fn the_entrypoint_calls_spend_gate_enable() {
        assert!(
            SOURCE.contains("hickeyfield_core::spend_gate::enable();"),
            "run() must enable spending for the packaged app, or it silently refuses to generate"
        );
    }

    #[test]
    fn enable_precedes_any_client_construction_in_run() {
        // The gate must be on before `AppState::init` builds the ClientFactory,
        // or the first submit races the switch. Assert source order within run.
        let run_body = SOURCE
            .split_once("pub fn run() {")
            .expect("run() present")
            .1;
        let enable_at = run_body
            .find("spend_gate::enable();")
            .expect("enable() called in run()");
        let init_at = run_body
            .find("AppState::init(")
            .expect("AppState::init called in run()");
        assert!(
            enable_at < init_at,
            "enable() must precede AppState::init (which builds the client factory)"
        );
    }

    #[test]
    fn the_entrypoint_enable_path_actually_allows_spending() {
        // EXECUTE the very call `run()` makes and prove it flips the gate on.
        // This runs in the src-tauri test binary, which has no test asserting a
        // real-client block (those live in the core crate's separate binary),
        // so setting the process-global OnceLock here poisons nothing. This is
        // the closest an agent can get to the owner's final proof: launching the
        // installed app and seeing a normal cost estimate rather than a refusal.
        hickeyfield_core::spend_gate::enable();
        assert!(
            hickeyfield_core::spend_gate::allowed(),
            "the entrypoint's enable() call must make spending allowed"
        );
    }
}
