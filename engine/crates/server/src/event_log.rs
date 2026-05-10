use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::Path;

use cex_proto::{EngineEvent, SequencedEngineEvent};
use tracing::warn;

pub(crate) struct EventLog {
    file: BufWriter<File>,
    sequence: u64,
}

impl EventLog {
    pub(crate) fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let sequence = Self::find_last_seq(&path)?;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self {
            file: BufWriter::new(file),
            sequence,
        })
    }

    pub(crate) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) fn append_next(&mut self, event: EngineEvent) -> io::Result<SequencedEngineEvent> {
        let sequenced = SequencedEngineEvent {
            seq: self.sequence.saturating_add(1),
            event,
        };
        let payload = serde_json::to_vec(&sequenced).map_err(io::Error::other)?;
        let len = u32::try_from(payload.len()).map_err(io::Error::other)?;
        self.file.write_all(&len.to_be_bytes())?;
        self.file.write_all(&payload)?;
        self.file.flush()?;
        self.sequence = sequenced.seq;
        Ok(sequenced)
    }

    fn find_last_seq(path: &Path) -> io::Result<u64> {
        let entries = Self::replay(path)?;
        Ok(entries.last().map_or(0, |event| event.seq))
    }

    fn replay(path: &Path) -> io::Result<Vec<SequencedEngineEvent>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };

        let mut events = Vec::new();
        let mut last_good_pos = 0_u64;
        loop {
            let pos = file.stream_position()?;
            match read_one_event(&mut file) {
                Ok(Some(event)) => {
                    last_good_pos = file.stream_position()?;
                    events.push(event);
                }
                Ok(None) => break,
                Err(error) => {
                    warn!(
                        error = %error,
                        pos,
                        last_good_pos,
                        "event_log.recovery.truncate_corrupt_tail"
                    );
                    let writable = OpenOptions::new().write(true).open(path)?;
                    writable.set_len(last_good_pos)?;
                    break;
                }
            }
        }
        Ok(events)
    }
}

fn read_one_event(reader: &mut impl Read) -> io::Result<Option<SequencedEngineEvent>> {
    let mut len_buf = [0_u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }

    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0_u8; len];
    reader.read_exact(&mut payload)?;
    let event = serde_json::from_slice(&payload).map_err(io::Error::other)?;
    Ok(Some(event))
}
