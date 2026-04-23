use app_server_host::{AbortDecision, JoinThenAbort, evaluate_shutdown};

fn main() {
    let strategy = JoinThenAbort::default();
    match evaluate_shutdown(false, &strategy) {
        AbortDecision::CleanExit => std::process::exit(0),
        AbortDecision::Abort => {
            eprintln!("warning: GUI shutdown exceeded 5s timeout, aborting process");
            std::process::abort();
        }
    }
}
