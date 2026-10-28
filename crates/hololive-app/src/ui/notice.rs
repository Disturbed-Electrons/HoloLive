use std::time::{Duration, Instant};

use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum NoticeKind {
    #[default]
    Status,
    Camera,
}

impl NoticeKind {
    pub(crate) fn color(self) -> Color32 {
        match self {
            Self::Status => Color32::from_rgb(245, 190, 90),
            Self::Camera => Color32::from_rgb(110, 205, 195),
        }
    }
}

/// Header toast. One message at a time; newer notices replace older ones.
#[derive(Default)]
pub(crate) struct Notice {
    pub message: Option<String>,
    pub kind: NoticeKind,
    deadline: Option<Instant>,
}

impl Notice {
    pub(crate) fn show(&mut self, kind: NoticeKind, message: impl Into<String>) {
        self.message = Some(message.into());
        self.kind = kind;
        self.deadline = Some(Instant::now() + Duration::from_secs(4));
    }

    pub(crate) fn expire(&mut self) {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.message = None;
            self.deadline = None;
        }
    }
}
