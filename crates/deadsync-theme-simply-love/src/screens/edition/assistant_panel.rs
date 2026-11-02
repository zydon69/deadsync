//! Keyboard-only host for isolated assistant proposals.
use super::*;
use deadsync_editor::assistant::agent::{Draft, Event, Job};
#[derive(Default)]
pub(super) struct Ui {
    pub(super) open: bool,
    input: String,
    transcript: String,
    conversation: String,
    job: Option<Job>,
    draft: Option<Draft>,
    bridge: Option<deadsync_editor::assistant::bridge::Bridge>,
    scroll: usize,
    preview: Option<(String, deadsync_editor::notes::Grid)>,
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
                self.assistant.bridge = Some(bridge);
            }
            Err(e) => self.assistant.transcript.push_str(&format!("\nMCP: {e}\n")),
        }
    }
    pub(super) fn assistant_text(&mut self, text: &str) -> bool {
        if self.assistant.job.is_some() || text.chars().all(char::is_control) {
            return false;
        }
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if self.assistant.input.len() + text.len() > 8192 {
            return false;
        }
        self.assistant.input.push_str(&text);
        true
    }
    fn assistant_prepare_preview(&mut self) {
        self.assistant.preview = self
            .assistant
            .draft
            .as_ref()
            .and_then(|d| d.preview_grid(&self.workspace).ok())
            .flatten();
        self.assistant.preview_open = false;
        self.assistant.preview_row = 0;
    }
    pub(super) fn assistant_key(&mut self, event: &RawKeyboardEvent) {
        if !event.pressed {
            return;
        }
        if self.assistant.preview_open {
            match event.code {
                KeyCode::Escape | KeyCode::F5 => self.assistant.preview_open = false,
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
                self.assistant.input.pop();
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
                self.assistant.draft = None;
                self.assistant.preview = None;
            }
            KeyCode::F2 if self.assistant.job.is_none() => {
                if let Some(draft) = self.assistant.draft.take() {
                    match draft.apply(&mut self.workspace) {
                        Ok(()) => {
                            self.assistant.preview = None;
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
                // Follow-ups refine the pending draft only while its original
                // revision is still current; Job rejects stale proposals.
                let prompt = format!(
                    "Prior conversation (context only):\n{}\n\nCurrent request:\n{}",
                    self.assistant.conversation, input
                );
                match Job::start_with_draft(&self.workspace, prompt, self.assistant.draft.as_ref())
                {
                    Ok(job) => {
                        self.assistant
                            .transcript
                            .push_str(&format!("\n> {input}\n"));
                        self.assistant
                            .conversation
                            .push_str(&format!("\nUser: {input}\n"));
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
            .is_some_and(|draft| !draft.is_current(&self.workspace))
        {
            self.assistant.draft = None;
            self.assistant.preview = None;
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
                        self.assistant.draft = Some(draft);
                        self.assistant_prepare_preview();
                        self.assistant.transcript.push_str(&format!(
                            "\nMCP: {}\n",
                            i18n::tr("Edition", "AssistantReady")
                        ));
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
                    self.assistant.transcript.push_str(&format!(
                        "\n{}\n",
                        i18n::tr("Edition", "AssistantCancelled")
                    ));
                    break;
                }
                Some(Event::Finished(result)) => {
                    self.assistant.job = None;
                    match result {
                        Ok(draft) => {
                            self.assistant
                                .conversation
                                .push_str(&format!("Assistant: {}\n", draft.message));
                            self.assistant.draft = Some(draft);
                            self.assistant_prepare_preview();
                            self.assistant.transcript.push_str(&format!(
                                "\n{}\n",
                                i18n::tr("Edition", "AssistantReady")
                            ));
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
}
