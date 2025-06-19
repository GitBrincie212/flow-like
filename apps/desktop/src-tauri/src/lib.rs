mod functions;
mod profile;
mod settings;
mod state;
mod utils;
use flow_like::{
    flow_like_storage::{
        Path,
        files::store::{FlowLikeStore, local_store::LocalObjectStore},
        lancedb,
    },
    state::{FlowLikeConfig, FlowLikeState},
    utils::http::HTTPClient,
};
use flow_like_types::{sync::Mutex, tokio::time::interval};
use serde_json::json;
use settings::Settings;
use state::TauriFlowLikeState;
use std::{sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::{DeepLinkExt, OpenUrlEvent};
use tracing_subscriber::prelude::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut settings_state = Settings::new();
    let project_dir = settings_state.project_dir.clone();
    let logs_dir = settings_state.logs_dir.clone();
    let temporary_dir = settings_state.temporary_dir.clone();

    let mut config: FlowLikeConfig = FlowLikeConfig::new();
    config.register_bits_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(settings_state.bit_dir.clone()).unwrap(),
    )));

    config.register_user_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(settings_state.user_dir.clone()).unwrap(),
    )));

    config.register_app_storage_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(project_dir.clone()).unwrap(),
    )));

    config.register_app_meta_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(project_dir.clone()).unwrap(),
    )));

    config.register_log_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(logs_dir.clone()).unwrap(),
    )));

    config.register_temporary_store(FlowLikeStore::Local(Arc::new(
        LocalObjectStore::new(temporary_dir.clone()).unwrap(),
    )));

    config.register_build_project_database(Arc::new(move |path: Path| {
        let directory = project_dir.join(path.to_string());
        lancedb::connect(directory.to_str().unwrap())
    }));

    config.register_build_logs_database(Arc::new(move |path: Path| {
        let directory = logs_dir.join(path.to_string());
        lancedb::connect(directory.to_str().unwrap())
    }));

    settings_state.set_config(&config);
    let settings_state = Arc::new(Mutex::new(settings_state));
    let (http_client, refetch_rx) = HTTPClient::new();
    let state = FlowLikeState::new(config, http_client);
    let state_ref = Arc::new(Mutex::new(state));

    let initialized_state = state_ref.clone();
    tauri::async_runtime::spawn(async move {
        let weak_ref = Arc::downgrade(&initialized_state);
        let catalog = flow_like_catalog::get_catalog().await;
        let state = initialized_state.lock().await;
        let registry_guard = state.node_registry.clone();
        drop(state);
        let mut registry = registry_guard.write().await;
        registry.initialize(weak_ref);
        registry.push_nodes(catalog).await.unwrap();
        println!("Catalog Initialized");
    });

    let sentry_endpoint = std::option_env!("PUBLIC_SENTRY_ENDPOINT");
    let guard = sentry_endpoint.map(|endpoint| {
        sentry::init((
            endpoint,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                auto_session_tracking: true,
                traces_sample_rate: 0.1,
                ..Default::default()
            },
        ))
    });

    match guard {
        Some(_) => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer())
                .with(sentry_tracing::layer())
                .init();

            println!("Sentry Tracing Layer Initialized");
        }
        None => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer())
                .init();

            println!("Sentry Tracing Layer Not Initialized");
        }
    };

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let relay_handle = app.app_handle().clone();
            let gc_handle = relay_handle.clone();
            let refetch_handle = relay_handle.clone();
            let deep_link_handle = relay_handle.clone();

            #[cfg(desktop)]
            {
                use tauri_plugin_window_state::StateFlags;

                if let Err(e) = app.handle().plugin(
                    tauri_plugin_window_state::Builder::default()
                        .with_state_flags(StateFlags::all())
                        .build(),
                ) {
                    eprintln!("Failed to register window state plugin: {}", e);
                } else {
                    println!("Window state plugin registered successfully");
                }
            }

            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                app.deep_link().register_all()?;
            }

            app.deep_link().on_open_url(move |event| {
                let deep_link_handle = deep_link_handle.clone();
                handle_deep_link(&deep_link_handle, event);
            });

            tauri::async_runtime::spawn(async move {
                let handle = gc_handle;

                let model_factory = {
                    println!("Starting GC");
                    let flow_like_state = TauriFlowLikeState::construct(&handle).await.unwrap();
                    let flow_like_state = flow_like_state.lock().await;

                    flow_like_state.model_factory.clone()
                };
                println!("GC Started");

                let mut interval = interval(Duration::from_secs(1));

                loop {
                    interval.tick().await;

                    {
                        let state = model_factory.try_lock();
                        if let Ok(mut state) = state {
                            state.gc();
                        }
                    }
                }
            });

            tauri::async_runtime::spawn(async move {
                let mut receiver = refetch_rx;
                let handle = refetch_handle;

                let http_client = {
                    println!("Starting Refetch Handler");
                    let flow_like_state = TauriFlowLikeState::construct(&handle).await.unwrap();
                    let flow_like_state = flow_like_state.lock().await;
                    flow_like_state.http_client.clone()
                };

                let client = http_client.client();

                println!("Refetch Handler Started");
                while let Some(event) = receiver.recv().await {
                    let request = event;
                    let request_hash = http_client.quick_hash(&request);
                    let response = match client.execute(request).await {
                        Ok(response) => response,
                        Err(e) => {
                            eprintln!("Error fetching request: {:?}", e);
                            continue;
                        }
                    };

                    let value = match response.json::<serde_json::Value>().await {
                        Ok(value) => value,
                        Err(e) => {
                            eprintln!("Error parsing response: {:?}", e);
                            continue;
                        }
                    };

                    match http_client.put(&request_hash, &value) {
                        Ok(result) => result,
                        Err(e) => {
                            eprintln!("Error putting value in cache: {:?}", e);
                            continue;
                        }
                    };
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state::TauriSettingsState(settings_state))
        .manage(state::TauriFlowLikeState(state_ref))
        .on_page_load(|view, payload| {
            let label = view.label();
            let app_handle = view.app_handle();
            let main_window = app_handle.get_webview_window("main");

            if let Some(main_window) = main_window {
                if label == "oidcFlow" {
                    let res = main_window.emit(
                        "oidc/url",
                        json!({
                            "url": payload.url(),
                        }),
                    );

                    if let Err(e) = res {
                        eprintln!("Error emitting oidcUrlChange: {}", e);
                    }
                }
            }

            println!("{} loaded: {}", label, payload.url());
        })
        .invoke_handler(tauri::generate_handler![
            functions::file::get_path_meta,
            functions::ai::invoke::predict,
            functions::ai::invoke::find_best_model,
            functions::system::get_system_info,
            functions::download::init::init_downloads,
            functions::download::init::get_downloads,
            functions::settings::profiles::get_profiles,
            functions::settings::profiles::get_default_profiles,
            functions::settings::profiles::get_current_profile,
            functions::settings::profiles::set_current_profile,
            functions::settings::profiles::upsert_profile,
            functions::settings::profiles::delete_profile,
            functions::settings::profiles::add_bit,
            functions::settings::profiles::remove_bit,
            functions::settings::profiles::get_bits_in_current_profile,
            functions::app::app_configured,
            functions::app::create_app_board,
            functions::app::delete_app_board,
            functions::app::get_app,
            functions::app::push_app_meta,
            functions::app::get_app_meta,
            functions::app::get_app_board,
            functions::app::get_app_boards,
            functions::app::set_app_config,
            functions::app::get_apps,
            functions::app::get_app_size,
            functions::app::create_app,
            functions::app::update_app,
            functions::app::delete_app,
            functions::bit::get_bit,
            functions::bit::is_bit_installed,
            functions::bit::get_bit_size,
            functions::bit::get_pack_from_bit,
            functions::bit::search_bits,
            functions::bit::download_bit,
            functions::bit::delete_bit,
            functions::bit::get_installed_bit,
            functions::flow::storage::storage_list,
            functions::flow::storage::storage_add,
            functions::flow::storage::storage_remove,
            functions::flow::storage::storage_rename,
            functions::flow::storage::storage_get,
            functions::flow::storage::storage_to_fullpath,
            functions::flow::catalog::get_catalog,
            functions::flow::board::create_board,
            functions::flow::board::create_board_version,
            functions::flow::board::get_board_versions,
            functions::flow::board::close_board,
            functions::flow::board::get_board,
            functions::flow::board::get_open_boards,
            functions::flow::board::update_board_meta,
            functions::flow::board::undo_board,
            functions::flow::board::redo_board,
            functions::flow::board::execute_command,
            functions::flow::board::execute_commands,
            functions::flow::board::save_board,
            functions::flow::run::execute_board,
            functions::flow::run::execute_event,
            functions::flow::run::list_runs,
            functions::flow::run::query_run,
            functions::flow::run::cancel_execution,
            functions::flow::event::validate_event,
            functions::flow::event::get_event,
            functions::flow::event::get_events,
            functions::flow::event::get_event_versions,
            functions::flow::event::upsert_event,
            functions::flow::event::delete_event,
            functions::flow::template::get_template,
            functions::flow::template::get_templates,
            functions::flow::template::get_template_versions,
            functions::flow::template::upsert_template,
            functions::flow::template::delete_template,
            functions::flow::template::get_template_meta,
            functions::flow::template::push_template_meta,
        ]);

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(handle_instance));
    }

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn handle_instance(app: &AppHandle, args: Vec<String>, _cwd: String) {
    let _ = app
        .get_webview_window("main")
        .expect("no main window")
        .set_focus();

    println!(
        "a new app instance was opened with {args:?} and the deep link event was already triggered"
    );
}

fn handle_deep_link(app: &AppHandle, event: OpenUrlEvent) {
    let _ = app
        .get_webview_window("main")
        .expect("no main window")
        .set_focus();

    println!("deep link URLs: {:?}", event.urls());
}
