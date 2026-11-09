use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum NoticeLevel {
    #[default]
    Info,
    Warning,
    Error,
}

impl NoticeLevel {
    pub(crate) fn color(self) -> Color32 {
        match self {
            Self::Info => Color32::from_rgb(110, 205, 195),
            Self::Warning => Color32::from_rgb(245, 190, 90),
            Self::Error => Color32::from_rgb(235, 80, 90),
        }
    }
}

/// Header toast. Errors queue; lower-priority messages never interrupt them.
#[derive(Default)]
pub(crate) struct Notice {
    pub message: Option<String>,
    pub level: NoticeLevel,
    deadline: Option<Instant>,
    pending_errors: VecDeque<String>,
}

impl Notice {
    pub(crate) fn show(&mut self, level: NoticeLevel, message: impl Into<String>) {
        self.expire();
        let message = message.into();
        if self.message.is_some() {
            if level < self.level {
                return;
            }
            if level == NoticeLevel::Error && self.level == NoticeLevel::Error {
                if self.message.as_ref() != Some(&message)
                    && !self.pending_errors.contains(&message)
                {
                    self.pending_errors.push_back(message);
                }
                return;
            }
        }
        self.display(level, message);
    }

    fn display(&mut self, level: NoticeLevel, message: String) {
        self.message = Some(message);
        self.level = level;
        self.deadline = Some(Instant::now() + Duration::from_secs(4));
    }

    pub(crate) fn expire(&mut self) {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.message = None;
            self.deadline = None;
            if let Some(message) = self.pending_errors.pop_front() {
                self.display(NoticeLevel::Error, message);
            }
        }
    }
}
