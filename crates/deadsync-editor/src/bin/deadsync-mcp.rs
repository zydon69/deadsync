//! Headless local editor MCP server; one explicit song per process.
fn main() {
    if let Err(error) = run() {
        eprintln!("DeadSync MCP: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut song = None;
    let mut allow_save = false;
    let mut connect = None;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--song") => {
                song = Some(args.next().ok_or("--song requires a file")?);
            }
            Some("--connect") => {
                connect = Some(args.next().ok_or("--connect requires a discovery file")?);
            }
            Some("--connect-editor") => {
                connect = Some(
                    deadsync_editor::assistant::bridge::default_connection_path()?.into_os_string(),
                );
            }
            Some("--allow-save") => allow_save = true,
            _ => return Err("Usage: deadsync-mcp --song FILE [--allow-save]".into()),
        }
    }
    if let Some(path) = connect {
        if song.is_some() || allow_save {
            return Err("--connect cannot be combined with --song or --allow-save".into());
        }
        return deadsync_editor::assistant::bridge::connect(
            std::path::Path::new(&path),
            std::io::stdin().lock(),
            std::io::stdout().lock(),
        )
        .map_err(Into::into);
    }
    let song = std::path::PathBuf::from(song.ok_or("--song is required")?).canonicalize()?;
    let file = deadsync_editor::storage::FileSession::open(&song)?;
    let mut workspace = if file.session.document().charts().is_empty() {
        deadsync_editor::workspace::Workspace::from_file_unfocused(file)
    } else {
        deadsync_editor::workspace::Workspace::from_file(file, 0)?
    };
    deadsync_editor::assistant::protocol::serve(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        &mut workspace,
        allow_save,
    )?;
    Ok(())
}
