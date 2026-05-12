fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new().app_manifest(
            tauri_build::AppManifest::new().commands(&[
                "create_pty",
                "write_pty",
                "resize_pty",
                "close_pty",
                "list_profiles",
                "list_workspaces",
                "get_workspace_status",
                "get_registered_repos",
                "get_default_app",
                "create_workspace",
                "log_error",
                "read_text_file",
                "get_user_guide_path",
                "get_theme",
                "crit_overlay_dismissed",
                "__test_report",
            ]),
        ),
    )
    .expect("failed to run tauri-build");
}
