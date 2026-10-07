#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    KeyDown { key: u16, flags: u64, repeat: bool },
    KeyUp { key: u16 },
    FlagsChanged { flags: u64 },
    Disabled,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Commit,
    Cancel,
    Disabled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub suppress: bool,
    pub end: Option<End>,
}

#[derive(Default)]
pub struct CaptureState {
    active: bool,
    owned_tab_down: bool,
    owned_escape_down: bool,
    pub forward: u32,
    pub reverse: u32,
    pub committed: u32,
    pub cancelled: u32,
    pub end: Option<End>,
}

impl CaptureState {
    pub fn handle(&mut self, event: Event) -> Output {
        let mut suppress = false;
        let mut end = None;
        match event {
            Event::Disabled => {
                *self = Self::default();
                self.end = Some(End::Disabled);
                end = self.end;
            }
            Event::KeyDown {
                key: 48,
                flags,
                repeat,
            } => {
                if self.owned_tab_down {
                    suppress = true;
                } else if !repeat && flags & CMD != 0 && flags & (CTRL | OPTION) == 0 {
                    if !self.active {
                        self.active = true;
                    }
                    self.owned_tab_down = true;
                    suppress = true;
                    if flags & SHIFT != 0 {
                        self.reverse = self.reverse.saturating_add(1);
                    } else {
                        self.forward = self.forward.saturating_add(1);
                    }
                }
            }
            Event::KeyUp { key: 48 } if self.owned_tab_down => {
                self.owned_tab_down = false;
                suppress = true;
            }
            Event::KeyDown {
                key: 53,
                flags: _,
                repeat,
            } if self.active && !repeat => {
                self.owned_escape_down = true;
                self.active = false;
                self.end = Some(End::Cancel);
                self.cancelled = self.cancelled.saturating_add(1);
                suppress = true;
                end = self.end;
            }
            Event::KeyDown { key: 53, .. } if self.owned_escape_down => suppress = true,
            Event::KeyUp { key: 53 } if self.owned_escape_down => {
                self.owned_escape_down = false;
                suppress = true;
            }
            Event::FlagsChanged { flags } if self.active && flags & CMD == 0 => {
                self.active = false;
                self.end = Some(End::Commit);
                self.committed = self.committed.saturating_add(1);
                end = self.end;
            }
            _ => {}
        }
        Output { suppress, end }
    }
}

const CMD: u64 = 1 << 20;
const SHIFT: u64 = 1 << 17;
const CTRL: u64 = 1 << 18;
const OPTION: u64 = 1 << 19;

#[cfg(test)]
mod tests {
    use super::*;

    fn down(key: u16, flags: u64, repeat: bool) -> Event {
        Event::KeyDown { key, flags, repeat }
    }

    #[test]
    fn plain_tab_and_modified_command_tab_pass() {
        let mut s = CaptureState::default();
        assert!(!s.handle(down(48, 0, false)).suppress);
        assert!(!s.handle(Event::KeyUp { key: 48 }).suppress);
        assert!(!s.handle(down(48, CMD, true)).suppress);
        assert!(!s.handle(down(48, CMD | CTRL, false)).suppress);
        assert!(!s.handle(down(48, CMD | OPTION, false)).suppress);
        assert_eq!((s.forward, s.reverse), (0, 0));
    }

    #[test]
    fn forward_reverse_and_repeats() {
        let mut s = CaptureState::default();
        assert!(s.handle(down(48, CMD, false)).suppress);
        assert!(s.handle(down(48, CMD, true)).suppress);
        s.handle(Event::KeyUp { key: 48 });
        s.handle(down(48, CMD | SHIFT, false));
        assert_eq!((s.forward, s.reverse), (1, 1));
    }

    #[test]
    fn command_release_commits_but_owned_tab_up_is_suppressed() {
        let mut s = CaptureState::default();
        s.handle(down(48, CMD, false));
        let released = s.handle(Event::FlagsChanged { flags: 0 });
        assert_eq!(released.end, Some(End::Commit));
        assert!(!released.suppress);
        assert_eq!(s.committed, 1);
        assert!(s.handle(Event::KeyUp { key: 48 }).suppress);
    }

    #[test]
    fn escape_cancels_and_pairs_keyup() {
        let mut s = CaptureState::default();
        s.handle(down(48, CMD, false));
        assert_eq!(s.handle(down(53, CMD, false)).end, Some(End::Cancel));
        assert!(s.handle(down(53, CMD, true)).suppress);
        assert_eq!(s.handle(Event::FlagsChanged { flags: 0 }).end, None);
        assert_eq!(s.cancelled, 1);
        assert!(s.handle(Event::KeyUp { key: 53 }).suppress);
    }

    #[test]
    fn aggregate_command_stays_active_until_all_command_keys_are_up() {
        let mut s = CaptureState::default();
        s.handle(down(48, CMD, false));
        assert_eq!(s.handle(Event::FlagsChanged { flags: CMD }).end, None);
        assert_eq!(
            s.handle(Event::FlagsChanged { flags: 0 }).end,
            Some(End::Commit)
        );
    }

    #[test]
    fn disable_resets_state_and_passes_the_disabling_event() {
        let mut s = CaptureState::default();
        s.handle(down(48, CMD, false));
        let output = s.handle(Event::Disabled);
        assert!(!output.suppress);
        assert_eq!(output.end, Some(End::Disabled));
        assert_eq!((s.forward, s.reverse), (0, 0));
        assert!(!s.handle(Event::KeyUp { key: 48 }).suppress);
        assert!(!s.handle(Event::KeyUp { key: 53 }).suppress);
    }
}
