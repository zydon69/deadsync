//! Keyboard-only host for isolated assistant proposals.
use super::*;
use deadsync_editor::assistant::agent::{Draft, Event, Job, PreviewChart};
const INPUT_BYTE_LIMIT: usize = 8192;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProposalSource {
    Native,
    Mcp(u64),
}
struct PendingDraft {
    source: ProposalSource,
    draft: Draft,
}
#[derive(Default)]
pub(super) struct Ui {
    pub(super) open: bool,
    input: String,
    transcript: String,
    conversation: String,
    pending_input: Option<String>,
    job: Option<Job>,
    draft: Option<PendingDraft>,
    bridge: Option<deadsync_editor::assistant::bridge::Bridge>,
    bridge_generation: u64,
    scroll: usize,
    preview: Option<(String, deadsync_editor::notes::Grid)>,
    preview_charts: Vec<PreviewChart>,
    preview_index: usize,
    preview_open: bool,
    preview_row: u64,
}
impl State {
    pub(super) fn open_assistant(&mut self) {
        if !self.has_song() || self.audio_output_busy() || self.file_modal() || self.save_as_modal()
        {
            return;
        }
        self.release_input();
        self.stop_audio();
        self.label_preedit.clear();
        self.assistant.open = true;
    }
    fn assistant_start_bridge(&mut self) {
        self.assistant.bridge = None;
        match deadsync_editor::assistant::bridge::Bridge::start(&self.workspace) {
            Ok(bridge) => {
                self.assistant.transcript.push_str(&format!(
                    "\nMCP: --connect-editor ({})\n",
                    bridge.discovery_path().display()
                ));
                self.assistant_install_bridge(bridge);
            }
            Err(e) => self.assistant.transcript.push_str(&format!("\nMCP: {e}\n")),
        }
    }
    fn assistant_install_bridge(&mut self, bridge: deadsync_editor::assistant::bridge::Bridge) {
        self.assistant.bridge_generation = self.assistant.bridge_generation.wrapping_add(1);
        self.assistant.bridge = Some(bridge);
    }
    fn assistant_discard_pending(&mut self) {
        // A native-only rejection must not reset unrelated external
        // clients. MCP-derived follow-ups retain their provenance.
        if self.assistant.draft.as_ref().is_some_and(|pending| {
            pending.source == ProposalSource::Mcp(self.assistant.bridge_generation)
        }) && let Some(bridge) = &self.assistant.bridge
            && let Err(error) = bridge.discard_proposals()
        {
            self.assistant
                .transcript
                .push_str(&format!("\nMCP: {error}\n"));
        }
        self.assistant.draft = None;
        self.assistant.preview = None;
        self.assistant.preview_charts.clear();
        self.assistant.preview_open = false;
    }
    pub(super) fn assistant_accepts_input(&self) -> bool {
        self.assistant.open && self.assistant.job.is_none() && !self.assistant.preview_open
    }
    pub(super) fn assistant_text(&mut self, text: &str) -> bool {
        // Even a clipboard containing only filtered controls must have bounded
        // UI-thread work. Twice the normalized bound accommodates CRLF input.
        if !self.assistant_accepts_input() || text.is_empty() || text.len() > INPUT_BYTE_LIMIT * 2 {
            return false;
        }
        // Preserve paragraphs and indentation without admitting native control
        // bytes such as NUL. Normalize pasted Windows and legacy Mac line ends.
        let remaining = INPUT_BYTE_LIMIT.saturating_sub(self.assistant.input.len());
        let mut normalized = String::with_capacity(text.len().min(remaining));
        let mut after_carriage_return = false;
        for mut character in text.chars() {
            if character == '\r' {
                character = '\n';
                after_carriage_return = true;
            } else {
                let paired_line_feed = after_carriage_return && character == '\n';
                after_carriage_return = false;
                if paired_line_feed {
                    continue;
                }
            }
            if character.is_control() && !matches!(character, '\n' | '\t') {
                continue;
            }
            if normalized.len() + character.len_utf8() > remaining {
                return false;
            }
            normalized.push(character);
        }
        if normalized.is_empty() {
            return false;
        }
        self.assistant.input.push_str(&normalized);
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        true
    }
    fn assistant_prepare_preview(&mut self) {
        self.assistant.preview_charts.clear();
        self.assistant.preview_index = 0;
        if let Some(pending) = &self.assistant.draft {
            match pending.draft.preview_charts(&self.workspace) {
                Ok(charts) => self.assistant.preview_charts = charts,
                Err(error) => self.assistant.transcript.push_str(&format!("\n{error}\n")),
            }
        }
        self.assistant.preview_open = false;
        self.assistant_load_preview();
    }
    fn assistant_load_preview(&mut self) {
        self.assistant.preview = None;
        self.assistant.preview_row = 0;
        if let Some(chart) = self
            .assistant
            .preview_charts
            .get(self.assistant.preview_index)
            && let Some(pending) = &self.assistant.draft
        {
            match pending.draft.preview_chart(&self.workspace, chart) {
                Ok(grid) => {
                    let version = i18n::tr(
                        "Edition",
                        if chart.before {
                            "AssistantPreviewBefore"
                        } else {
                            "AssistantPreviewAfter"
                        },
                    );
                    self.assistant.preview = Some((
                        format!(
                            "{} - {}: {} ({}/{})",
                            version,
                            chart.index + 1,
                            chart.layout,
                            self.assistant.preview_index + 1,
                            self.assistant.preview_charts.len()
                        ),
                        grid,
                    ));
                }
                Err(error) => self.assistant.transcript.push_str(&format!("\n{error}\n")),
            }
        }
    }
    pub(super) fn assistant_key(&mut self, event: &RawKeyboardEvent) {
        if !event.pressed {
            return;
        }
        if self.assistant.preview_open {
            match event.code {
                KeyCode::Escape | KeyCode::F5 => self.assistant.preview_open = false,
                KeyCode::ArrowLeft | KeyCode::ArrowRight
                    if !self.assistant.preview_charts.is_empty() =>
                {
                    let count = self.assistant.preview_charts.len();
                    self.assistant.preview_index = if event.code == KeyCode::ArrowRight {
                        (self.assistant.preview_index + 1) % count
                    } else {
                        (self.assistant.preview_index + count - 1) % count
                    };
                    self.assistant_load_preview();
                }
                KeyCode::ArrowUp => {
                    self.assistant.preview_row = self.assistant.preview_row.saturating_sub(48)
                }
                KeyCode::ArrowDown => {
                    self.assistant.preview_row = self
                        .assistant
                        .preview_row
                        .saturating_add(48)
                        .min(i32::MAX as u64)
                }
                KeyCode::PageUp => {
                    self.assistant.preview_row = self.assistant.preview_row.saturating_sub(768)
                }
                KeyCode::PageDown => {
                    self.assistant.preview_row = self
                        .assistant
                        .preview_row
                        .saturating_add(768)
                        .min(i32::MAX as u64)
                }
                _ => {}
            }
            return;
        }
        if event.code == KeyCode::KeyV && self.physical_clipboard_modifier_down() {
            if !event.repeat {
                self.queue_assistant_clipboard_paste();
            }
            return;
        }
        match event.code {
            KeyCode::F5 if self.assistant.preview.is_some() => {
                self.assistant.preview_open = true;
            }
            KeyCode::Escape => {
                if let Some(job) = &self.assistant.job {
                    job.cancel();
                } else {
                    self.assistant.open = false;
                    self.release_input();
                }
            }
            KeyCode::Backspace if self.assistant.job.is_none() => {
                if self.assistant.input.pop().is_some() {
                    self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
                }
            }
            KeyCode::PageUp => {
                self.assistant.scroll = self.assistant.scroll.saturating_add(8);
            }
            KeyCode::PageDown => {
                self.assistant.scroll = self.assistant.scroll.saturating_sub(8);
            }
            KeyCode::F4 if self.assistant.job.is_none() => {
                if self.assistant.bridge.take().is_some() {
                    self.assistant.transcript.push_str(&format!(
                        "\n{}\n",
                        i18n::tr("Edition", "AssistantMcpStopped")
                    ));
                } else {
                    self.assistant_start_bridge();
                }
            }
            KeyCode::F3 if self.assistant.job.is_none() => {
                self.assistant_discard_pending();
            }
            KeyCode::F2 if self.assistant.job.is_none() => {
                if let Some(draft) = self.assistant.draft.take() {
                    match draft.draft.apply(&mut self.workspace) {
                        Ok(()) => {
                            self.assistant.preview = None;
                            self.assistant.preview_charts.clear();
                            self.assistant.preview_open = false;
                            self.focus_position = None;
                            self.clear_unfocused_chart_selection();
                            self.refresh();
                            self.assistant.transcript.push_str(&format!(
                                "\n{}\n",
                                i18n::tr("Edition", "AssistantApplied")
                            ));
                        }
                        Err(e) => {
                            self.assistant.transcript.push_str(&format!("\n{e}\n"));
                        }
                    }
                }
            }
            KeyCode::Enter
                if !event.repeat
                    && self.assistant.job.is_none()
                    && !self.assistant.input.trim().is_empty() =>
            {
                let input = self.assistant.input.clone();
                trim_front(&mut self.assistant.conversation, 16_384);
                // Follow-ups refine the pending draft only while its original
                // revision is still current; Job rejects stale proposals.
                let prompt = format!(
                    "Prior conversation (context only):\n{}\n\nCurrent request:\n{}",
                    self.assistant.conversation, input
                );
                match Job::start_with_draft(
                    &self.workspace,
                    prompt,
                    self.assistant.draft.as_ref().map(|pending| &pending.draft),
                ) {
                    Ok(job) => {
                        self.assistant
                            .transcript
                            .push_str(&format!("\n> {input}\n"));
                        self.assistant.pending_input = Some(input);
                        self.assistant.input.clear();
                        self.assistant.scroll = 0;
                        self.assistant.job = Some(job);
                    }
                    Err(e) => {
                        self.assistant.transcript.push_str(&format!("\n{e}\n"));
                    }
                }
            }
            _ => {}
        }
    }
    pub(super) fn poll_assistant(&mut self) {
        if self
            .assistant
            .draft
            .as_ref()
            .is_some_and(|draft| !draft.draft.is_current(&self.workspace))
        {
            self.assistant.draft = None;
            self.assistant.preview = None;
            self.assistant.preview_charts.clear();
            self.assistant.preview_open = false;
            self.assistant
                .transcript
                .push_str(&format!("\n{}\n", i18n::tr("Edition", "AssistantStale")));
        }
        if let Some(bridge) = &self.assistant.bridge {
            if let Err(error) = bridge.synchronize(&self.workspace) {
                self.assistant
                    .transcript
                    .push_str(&format!("\nMCP: {error}\n"));
                self.assistant.bridge = None;
            }
        }
        // Preserve the proposal currently under review. The bridge mailbox
        // provides backpressure until the user applies or discards it.
        if self.assistant.job.is_none() && self.assistant.draft.is_none() {
            if let Some(result) = self.assistant.bridge.as_ref().and_then(|b| b.poll()) {
                match result {
                    Ok(draft) if draft.is_current(&self.workspace) => {
                        match draft.has_changes(&self.workspace) {
                            Ok(false) => {}
                            Ok(true) => {
                                self.assistant.draft = Some(PendingDraft {
                                    source: ProposalSource::Mcp(self.assistant.bridge_generation),
                                    draft,
                                });
                                self.assistant_prepare_preview();
                                self.assistant.transcript.push_str(&format!(
                                    "\nMCP: {}\n",
                                    i18n::tr("Edition", "AssistantReady")
                                ));
                            }
                            Err(error) => self
                                .assistant
                                .transcript
                                .push_str(&format!("\nMCP: {error}\n")),
                        }
                    }
                    Ok(_) => self
                        .assistant
                        .transcript
                        .push_str(&format!("\n{}\n", i18n::tr("Edition", "AssistantStale"))),
                    Err(e) => self.assistant.transcript.push_str(&format!("\nMCP: {e}\n")),
                }
            }
        }
        loop {
            let event = self.assistant.job.as_ref().and_then(Job::poll);
            match event {
                Some(Event::Text(delta)) => self.assistant.transcript.push_str(&delta),
                Some(Event::Tool(name)) => self
                    .assistant
                    .transcript
                    .push_str(&format!("\n[{}]\n", assistant_tool_label(&name))),
                Some(Event::Cancelled) => {
                    self.assistant.job = None;
                    self.assistant.pending_input = None;
                    self.assistant.transcript.push_str(&format!(
                        "\n{}\n",
                        i18n::tr("Edition", "AssistantCancelled")
                    ));
                    break;
                }
                Some(Event::Finished(result)) => {
                    self.assistant.job = None;
                    let input = self.assistant.pending_input.take();
                    match result {
                        Ok(draft) => {
                            if let Some(input) = input {
                                self.assistant
                                    .conversation
                                    .push_str(&format!("\nUser: {input}\n"));
                            }
                            self.assistant
                                .conversation
                                .push_str(&format!("Assistant: {}\n", draft.message));
                            match draft.has_changes(&self.workspace) {
                                Ok(true) => {
                                    let source = self
                                        .assistant
                                        .draft
                                        .as_ref()
                                        .map_or(ProposalSource::Native, |pending| pending.source);
                                    self.assistant.draft = Some(PendingDraft { source, draft });
                                    self.assistant_prepare_preview();
                                    self.assistant.transcript.push_str(&format!(
                                        "\n{}\n",
                                        i18n::tr("Edition", "AssistantReady")
                                    ));
                                }
                                Ok(false) => self.assistant_discard_pending(),
                                Err(error) => {
                                    self.assistant.transcript.push_str(&format!("\n{error}\n"));
                                }
                            }
                        }
                        Err(error) => self.assistant.transcript.push_str(&format!("\n{error}\n")),
                    }
                    // Bound both display memory and subsequent model context.
                    trim_front(&mut self.assistant.conversation, 16_384);
                    trim_front(&mut self.assistant.transcript, 131_072);
                    break;
                }
                None => break,
            }
        }
        trim_front(&mut self.assistant.conversation, 16_384);
        trim_front(&mut self.assistant.transcript, 131_072);
    }
    fn assistant_preview_actors(&self, actors: &mut Vec<Actor>) {
        let Some((label, grid)) = &self.assistant.preview else {
            return;
        };
        let w = screen_width();
        let h = screen_height();
        actors
            .push(act!(quad: xy(w*0.5,h*0.5): zoomto(w,h): diffuse(0.035,0.045,0.07,1.0): z(8100)));
        actors.push(act!(text: font("miso"): settext(format!("{} - {}",i18n::tr("Edition","AssistantPreview"),label)): align(0.0,0.0): xy(20.0,20.0): zoom(0.65): maxwidth(w-40.0): z(8102)));
        actors.push(act!(text: font("miso"): settext(i18n::tr("Edition","AssistantPreviewHelp")): align(0.0,0.0): xy(20.0,h-40.0): zoom(0.5): maxwidth(w-40.0): z(8102)));
        let top = 65.0;
        let bottom = h - 65.0;
        let left = w * 0.2;
        let width = w * 0.6;
        let columns = f32::from(grid.columns());
        let lane = width / columns;
        let start = self.assistant.preview_row;
        let rows = 768.0;
        let scale = (bottom - top) / rows;
        for column in 0..=columns as usize {
            let x = left + column as f32 * lane;
            actors.push(act!(quad: xy(x,(top+bottom)*0.5): zoomto(1.0,bottom-top): diffuse(0.3,0.35,0.4,1.0): z(8101)));
        }
        for (position, note) in grid.iter() {
            let end = note.end.unwrap_or(position.row);
            if end < start || position.row > start + rows as u64 {
                continue;
            }
            let x = left + (f32::from(position.lane) + 0.5) * lane;
            let head = top + (position.row.saturating_sub(start) as f32) * scale;
            let tail = (top + (end.saturating_sub(start) as f32) * scale).min(bottom);
            if note.end.is_some() {
                actors.push(act!(quad: xy(x,(head+tail)*0.5): zoomto(lane*0.35,(tail-head).max(2.0)): diffuse(0.3,0.6,0.9,0.7): z(8101)));
            }
            if position.row >= start {
                use deadsync_editor::notes::Kind;
                let color = match note.kind {
                    Kind::Mine => [1.0, 0.2, 0.2],
                    Kind::Fake => [0.5, 0.5, 0.5],
                    Kind::Lift => [0.3, 1.0, 0.4],
                    Kind::Roll => [0.85, 0.3, 1.0],
                    _ => {
                        if position.player == 0 {
                            [0.4, 0.85, 1.0]
                        } else {
                            [1.0, 0.7, 0.2]
                        }
                    }
                };
                actors.push(act!(quad: xy(x,head): zoomto(lane*0.7,5.0): diffuse(color[0],color[1],color[2],1.0): z(8102)));
            }
        }
    }
    pub(super) fn assistant_actors(&self, actors: &mut Vec<Actor>) {
        if self.assistant.preview_open {
            self.assistant_preview_actors(actors);
            return;
        }
        let width = screen_width();
        let height = screen_height();
        actors.push(act!(quad: xy(width*0.5,height*0.5): zoomto(width,height): diffuse(0.035,0.045,0.07,1.0): z(8100)));
        let mut add = |content: String, y: f32, h: f32, zoom: f32| {
            actors.push(act!(text: font("miso"): settext(content): align(0.0,0.0): xy(22.0,y): zoom(zoom): wrapwidthpixels((width-44.0)/zoom): maxwidth(width-44.0): maxheight(h): z(8101)));
        };
        add(
            i18n::tr("Edition", "AssistantTitle").to_string(),
            16.0,
            32.0,
            0.8,
        );
        add(
            i18n::tr("Edition", "AssistantIntro").to_string(),
            52.0,
            84.0,
            0.5,
        );
        let columns = ((width - 44.0) / (0.52 * 10.0)).max(16.0) as usize;
        let mut wrapped = Vec::new();
        for line in self.assistant.transcript.lines() {
            let mut current = String::new();
            let mut length = 0;
            for character in line.chars() {
                if length == columns {
                    wrapped.push(std::mem::take(&mut current));
                    length = 0;
                }
                current.push(character);
                length += 1;
            }
            wrapped.push(current);
        }
        let lines: Vec<_> = wrapped.iter().map(String::as_str).collect();
        let end = lines
            .len()
            .saturating_sub(self.assistant.scroll.min(lines.len()));
        let count = ((height - 250.0) / 16.0).max(1.0) as usize;
        add(
            lines[end.saturating_sub(count)..end].join("\n"),
            140.0,
            (height - 250.0).max(1.0),
            0.52,
        );
        let input = if self.assistant.job.is_some() {
            i18n::tr("Edition", "AssistantWorking").to_string()
        } else {
            let tail: String = self
                .assistant
                .input
                .chars()
                .rev()
                .take(240)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("> {}{}", tail, self.label_preedit)
        };
        add(input, height - 94.0, 46.0, 0.55);
        add(
            i18n::tr("Edition", "AssistantHelp").to_string(),
            height - 40.0,
            36.0,
            0.45,
        );
    }
}
fn trim_front(text: &mut String, limit: usize) {
    if text.len() <= limit {
        return;
    }
    let mut boundary = text.len() - limit;
    while !text.is_char_boundary(boundary) {
        boundary += 1;
    }
    text.drain(..boundary);
}

fn assistant_tool_label(name: &str) -> String {
    let key = match name {
        "inspect_song" | "inspect_chart" | "inspect_timing" => "AssistantInspect",
        "analyze_chart" => "AssistantAnalyzeChart",
        "analyze_audio" => "AssistantAnalyzeAudio",
        "generate_chart" | "generate_audio_chart" => "AssistantGenerate",
        "preview_note_edits" | "preview_tempo_edits" | "preview_changes" => "AssistantPrepare",
        "apply_preview" => "AssistantApplyDraft",
        "undo" | "redo" => "AssistantHistory",
        _ => "AssistantWorking",
    };
    i18n::tr("Edition", key).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(state: &mut State, code: KeyCode, pressed: bool) {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: std::time::Instant::now(),
            host_nanos: 0,
        });
    }
    #[test]
    fn closing_assistant_releases_shortcut_modifiers_before_undo() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:\n1000\n0000\n0000\n0000\n;\n").unwrap();
        let mut state = State::from_workspace(Workspace::open(&path, 0).unwrap()).unwrap();
        state.workspace.execute(Command::DuplicateChart).unwrap();
        assert_eq!(state.workspace.file().session.document().charts().len(), 2);
        // Enter the modal with held shortcut modifiers without starting an
        // external bridge: this test owns keyboard routing, not user discovery.
        key(&mut state, KeyCode::ControlLeft, true);
        key(&mut state, KeyCode::ShiftLeft, true);
        state.release_input();
        state.assistant.open = true;
        key(&mut state, KeyCode::KeyI, true);
        for code in [KeyCode::KeyI, KeyCode::ShiftLeft, KeyCode::ControlLeft] {
            key(&mut state, code, false);
        }
        key(&mut state, KeyCode::Escape, true);
        key(&mut state, KeyCode::Escape, false);
        assert!(!state.assistant.open);
        key(&mut state, KeyCode::ControlLeft, true);
        key(&mut state, KeyCode::KeyZ, true);
        key(&mut state, KeyCode::KeyZ, false);
        key(&mut state, KeyCode::ControlLeft, false);
        assert_eq!(state.workspace.file().session.document().charts().len(), 1);
    }

    #[derive(Default)]
    struct Clipboard {
        text: String,
        reads: usize,
    }
    impl deadsync_editor::clipboard::TextClipboard for Clipboard {
        fn read_text(&mut self) -> Result<String, deadsync_editor::clipboard::Error> {
            self.reads += 1;
            Ok(self.text.clone())
        }
        fn write_text(&mut self, _: &str) -> Result<(), deadsync_editor::clipboard::Error> {
            panic!("Assistant paste must not overwrite the system clipboard")
        }
    }
    fn input_fixture() -> (tempfile::TempDir, State) {
        crate::i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("clipboard.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.open_assistant();
        assert!(state.assistant.open);
        (directory, state)
    }
    fn paste_chord(state: &mut State, modifier: KeyCode) {
        key(state, modifier, true);
        key(state, KeyCode::KeyV, true);
        key(state, KeyCode::KeyV, false);
        key(state, modifier, false);
    }
    fn song_bytes(state: &State) -> Vec<u8> {
        state.workspace.file().session.serialize().unwrap()
    }
    fn mcp_worker_call(
        path: &std::path::Path,
        name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        use std::io::{BufRead, BufReader, Write};
        let connection: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut stream =
            std::net::TcpStream::connect(connection["address"].as_str().unwrap()).unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        serde_json::to_writer(
            &mut stream,
            &serde_json::json!({
                "token":connection["token"], "client":"assistant-ui-regression",
                "request":{"jsonrpc":"2.0", "id":1, "method":"tools/call",
                    "params":{"name":name, "arguments":arguments}}
            }),
        )
        .unwrap();
        stream.write_all(b"\n").unwrap();
        let mut response = String::new();
        BufReader::new(stream).read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }
    fn mcp_call(
        state: &mut State,
        path: &std::path::Path,
        name: &str,
        arguments: serde_json::Value,
    ) -> serde_json::Value {
        let path = path.to_owned();
        let name = name.to_owned();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            sender
                .send(mcp_worker_call(&path, &name, arguments))
                .unwrap();
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            state.poll_assistant();
            match receiver.recv_timeout(std::time::Duration::from_millis(1)) {
                Ok(response) => {
                    worker.join().unwrap();
                    return response;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
                    if std::time::Instant::now() < deadline => {}
                error => panic!("MCP fixture did not finish: {error:?}"),
            }
        }
    }
    fn prepare_mcp_note(state: &mut State, path: &std::path::Path, row: u64) -> serde_json::Value {
        let response = mcp_call(
            state,
            path,
            "preview_note_edits",
            serde_json::json!({
                "chart":0, "edits":[{"row":row,"lane":1,"kind":"tap"}]
            }),
        );
        assert_eq!(response["result"]["isError"], false);
        response["result"]["structuredContent"]["preview_id"].clone()
    }
    fn apply_mcp_note(state: &mut State, path: &std::path::Path, preview: serde_json::Value) {
        assert_eq!(
            mcp_call(
                state,
                path,
                "apply_preview",
                serde_json::json!({"preview_id":preview})
            )["result"]["isError"],
            false
        );
    }
    #[test]
    fn rejecting_native_proposal_preserves_external_client_preview() {
        let (directory, mut state) = input_fixture();
        let connection = directory.path().join("connection.json");
        state.assistant_install_bridge(
            deadsync_editor::assistant::bridge::Bridge::start_for_test(
                &state.workspace,
                connection.clone(),
            )
            .unwrap(),
        );
        let preview = prepare_mcp_note(&mut state, &connection, 48);
        let mut detached = Workspace::open(&directory.path().join("clipboard.ssc"), 0).unwrap();
        detached.execute(Command::DuplicateChart).unwrap();
        state.assistant.draft = Some(PendingDraft {
            source: ProposalSource::Native,
            draft: Draft::from_bytes_for_test(
                &state.workspace,
                detached.file().session.serialize().unwrap(),
            ),
        });
        key(&mut state, KeyCode::F3, true);
        key(&mut state, KeyCode::F3, false);
        assert!(state.assistant.draft.is_none());
        // Pressing discard without any displayed proposal also leaves the
        // external client's independent preview untouched.
        key(&mut state, KeyCode::F3, true);
        key(&mut state, KeyCode::F3, false);
        apply_mcp_note(&mut state, &connection, preview);
        state.poll_assistant();
        assert_eq!(
            state.assistant.draft.as_ref().unwrap().source,
            ProposalSource::Mcp(state.assistant.bridge_generation)
        );
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        assert_eq!(state.workspace.file().session.document().charts().len(), 1);
        let rows: Vec<_> = state
            .workspace
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 48]);
    }
    #[test]
    fn rejecting_mcp_proposal_or_native_refinement_resets_its_external_client() {
        for refine in [false, true] {
            let (directory, mut state) = input_fixture();
            let connection = directory.path().join("connection.json");
            state.assistant_install_bridge(
                deadsync_editor::assistant::bridge::Bridge::start_for_test(
                    &state.workspace,
                    connection.clone(),
                )
                .unwrap(),
            );
            let preview = prepare_mcp_note(&mut state, &connection, 48);
            apply_mcp_note(&mut state, &connection, preview);
            state.poll_assistant();
            assert_eq!(
                state.assistant.draft.as_ref().unwrap().source,
                ProposalSource::Mcp(state.assistant.bridge_generation)
            );
            if refine {
                let bytes = state
                    .assistant
                    .draft
                    .as_ref()
                    .unwrap()
                    .draft
                    .bytes()
                    .to_vec();
                let answer = Draft::from_bytes_for_test(&state.workspace, bytes);
                let (job, worker) = Job::pending_for_test();
                state.assistant.job = Some(job);
                worker.send(Event::Finished(Ok(answer))).unwrap();
                state.poll_assistant();
                assert_eq!(
                    state.assistant.draft.as_ref().unwrap().source,
                    ProposalSource::Mcp(state.assistant.bridge_generation)
                );
            }
            key(&mut state, KeyCode::F3, true);
            key(&mut state, KeyCode::F3, false);
            assert!(state.assistant.draft.is_none());
            let inspected = mcp_call(
                &mut state,
                &connection,
                "inspect_chart",
                serde_json::json!({"chart":0}),
            );
            assert_eq!(inspected["result"]["structuredContent"]["total_notes"], 1);
            let preview = prepare_mcp_note(&mut state, &connection, 96);
            apply_mcp_note(&mut state, &connection, preview);
            state.poll_assistant();
            key(&mut state, KeyCode::F2, true);
            key(&mut state, KeyCode::F2, false);
            let rows: Vec<_> = state
                .workspace
                .grid()
                .iter()
                .map(|(position, _)| position.row)
                .collect();
            assert_eq!(rows, vec![0, 96]);
        }
    }

    #[test]
    fn rejected_proposal_from_replaced_bridge_does_not_reset_new_client() {
        let (directory, mut state) = input_fixture();
        let old = directory.path().join("old-connection.json");
        state.assistant_install_bridge(
            deadsync_editor::assistant::bridge::Bridge::start_for_test(
                &state.workspace,
                old.clone(),
            )
            .unwrap(),
        );
        let preview = prepare_mcp_note(&mut state, &old, 48);
        apply_mcp_note(&mut state, &old, preview);
        state.poll_assistant();
        assert!(state.assistant.draft.is_some());
        let connection = directory.path().join("new-connection.json");
        state.assistant_install_bridge(
            deadsync_editor::assistant::bridge::Bridge::start_for_test(
                &state.workspace,
                connection.clone(),
            )
            .unwrap(),
        );
        let preview = prepare_mcp_note(&mut state, &connection, 96);
        key(&mut state, KeyCode::F3, true);
        key(&mut state, KeyCode::F3, false);
        apply_mcp_note(&mut state, &connection, preview);
        state.poll_assistant();
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        let rows: Vec<_> = state
            .workspace
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 96]);
    }
    #[test]
    fn native_followup_reverting_mcp_draft_to_host_invalidates_rejected_edits() {
        let (directory, mut state) = input_fixture();
        let original = song_bytes(&state);
        let connection = directory.path().join("connection.json");
        state.assistant_install_bridge(
            deadsync_editor::assistant::bridge::Bridge::start_for_test(
                &state.workspace,
                connection.clone(),
            )
            .unwrap(),
        );
        let preview = prepare_mcp_note(&mut state, &connection, 48);
        apply_mcp_note(&mut state, &connection, preview);
        state.poll_assistant();
        assert!(state.assistant.draft.is_some());
        let answer = Draft::from_bytes_for_test(&state.workspace, original.clone());
        let (job, worker) = Job::pending_for_test();
        state.assistant.job = Some(job);
        worker.send(Event::Finished(Ok(answer))).unwrap();
        state.poll_assistant();
        assert!(state.assistant.draft.is_none());
        assert_eq!(song_bytes(&state), original);
        let inspected = mcp_call(
            &mut state,
            &connection,
            "inspect_chart",
            serde_json::json!({"chart":0}),
        );
        assert_eq!(inspected["result"]["structuredContent"]["total_notes"], 1);
        let preview = prepare_mcp_note(&mut state, &connection, 96);
        apply_mcp_note(&mut state, &connection, preview);
        state.poll_assistant();
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        let rows: Vec<_> = state
            .workspace
            .grid()
            .iter()
            .map(|(position, _)| position.row)
            .collect();
        assert_eq!(rows, vec![0, 96]);
    }

    #[test]
    fn conversational_completion_has_no_proposal_or_apply_status() {
        let (_directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        let mut draft = Draft::from_bytes_for_test(&state.workspace, before.clone());
        draft.message = "Timing explanation without edits.".into();
        let (job, worker) = Job::pending_for_test();
        state.assistant.job = Some(job);
        worker.send(Event::Finished(Ok(draft))).unwrap();
        state.poll_assistant();
        assert!(state.assistant.job.is_none());
        assert!(state.assistant.draft.is_none());
        assert!(state.assistant.preview.is_none());
        assert!(
            state
                .assistant
                .conversation
                .contains("Timing explanation without edits.")
        );
        assert!(
            !state
                .assistant
                .transcript
                .contains(i18n::tr("Edition", "AssistantReady").as_ref())
        );
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        assert!(
            !state
                .assistant
                .transcript
                .contains(i18n::tr("Edition", "AssistantApplied").as_ref())
        );
        assert_eq!(song_bytes(&state), before);
    }

    #[test]
    fn conversational_followup_retains_previous_unapplied_edits() {
        let (directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        let mut detached = Workspace::open(&directory.path().join("clipboard.ssc"), 0).unwrap();
        detached.execute(Command::DuplicateChart).unwrap();
        let edited = detached.file().session.serialize().unwrap();
        state.assistant.draft = Some(PendingDraft {
            source: ProposalSource::Native,
            draft: Draft::from_bytes_for_test(&state.workspace, edited.clone()),
        });
        let mut answer = Draft::from_bytes_for_test(&state.workspace, edited.clone());
        answer.message = "Explanation of the previous edit.".into();
        let (job, worker) = Job::pending_for_test();
        state.assistant.job = Some(job);
        worker.send(Event::Finished(Ok(answer))).unwrap();
        state.poll_assistant();
        assert!(state.assistant.draft.is_some());
        assert!(
            state
                .assistant
                .transcript
                .contains(i18n::tr("Edition", "AssistantReady").as_ref())
        );
        assert_eq!(song_bytes(&state), before);
        assert_eq!(
            state.assistant.draft.as_ref().unwrap().draft.bytes(),
            edited
        );
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        let expected = deadsync_editor::document::Document::parse(
            edited.as_slice(),
            deadsync_editor::document::Format::Ssc,
        )
        .unwrap();
        let applied = state.workspace.file().session.document();
        assert_eq!(applied.charts().len(), 2);
        for (wanted, actual) in expected.charts().iter().zip(applied.charts()) {
            assert_eq!(actual.steps_type, wanted.steps_type);
            assert_eq!(actual.description, wanted.description);
            assert_eq!(actual.difficulty, wanted.difficulty);
            assert_eq!(actual.meter, wanted.meter);
            assert_eq!(
                applied.value(actual.notes.clone()),
                expected.value(wanted.notes.clone())
            );
        }
        // Publication materializes the existing song timing choice in GA song
        // metadata. Every supplied tag, including notes and BPMs, must survive
        // exactly; only that owned metadata may be added by the host.
        let tag_values = |document: &deadsync_editor::document::Document| {
            document
                .tags()
                .iter()
                .filter(|tag| !matches!(tag.name.as_str(), "GASONGDATA" | "GASONGDATAVERSION"))
                .map(|tag| {
                    (
                        tag.chart,
                        tag.name.clone(),
                        document.value(tag.value.clone()).to_vec(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(tag_values(applied), tag_values(&expected));
        assert_eq!(state.workspace.active_chart(), Some(1));
        assert!(
            state
                .assistant
                .transcript
                .contains(i18n::tr("Edition", "AssistantApplied").as_ref())
        );
    }

    #[test]
    fn assistant_native_paste_preserves_unicode_paragraphs_and_indentation_without_editing_song() {
        for modifier in [
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
        ] {
            let (_directory, mut state) = input_fixture();
            let before = song_bytes(&state);
            assert!(state.text_input("Demande : "));
            let mut clipboard = Clipboard {
                text: "Été 日本語 🎵\r\n\tDeuxième ligne\rTroisième\0\u{1b}".into(),
                ..Default::default()
            };
            paste_chord(&mut state, modifier);
            assert!(state.process_text_clipboard(&mut clipboard));
            assert_eq!(clipboard.reads, 1);
            assert_eq!(
                state.assistant.input,
                "Demande : Été 日本語 🎵\n\tDeuxième ligne\nTroisième"
            );
            assert!(state.assistant.job.is_none());
            assert!(state.assistant.bridge.is_none());
            assert!(!state.assistant.preview_open);
            assert_eq!(song_bytes(&state), before);
            assert!(!state.workspace.file().session.is_dirty());
            assert!(!state.process_text_clipboard(&mut clipboard));
            assert_eq!(clipboard.reads, 1);
        }
    }
    #[test]
    fn assistant_paste_limit_counts_utf8_bytes_and_rejects_overflow_atomically() {
        let (_directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        state.assistant.input = "é".repeat(4095);
        let mut clipboard = Clipboard {
            text: "é".into(),
            ..Default::default()
        };
        paste_chord(&mut state, KeyCode::SuperLeft);
        assert!(state.process_text_clipboard(&mut clipboard));
        assert_eq!(state.assistant.input.len(), 8192);
        let full = state.assistant.input.clone();
        for text in [
            "日本語".to_owned(),
            "a".repeat(1_000_000),
            "\0\u{1b}".into(),
        ] {
            clipboard.text = text;
            paste_chord(&mut state, KeyCode::SuperLeft);
            assert!(state.process_text_clipboard(&mut clipboard));
            assert_eq!(state.assistant.input, full);
        }
        state.assistant.input.clear();
        clipboard.text = "\0".repeat(1_000_000);
        paste_chord(&mut state, KeyCode::SuperLeft);
        assert!(state.process_text_clipboard(&mut clipboard));
        assert!(state.assistant.input.is_empty());
        assert_eq!(song_bytes(&state), before);
    }
    #[test]
    fn assistant_busy_job_rejects_queued_and_new_pastes_before_reading_clipboard() {
        let (_directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        state.assistant.input = "Pending request".into();
        let mut clipboard = Clipboard {
            text: "must not be read".into(),
            ..Default::default()
        };
        paste_chord(&mut state, KeyCode::SuperLeft);
        let (job, _worker) = Job::pending_for_test();
        state.assistant.job = Some(job);
        assert!(!state.process_text_clipboard(&mut clipboard));
        paste_chord(&mut state, KeyCode::ControlLeft);
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert!(!state.text_input("typed while busy"));
        assert_eq!(clipboard.reads, 0);
        assert_eq!(state.assistant.input, "Pending request");
        assert_eq!(song_bytes(&state), before);
        state.assistant.job = None;
        assert!(!state.process_text_clipboard(&mut clipboard));
        paste_chord(&mut state, KeyCode::SuperRight);
        assert!(state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 1);
    }
    #[test]
    fn assistant_preview_modal_rejects_text_and_queued_paste_without_applying_a_chart_change() {
        let (_directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        state.assistant.input = "Keep prompt".into();
        state.assistant.preview = Some((
            "dance-single".into(),
            deadsync_editor::notes::Grid::new(4, 1).unwrap(),
        ));
        let mut clipboard = Clipboard {
            text: "blocked".into(),
            ..Default::default()
        };
        paste_chord(&mut state, KeyCode::SuperLeft);
        key(&mut state, KeyCode::F5, true);
        key(&mut state, KeyCode::F5, false);
        assert!(state.assistant.preview_open);
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert!(!state.text_input("blocked typing"));
        paste_chord(&mut state, KeyCode::ControlLeft);
        assert!(!state.process_text_clipboard(&mut clipboard));
        key(&mut state, KeyCode::F2, true);
        key(&mut state, KeyCode::F2, false);
        assert_eq!(clipboard.reads, 0);
        assert_eq!(state.assistant.input, "Keep prompt");
        assert_eq!(song_bytes(&state), before);
        assert!(!state.workspace.file().session.is_dirty());
        key(&mut state, KeyCode::Escape, true);
        key(&mut state, KeyCode::Escape, false);
        assert!(state.assistant.open);
        assert!(!state.assistant.preview_open);
        paste_chord(&mut state, KeyCode::SuperLeft);
        assert!(state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 1);
        assert_eq!(song_bytes(&state), before);
    }
    #[test]
    fn assistant_paste_is_not_replayed_after_intervening_text_or_close_and_reopen() {
        let (_directory, mut state) = input_fixture();
        let before = song_bytes(&state);
        let mut clipboard = Clipboard {
            text: "stale".into(),
            ..Default::default()
        };
        paste_chord(&mut state, KeyCode::SuperLeft);
        assert!(state.text_input("newer text"));
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        paste_chord(&mut state, KeyCode::SuperLeft);
        key(&mut state, KeyCode::Backspace, true);
        key(&mut state, KeyCode::Backspace, false);
        assert_eq!(state.assistant.input, "newer tex");
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        paste_chord(&mut state, KeyCode::SuperLeft);
        key(&mut state, KeyCode::Escape, true);
        key(&mut state, KeyCode::Escape, false);
        assert!(!state.assistant.open);
        state.open_assistant();
        assert!(!state.process_text_clipboard(&mut clipboard));
        assert_eq!(clipboard.reads, 0);
        assert_eq!(state.assistant.input, "newer tex");
        assert_eq!(song_bytes(&state), before);
    }
}
