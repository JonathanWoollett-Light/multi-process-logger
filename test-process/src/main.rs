use log::LevelFilter;
use logger_client::Logger;
use std::thread::sleep;
use std::time::Duration;

const SPACING: Duration = Duration::from_millis(100);
fn main() {
    Logger::init("./a-local-socket", LevelFilter::Debug).unwrap();

    let handles = (0..10)
        .map(|_| std::thread::spawn(tester))
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap();
    }
}

fn tester() {
    for _ in 0..5 {
        log::trace!("test trace");
        sleep(SPACING);
        log::debug!("test debug");
        sleep(SPACING);
        log::info!("test info");
        sleep(SPACING);
        log::warn!("test warn");
        sleep(SPACING);
        log::error!("test error");
        sleep(SPACING);
    }
}
