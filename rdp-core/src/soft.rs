use crate::cmd::{command_words, op, opcode};
use crate::raster;
use crate::rdram::Rdram;
use crate::state::{CycleType, State};

/// The software RDP: an interpreter for raw command words operating on an [`Rdram`].
///
/// This only implements what has been verified on real hardware (or is explicitly marked
/// provisional on the way there). Commands outside that set are counted in `unhandled` instead of
/// silently guessed - a differential test asserts the counter stayed at zero.
pub struct SoftRdp {
    pub state: State,
    /// Number of commands (or command/state combinations) hit that this implementation does not
    /// cover yet.
    pub unhandled: u32,
}

impl SoftRdp {
    pub fn new() -> Self {
        Self {
            state: State::default(),
            unhandled: 0,
        }
    }

    /// Runs a full command stream. `stream` is the same sequence of 64-bit words the real DP would
    /// fetch between DP_START and DP_END.
    pub fn run(&mut self, stream: &[u64], mem: &mut impl Rdram) {
        let mut index = 0;
        while index < stream.len() {
            let length = command_words(opcode(stream[index]));
            if index + length > stream.len() {
                // Truncated command: the real DP would stall waiting for more words.
                self.unhandled += 1;
                return;
            }
            self.execute(&stream[index..index + length], mem);
            index += length;
        }
    }

    fn execute(&mut self, command: &[u64], mem: &mut impl Rdram) {
        let word = command[0];
        match opcode(word) {
            op::NO_OP => {}
            // The syncs only affect pipeline timing, not results.
            op::SYNC_LOAD | op::SYNC_PIPE | op::SYNC_TILE | op::SYNC_FULL => {}
            op::SET_SCISSOR => self.state.scissor = word,
            op::SET_OTHER_MODES => self.state.other_modes = word,
            op::SET_FILL_COLOR => self.state.fill_color = word as u32,
            op::SET_BLEND_COLOR => self.state.blend_color = word as u32,
            op::SET_COLOR_IMAGE => self.state.color_image = word,
            op::FILL_RECTANGLE => match self.state.cycle_type() {
                CycleType::Fill => raster::fill_rectangle(&self.state, mem, word),
                CycleType::OneCycle => {
                    if !raster::one_cycle_rectangle(&self.state, mem, word) {
                        self.unhandled += 1;
                    }
                }
                _ => self.unhandled += 1,
            },
            _ => self.unhandled += 1,
        }
    }
}

impl Default for SoftRdp {
    fn default() -> Self {
        Self::new()
    }
}
