fn main() {
    let mut args = std::env::args().skip(1);
    let summary = match args.next() {
        Some(value) => value,
        None => {
            eprintln!("Usage: relay \"summary\" [timeout_seconds]");
            std::process::exit(1);
        }
    };

    let timeout_seconds = args
        .next()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(600);

    if let Err(err) = relay_mcp::run_feedback_cli(summary, timeout_seconds) {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
