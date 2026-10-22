use std::io::{BufWriter, Read, Write};

pub(super) fn copy_exact<R: Read, W: Write, P: FnMut(u64)>(
    reader: &mut R,
    writer: &mut W,
    expected_size: u64,
    mut progress: P,
) -> Result<(), String> {
    let mut output = BufWriter::with_capacity(256 * 1024, writer);
    let result = (|| {
        let mut buffer = [0; 256 * 1024];
        let mut count = 0u64;
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            count = count.checked_add(read as u64).ok_or("copy_size_mismatch")?;
            if count > expected_size {
                return Err("copy_size_mismatch".into());
            }
            output
                .write_all(&buffer[..read])
                .map_err(|error| error.to_string())?;
            progress(count);
        }
        if count != expected_size {
            return Err("copy_size_mismatch".into());
        }
        output.flush().map_err(|error| error.to_string())
    })();
    // Drop must not flush an invalid tail or retry an already failed write.
    let _ = output.into_parts();
    result
}

#[cfg(test)]
mod tests {
    use super::copy_exact;
    use std::io::{self, Cursor, Read, Write};

    struct Fragments {
        bytes: Cursor<Vec<u8>>,
        fail_after_first: bool,
        reads: usize,
    }
    impl Read for Fragments {
        fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
            if self.fail_after_first && self.reads > 0 {
                return Err(io::Error::other("source_read_failed"));
            }
            self.reads += 1;
            let limit = target.len().min(4096);
            self.bytes.read(&mut target[..limit])
        }
    }
    #[derive(Default)]
    struct CountedWriter {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
        limit: Option<usize>,
        zero: bool,
        fail_after: Option<usize>,
        fail_flush: bool,
    }
    impl Write for CountedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.zero {
                return Ok(0);
            }
            if self
                .fail_after
                .is_some_and(|limit| self.bytes.len() >= limit)
            {
                return Err(io::Error::other("destination_write_failed"));
            }
            let size = bytes.len().min(self.limit.unwrap_or(usize::MAX));
            self.bytes.extend_from_slice(&bytes[..size]);
            Ok(size)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                return Err(io::Error::other("destination_flush_failed"));
            }
            Ok(())
        }
    }
    fn payload(size: usize) -> Vec<u8> {
        (0..size)
            .map(|index| (index.wrapping_mul(37) % 251) as u8)
            .collect()
    }
    fn fragments(bytes: Vec<u8>) -> Fragments {
        Fragments {
            bytes: Cursor::new(bytes),
            fail_after_first: false,
            reads: 0,
        }
    }

    #[test]
    fn fragments_are_coalesced_and_exact_tail_is_flushed() {
        let data = payload(1024 * 1024 + 17);
        let mut input = fragments(data.clone());
        let mut output = CountedWriter::default();
        let mut progress = Vec::new();
        copy_exact(&mut input, &mut output, data.len() as u64, |count| {
            progress.push(count)
        })
        .unwrap();
        assert_eq!(output.bytes, data);
        assert!(
            output.writes <= 5,
            "{} fragmented file writes instead of at most five",
            output.writes
        );
        assert_eq!(output.flushes, 1);
        assert_eq!(progress.last(), Some(&(data.len() as u64)));
        assert!(progress.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn empty_and_sub_buffer_tails_are_explicitly_flushed() {
        for size in [0, 17, 256 * 1024 - 1] {
            let data = payload(size);
            let mut output = CountedWriter::default();
            copy_exact(
                &mut fragments(data.clone()),
                &mut output,
                size as u64,
                |_| {},
            )
            .unwrap();
            assert_eq!(output.bytes, data);
            assert_eq!(output.flushes, 1);
        }
    }

    #[test]
    fn short_extra_and_unreachable_size_discard_unflushed_tail() {
        for expected in [7, 9, u64::MAX] {
            let mut output = CountedWriter::default();
            assert_eq!(
                copy_exact(&mut fragments(payload(8)), &mut output, expected, |_| {}),
                Err("copy_size_mismatch".into())
            );
            assert_eq!(
                output.writes, 0,
                "invalid tail was written during failure/drop"
            );
            assert_eq!(output.flushes, 0);
        }
    }

    #[test]
    fn reader_failure_discards_buffered_prefix() {
        let mut input = fragments(payload(8192));
        input.fail_after_first = true;
        let mut output = CountedWriter::default();
        assert_eq!(
            copy_exact(&mut input, &mut output, 8192, |_| {}),
            Err("source_read_failed".into())
        );
        assert_eq!(output.writes, 0);
        assert_eq!(output.flushes, 0);
    }

    #[test]
    fn short_writes_preserve_every_byte() {
        let data = payload(65536 + 17);
        let mut output = CountedWriter {
            limit: Some(101),
            ..Default::default()
        };
        copy_exact(
            &mut fragments(data.clone()),
            &mut output,
            data.len() as u64,
            |_| {},
        )
        .unwrap();
        assert_eq!(output.bytes, data);
        assert_eq!(output.flushes, 1);
    }

    #[test]
    fn zero_write_is_failure_without_drop_retry() {
        let mut output = CountedWriter {
            zero: true,
            ..Default::default()
        };
        assert!(copy_exact(&mut fragments(payload(17)), &mut output, 17, |_| {}).is_err());
        assert_eq!(output.writes, 1);
        assert_eq!(output.flushes, 0);
    }

    #[test]
    fn partially_failed_write_is_not_retried_on_drop() {
        let mut output = CountedWriter {
            limit: Some(13),
            fail_after: Some(13),
            ..Default::default()
        };
        assert_eq!(
            copy_exact(&mut fragments(payload(17)), &mut output, 17, |_| {}),
            Err("destination_write_failed".into())
        );
        assert_eq!(output.bytes.len(), 13);
        assert_eq!(output.writes, 2);
        assert_eq!(output.flushes, 0);
    }

    #[test]
    fn final_flush_error_is_never_success() {
        let mut output = CountedWriter {
            fail_flush: true,
            ..Default::default()
        };
        assert_eq!(
            copy_exact(&mut fragments(payload(17)), &mut output, 17, |_| {}),
            Err("destination_flush_failed".into())
        );
        assert_eq!(output.flushes, 1);
    }

    #[test]
    fn success_exposes_the_actual_file_tail_before_sync_or_hash() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let data = payload(17);
        copy_exact(&mut fragments(data.clone()), file.as_file_mut(), 17, |_| {}).unwrap();
        assert_eq!(file.as_file().metadata().unwrap().len(), 17);
        assert_eq!(std::fs::read(file.path()).unwrap(), data);
    }
}
