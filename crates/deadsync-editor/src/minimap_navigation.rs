//! Explicit capture gesture shared by keyboard/pad and future pointer navigation.
//! Caller owns timing conversion; no rounding, clock reads or document mutation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub chart_time: f64,
    pub music_time: f64,
    pub row: f64,
}
impl Position {
    fn valid(self) -> bool {
        self.chart_time.is_finite() && self.music_time.is_finite() && self.row.is_finite()
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Input {
    Begin {
        position: Position,
        playing: bool,
        stop_playback_on_release: bool,
    },
    Move(Position),
    End,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Pause(Position),
    Focus(Position),
    Resume(Position),
}
#[derive(Clone, Copy)]
struct Capture {
    position: Position,
    resume: bool,
}
#[derive(Default)]
pub struct Gesture {
    capture: Option<Capture>,
}
impl Gesture {
    pub fn active(&self) -> bool {
        self.capture.is_some()
    }
    /// At most one effect per input; holding/repeating Begin cannot pause twice.
    /// Invalid positions leave the most recent exact destination untouched.
    pub fn input(&mut self, input: Input) -> Option<Effect> {
        match input {
            Input::Begin {
                position,
                playing,
                stop_playback_on_release,
            } => {
                if self.capture.is_some() || !position.valid() {
                    return None;
                }
                self.capture = Some(Capture {
                    position,
                    resume: playing && !stop_playback_on_release,
                });
                playing.then_some(Effect::Pause(position))
            }
            Input::Move(position) => {
                let capture = self.capture.as_mut()?;
                if !position.valid() || capture.position == position {
                    return None;
                }
                capture.position = position;
                Some(Effect::Focus(position))
            }
            Input::End => {
                let capture = self.capture.take()?;
                capture.resume.then_some(Effect::Resume(capture.position))
            }
        }
    }
    /// Owner/chart teardown drops capture without issuing a delayed resume into
    /// another document. This is lifecycle cleanup, not a user cancel command.
    pub fn clear(&mut self) {
        self.capture = None;
    }
}
