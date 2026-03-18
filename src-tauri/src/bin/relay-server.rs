fn main() {
    if let Err(err) = relay_mcp::run_feedback_server() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
