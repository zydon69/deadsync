//! Shared explicit editor URL/log-file launcher. One worker/child at a time,
//! a retained worker/child until its natural exit, completed thread
//! joined by the host. No shell strings or render-time filesystem reads.
use std::{
    io,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    thread::JoinHandle,
    time::Duration,
};
#[derive(Clone, Debug)]
pub(super) enum Target {
    Url(String),
    File(PathBuf),
}
struct Service {
    task: Option<(bool, JoinHandle<io::Result<()>>)>,
}
fn service() -> &'static Mutex<Service> {
    static SERVICE: OnceLock<Mutex<Service>> = OnceLock::new();
    SERVICE.get_or_init(|| Mutex::new(Service { task: None }))
}
fn valid(target: &Target) -> bool {
    match target {
        Target::Url(url) => {
            url.len() <= 8192 && url.starts_with("https://") && !url.contains(['\r', '\n', '\0'])
        }
        Target::File(path) => {
            !path.as_os_str().is_empty()
                && path.as_os_str().as_encoded_bytes().len() <= 128 * 1024
                && !path.as_os_str().as_encoded_bytes().contains(&0)
        }
    }
}
fn command(target: &Target) -> io::Result<Command> {
    if !valid(target) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid editor external target",
        ));
    }
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = match target {
        Target::Url(_) => {
            let mut command = Command::new("rundll32.exe");
            command.arg("url.dll,FileProtocolHandler");
            command
        }
        Target::File(_) => Command::new("explorer.exe"),
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    command.arg("--");
    match target {
        Target::Url(url) => {
            command.arg(url);
        }
        Target::File(path) => {
            let path = if path.is_absolute() {
                path.clone()
            } else {
                std::env::current_dir()?.join(path)
            };
            command.arg(path);
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}
trait ManagedChild {
    fn status(&mut self) -> io::Result<Option<bool>>;
    fn reap(&mut self) -> io::Result<()>;
}
impl ManagedChild for Child {
    fn status(&mut self) -> io::Result<Option<bool>> {
        self.try_wait().map(|status| status.map(|s| s.success()))
    }
    fn reap(&mut self) -> io::Result<()> {
        self.wait().map(|_| ())
    }
}
fn supervise(child: &mut impl ManagedChild, mut pause: impl FnMut()) -> io::Result<()> {
    loop {
        match child.status() {
            Ok(Some(true)) => return child.reap(),
            Ok(Some(false)) => {
                child.reap()?;
                return Err(io::Error::other("External launcher failed"));
            }
            Ok(None) => pause(),
            Err(error) => {
                let _ = child.reap();
                return Err(error);
            }
        }
    }
}
fn run(target: Target) -> io::Result<()> {
    let mut child = command(&target)?.spawn()?;
    supervise(&mut child, || std::thread::sleep(Duration::from_millis(10)))
}
pub(super) fn poll() {
    let mut service = service().lock().unwrap_or_else(|p| p.into_inner());
    if service
        .task
        .as_ref()
        .is_some_and(|(_, task)| task.is_finished())
    {
        let (file, task) = service.task.take().expect("finished worker");
        let error = match task.join() {
            Ok(Ok(())) => None,
            Ok(Err(error)) => Some(format!("Editor external open failed: {error}")),
            Err(_) => Some("Editor external open worker panicked".into()),
        };
        if let Some(error) = error {
            if file {
                log::error!("{error}");
            } else {
                log::warn!("{error}");
            }
        }
    }
}
pub(super) fn request(target: Target) -> bool {
    poll();
    if !valid(&target) {
        return false;
    }
    let mut service = service().lock().unwrap_or_else(|p| p.into_inner());
    if service.task.is_some() {
        return false;
    }
    let file = matches!(&target, Target::File(_));
    match std::thread::Builder::new()
        .name("editor-external-open".into())
        .spawn(move || run(target))
    {
        Ok(task) => {
            service.task = Some((file, task));
            true
        }
        Err(error) => {
            if file {
                log::error!("Could not start editor log opener: {error}");
            } else {
                log::warn!("Could not start editor external opener: {error}");
            }
            false
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        pending: usize,
        success: bool,
        reaped: bool,
    }
    impl ManagedChild for Fake {
        fn status(&mut self) -> io::Result<Option<bool>> {
            if self.pending > 0 {
                self.pending -= 1;
                Ok(None)
            } else {
                Ok(Some(self.success))
            }
        }
        fn reap(&mut self) -> io::Result<()> {
            self.reaped = true;
            Ok(())
        }
    }
    #[test]
    fn long_running_and_failed_launchers_are_retained_and_reaped() {
        for (pending, success) in [(1001, true), (0, false), (2, true)] {
            let mut child = Fake {
                pending,
                success,
                reaped: false,
            };
            let result = supervise(&mut child, || {});
            assert!(child.reaped);
            assert_eq!(result.is_ok(), success);
        }
    }
    #[test]
    fn urls_and_paths_are_separate_literal_arguments() {
        assert!(!valid(&Target::Url("file:///tmp/evil".into())));
        assert!(!valid(&Target::Url("https://example.com/\ncommand".into())));
        let url = "https://example.com/$(not-a-shell)?a=1&b=2";
        let cmd = command(&Target::Url(url.into())).unwrap();
        assert_eq!(cmd.get_args().last().unwrap(), url);
        let path = std::env::temp_dir().join("log $(literal).txt");
        let cmd = command(&Target::File(path.clone())).unwrap();
        assert_eq!(cmd.get_args().last().unwrap(), path.as_os_str());
    }
}
