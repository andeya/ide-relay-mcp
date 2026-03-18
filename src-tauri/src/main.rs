use std::path::Path;

use tauri::State;

use relay_mcp::{
    launch_state_from_args, read_control_status, read_launch_args, write_text_file, ControlStatus,
    LaunchState,
};

#[tauri::command]
fn get_launch_state(state: State<'_, LaunchState>) -> LaunchState {
    state.inner().clone()
}

#[tauri::command]
fn read_feedback_status(state: State<'_, LaunchState>) -> Option<ControlStatus> {
    read_control_status(Path::new(&state.control_file))
}

#[tauri::command]
fn submit_feedback(state: State<'_, LaunchState>, feedback: String) -> Result<(), String> {
    write_text_file(Path::new(&state.result_file), &feedback).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_file(&state.control_file);
    Ok(())
}

fn main() {
    let (summary, result_file, control_file) = match read_launch_args() {
        Ok(values) => values,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    };

    let launch_state = launch_state_from_args(summary, result_file, control_file);

    tauri::Builder::default()
        .manage(launch_state)
        .invoke_handler(tauri::generate_handler![
            get_launch_state,
            read_feedback_status,
            submit_feedback
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Relay GUI");
}
