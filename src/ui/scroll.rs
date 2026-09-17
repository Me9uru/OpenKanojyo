pub(super) struct ScrollState {
    pub(super) offset: u16,
    pub(super) follow_tail: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            follow_tail: true,
        }
    }
}

impl ScrollState {
    pub(super) fn up(&mut self, amount: u16) {
        self.offset = self.offset.saturating_sub(amount);
        self.follow_tail = false;
    }

    pub(super) fn down(&mut self, amount: u16) {
        self.offset = self.offset.saturating_add(amount);
        self.follow_tail = false;
    }

    pub(super) fn resolve(&mut self, content_height: usize, viewport_height: usize) -> u16 {
        let max = content_height
            .saturating_sub(viewport_height)
            .min(u16::MAX as usize) as u16;
        self.offset = if self.follow_tail {
            max
        } else {
            self.offset.min(max)
        };
        self.follow_tail = self.offset == max;
        self.offset
    }
}
