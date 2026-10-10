#[derive(Debug)] struct Error;
type Result<T, E = Error> = std::result::Result<T, E>;
fn run<F: FnOnce() -> R + Send, R: Send>(f: F) -> std::thread::Result<R> { Ok(f()) }
fn work() -> Result<()> {
    run(|| {
        Ok(()) as Result<_>
    })
    .unwrap()?;
    Ok(())
}
fn main() { work().unwrap(); }
