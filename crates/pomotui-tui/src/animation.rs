#![allow(clippy::missing_errors_doc)]

pub const MAX_FRAMES: usize = 120;
pub const MAX_WIDTH: usize = 160;
pub const MAX_HEIGHT: usize = 80;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Animation {
    pub frame_ms: u64,
    pub hold_frames: u64,
    pub frames: Vec<String>,
}

impl Animation {
    pub fn parse(source: &str) -> Result<Self, String> {
        let mut lines = source.lines();
        let frame_ms = value(lines.next(), "frame_ms")?;
        let hold_frames = value(lines.next(), "hold_frames")?;
        if frame_ms == 0 {
            return Err("frame_ms must be nonzero".into());
        }
        let frames: Vec<_> = lines
            .collect::<Vec<_>>()
            .join("\n")
            .split("\n---\n")
            .map(str::to_owned)
            .collect();
        validate_frames(&frames)?;
        Ok(Self {
            frame_ms,
            hold_frames,
            frames,
        })
    }

    #[must_use]
    pub fn frame(&self, elapsed_ms: u64) -> (&str, bool) {
        let index = usize::try_from(elapsed_ms / self.frame_ms).unwrap_or(usize::MAX);
        let finished_at = self
            .frames
            .len()
            .saturating_add(usize::try_from(self.hold_frames).unwrap_or(usize::MAX));
        let finished = index >= finished_at;
        (&self.frames[index.min(self.frames.len() - 1)], finished)
    }
}

fn value(line: Option<&str>, name: &str) -> Result<u64, String> {
    let line = line.ok_or_else(|| format!("missing {name}"))?;
    let (key, value) = line
        .split_once('=')
        .ok_or_else(|| format!("invalid {name}"))?;
    if key.trim() != name {
        return Err(format!("expected {name}"));
    }
    value.trim().parse().map_err(|_| format!("invalid {name}"))
}

fn validate_frames(frames: &[String]) -> Result<(), String> {
    if frames.is_empty() || frames[0].is_empty() {
        return Err("at least one frame is required".into());
    }
    if frames.len() > MAX_FRAMES {
        return Err("too many frames".into());
    }
    let expected_canvas = canvas(&frames[0]);
    for frame in frames {
        if frame
            .chars()
            .any(|character| character.is_control() && character != '\n')
        {
            return Err("frame contains terminal control text".into());
        }
        if frame.lines().count() > MAX_HEIGHT
            || frame.lines().any(|line| line.chars().count() > MAX_WIDTH)
        {
            return Err("frame canvas exceeds limits".into());
        }
        if canvas(frame) != expected_canvas {
            return Err("all frames must use one fixed canvas".into());
        }
    }
    Ok(())
}

fn canvas(frame: &str) -> (usize, usize) {
    (
        frame
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0),
        frame.lines().count(),
    )
}

#[must_use]
pub fn built_in() -> Animation {
    Animation {
        frame_ms: 90,
        hold_frames: 3,
        frames: vec![
            "╔════╗\n║    ║\n╚════╝".into(),
            "╲    ╱\n│ ╲╱ │\n▔▔▔▔▔▔".into(),
            "│    │\n│    │\n▁▂▃▂▁▁".into(),
        ],
    }
}

#[must_use]
pub fn custom_or_builtin(source: &str) -> (Animation, Option<String>) {
    match Animation::parse(source) {
        Ok(animation) => (animation, None),
        Err(error) => (
            built_in(),
            Some(format!("custom animation invalid: {error}; using built-in")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_selects_frames_and_finishes_after_hold() {
        let animation = Animation::parse("frame_ms=10\nhold_frames=2\nA\n---\nB").expect("parse");
        assert_eq!(animation.frame(0), ("A", false));
        assert_eq!(animation.frame(10), ("B", false));
        assert_eq!(animation.frame(40), ("B", true));
    }

    #[test]
    fn invalid_custom_animation_falls_back_visibly() {
        let (animation, warning) = custom_or_builtin("frame_ms=0\nhold_frames=1\nA");
        assert_eq!(animation, built_in());
        assert!(warning.expect("warning").contains("using built-in"));
    }

    #[test]
    fn mismatched_canvas_is_rejected() {
        let error =
            Animation::parse("frame_ms=10\nhold_frames=1\nAA\n---\nB").expect_err("fixed canvas");
        assert!(error.contains("fixed canvas"));
    }
}
