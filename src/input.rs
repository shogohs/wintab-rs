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
pub enum Direction {
    Forward,
    Reverse,
}

pub fn candidate_owner_pids(entries: &[(i32, i32)], self_pid: i32) -> Vec<i32> {
    let mut owners = Vec::new();
    for &(pid, layer) in entries {
        if pid > 0 && pid != self_pid && layer == 0 && !owners.contains(&pid) {
            owners.push(pid);
        }
    }
    owners
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    pub suppress: bool,
    pub end: Option<End>,
    pub step: Option<Direction>,
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
        let mut step = None;
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
                } else if repeat {
                    // An unpaired repeat cannot start or cancel a selection.
                } else if flags & CMD != 0 && flags & (CTRL | OPTION) == 0 {
                    if !self.active {
                        self.active = true;
                    }
                    self.owned_tab_down = true;
                    suppress = true;
                    if flags & SHIFT != 0 {
                        self.reverse = self.reverse.saturating_add(1);
                        step = Some(Direction::Reverse);
                    } else {
                        self.forward = self.forward.saturating_add(1);
                        step = Some(Direction::Forward);
                    }
                } else if self.active {
                    self.active = false;
                    self.end = Some(End::Cancel);
                    self.cancelled = self.cancelled.saturating_add(1);
                    end = self.end;
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
            Event::KeyDown { .. } if self.active => {
                self.active = false;
                self.end = Some(End::Cancel);
                self.cancelled = self.cancelled.saturating_add(1);
                end = self.end;
            }
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
        Output {
            suppress,
            end,
            step,
        }
    }

    pub fn handle_owned_tail(&mut self, event: Event) -> Output {
        let suppress = match event {
            Event::KeyDown { key: 48, .. } if self.owned_tab_down => true,
            Event::KeyUp { key: 48 } if self.owned_tab_down => {
                self.owned_tab_down = false;
                true
            }
            Event::KeyDown { key: 53, .. } if self.owned_escape_down => true,
            Event::KeyUp { key: 53 } if self.owned_escape_down => {
                self.owned_escape_down = false;
                true
            }
            _ => false,
        };
        Output {
            suppress,
            end: None,
            step: None,
        }
    }

    pub fn has_owned_keys(&self) -> bool {
        self.owned_tab_down || self.owned_escape_down
    }
}

const CMD: u64 = 1 << 20;
const SHIFT: u64 = 1 << 17;
const CTRL: u64 = 1 << 18;
const OPTION: u64 = 1 << 19;

pub struct Selection {
    count: usize,
    cursor: Option<usize>,
    end: Option<End>,
    commit_taken: bool,
}

impl Selection {
    pub fn new(count: usize, focused: Option<usize>) -> Option<Self> {
        (count > 0).then_some(Self {
            count,
            cursor: focused.filter(|index| *index < count),
            end: None,
            commit_taken: false,
        })
    }

    pub fn step(&mut self, direction: Direction) {
        if self.end.is_some() || self.count == 0 {
            return;
        }
        self.cursor = Some(match (self.cursor, direction) {
            (Some(index), Direction::Forward) => (index + 1) % self.count,
            (Some(0), Direction::Reverse) => self.count - 1,
            (Some(index), Direction::Reverse) => index - 1,
            (None, Direction::Forward) => 0,
            (None, Direction::Reverse) => self.count - 1,
        });
    }

    pub fn finish(&mut self, end: End) {
        if self.end.is_none() {
            self.end = Some(end);
        }
    }

    pub fn cancel(&mut self) {
        self.end = Some(End::Cancel);
        self.cursor = None;
        self.commit_taken = true;
    }

    pub fn disable(&mut self) {
        self.end = Some(End::Disabled);
        self.cursor = None;
        self.commit_taken = true;
    }

    pub fn take_commit(&mut self) -> Option<usize> {
        if self.end == Some(End::Commit) && !self.commit_taken {
            self.commit_taken = true;
            self.cursor
        } else {
            None
        }
    }

    pub fn terminal(&self) -> bool {
        self.end.is_some()
    }

    pub fn selected(&self) -> Option<usize> {
        self.cursor
    }
}

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

    #[test]
    fn selection_handles_empty_single_and_wrapping_lists() {
        assert!(Selection::new(0, None).is_none());
        let mut one = Selection::new(1, Some(0)).unwrap();
        one.step(Direction::Forward);
        assert_eq!(one.take_commit(), None);
        one.finish(End::Commit);
        assert_eq!(one.take_commit(), Some(0));
        assert_eq!(one.take_commit(), None);

        let mut many = Selection::new(3, Some(1)).unwrap();
        many.step(Direction::Forward);
        assert_eq!(many.cursor, Some(2));
        many.step(Direction::Forward);
        assert_eq!(many.cursor, Some(0));
        many.step(Direction::Reverse);
        assert_eq!(many.cursor, Some(2));
        many.finish(End::Commit);
        many.step(Direction::Forward);
        assert_eq!(many.take_commit(), Some(2));
        assert_eq!(many.take_commit(), None);
    }

    #[test]
    fn selection_fallback_and_terminal_states_never_commit() {
        let mut first = Selection::new(3, Some(9)).unwrap();
        first.step(Direction::Forward);
        assert_eq!(first.cursor, Some(0));
        let mut last = Selection::new(3, None).unwrap();
        last.step(Direction::Reverse);
        assert_eq!(last.cursor, Some(2));

        let mut cancelled = Selection::new(2, None).unwrap();
        cancelled.finish(End::Cancel);
        cancelled.step(Direction::Forward);
        assert_eq!(cancelled.take_commit(), None);
        assert_eq!(cancelled.cursor, None);
        let mut disabled = Selection::new(2, None).unwrap();
        disabled.finish(End::Disabled);
        assert_eq!(disabled.take_commit(), None);
        let mut disabled_after_commit = Selection::new(2, Some(0)).unwrap();
        disabled_after_commit.step(Direction::Forward);
        disabled_after_commit.finish(End::Commit);
        disabled_after_commit.disable();
        assert_eq!(disabled_after_commit.take_commit(), None);
        let mut timed_out_pending_commit = Selection::new(2, Some(0)).unwrap();
        timed_out_pending_commit.step(Direction::Forward);
        timed_out_pending_commit.finish(End::Commit);
        timed_out_pending_commit.cancel();
        assert_eq!(timed_out_pending_commit.take_commit(), None);
        let mut timeout = Selection::new(2, None).unwrap();
        assert_eq!(timeout.take_commit(), None);
    }

    #[test]
    fn repeats_do_not_advance_selection() {
        let mut state = CaptureState::default();
        let mut selection = Selection::new(4, Some(1)).unwrap();
        let first = state.handle(down(48, CMD, false));
        selection.step(first.step.unwrap());
        let repeat = state.handle(down(48, CMD, true));
        assert!(repeat.suppress);
        assert_eq!(repeat.step, None);
        assert_eq!(selection.cursor, Some(2));
    }

    #[test]
    fn terminal_owned_repeats_stay_suppressed_without_advancing() {
        let mut state = CaptureState::default();
        let mut selection = Selection::new(3, None).unwrap();
        let first = state.handle(down(48, CMD, false));
        selection.step(first.step.unwrap());
        selection.finish(End::Commit);
        assert!(state.handle_owned_tail(down(48, CMD, true)).suppress);
        assert!(state.handle_owned_tail(down(48, CMD, false)).suppress);
        assert_eq!(selection.cursor, Some(0));
    }

    #[test]
    fn ordinary_key_and_modified_tab_cancel_but_pass_through() {
        let mut state = CaptureState::default();
        state.handle(down(48, CMD, false));
        state.handle(Event::KeyUp { key: 48 });
        let ordinary = state.handle(down(0, CMD, false));
        assert!(!ordinary.suppress);
        assert_eq!(ordinary.end, Some(End::Cancel));
        assert_eq!(state.handle(Event::FlagsChanged { flags: 0 }).end, None);

        let mut state = CaptureState::default();
        state.handle(down(48, CMD, false));
        state.handle(Event::KeyUp { key: 48 });
        let modified = state.handle(down(48, CMD | OPTION, false));
        assert!(!modified.suppress);
        assert_eq!(modified.end, Some(End::Cancel));
    }

    #[test]
    fn candidate_owner_dedup_keeps_first_front_to_back_order_and_exclusions() {
        let visible = [(40, 0), (41, 1), (42, 0), (40, 0), (-1, 0), (0, 0), (1, 0)];
        let all = [(40, 0), (43, 0), (42, 0)];
        let entries: Vec<_> = visible.into_iter().chain(all).collect();
        assert_eq!(candidate_owner_pids(&entries, 1), vec![40, 42, 43]);
    }
}
