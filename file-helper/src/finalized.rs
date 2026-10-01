use std::io::{Read, Seek, SeekFrom};

#[cfg(test)]
pub fn finalized(reader: &mut (impl Read + Seek), size: u64) -> bool {
    duration_ms(reader, size).is_some()
}
// The frontend uses exact JavaScript integers for milliseconds.
const MAX_DURATION_MS: u64 = 9_007_199_254_740_991;
pub fn duration_ms(reader: &mut (impl Read + Seek), size: u64) -> Option<u64> {
    fn check(r: &mut (impl Read + Seek), size: u64) -> std::io::Result<Option<u64>> {
        r.seek(SeekFrom::Start(0))?;
        let mut magic = [0; 4];
        r.read_exact(&mut magic)?;
        if magic == [0x1a, 0x45, 0xdf, 0xa3] {
            ebml(r, 0, size, 0)
        } else {
            mp4(r, 0, size, false)
        }
    }
    check(reader, size).ok().flatten()
}
fn mp4(
    r: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    movie: bool,
) -> std::io::Result<Option<u64>> {
    let mut at = start;
    let mut ftyp = false;
    let mut duration = None;
    let mut count = 0;
    while at < end {
        count += 1;
        if count > 20_000 || end - at < 8 {
            return Ok(None);
        }
        r.seek(SeekFrom::Start(at))?;
        let mut header = [0; 8];
        r.read_exact(&mut header)?;
        let short = u32::from_be_bytes(header[..4].try_into().unwrap());
        let (length, head) = if short == 1 {
            let mut long = [0; 8];
            r.read_exact(&mut long)?;
            (u64::from_be_bytes(long), 16)
        } else {
            (short as u64, 8)
        };
        if length < head || length > end - at {
            return Ok(None);
        }
        match &header[4..] {
            b"ftyp" if !movie => ftyp = true,
            b"moov" if !movie => duration = mp4(r, at + head, at + length, true)?,
            b"mvhd" if movie => {
                let mut version = [0; 1];
                r.read_exact(&mut version)?;
                let offset = if version[0] == 0 {
                    16
                } else if version[0] == 1 {
                    24
                } else {
                    return Ok(None);
                };
                let width = if version[0] == 0 { 4 } else { 8 };
                if length - head < offset + width {
                    return Ok(None);
                }
                r.seek(SeekFrom::Start(at + head + offset - 4))?;
                let mut scale = [0; 4];
                r.read_exact(&mut scale)?;
                let scale = u32::from_be_bytes(scale) as u128;
                r.seek(SeekFrom::Start(at + head + offset))?;
                let mut data = [0; 8];
                r.read_exact(&mut data[8 - width as usize..])?;
                let ticks = u64::from_be_bytes(data);
                if ticks == 0
                    || ticks == u64::MAX
                    || (width == 4 && ticks == u32::MAX as u64)
                    || scale == 0
                {
                    return Ok(None);
                }
                let millis = (ticks as u128 * 1000 + scale / 2) / scale;
                duration = (millis <= MAX_DURATION_MS as u128).then_some(millis as u64);
            }
            _ => {}
        }
        at += length;
    }
    Ok(if movie || ftyp { duration } else { None })
}
fn vint(r: &mut impl Read, id: bool) -> std::io::Result<(u64, bool)> {
    let mut byte = [0; 1];
    r.read_exact(&mut byte)?;
    let length = byte[0].leading_zeros() + 1;
    if length > 8 || (id && length > 4) {
        return Err(std::io::ErrorKind::InvalidData.into());
    }
    let mask = 0x80u8 >> (length - 1);
    let mut value = if id {
        byte[0] as u64
    } else {
        (byte[0] & !mask) as u64
    };
    for _ in 1..length {
        r.read_exact(&mut byte)?;
        value = (value << 8) | byte[0] as u64;
    }
    Ok((value, !id && value == (1u64 << (7 * length)) - 1))
}
fn ebml(
    r: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    level: u8,
) -> std::io::Result<Option<u64>> {
    let mut at = start;
    let mut count = 0;
    let mut duration = None;
    let mut scale = 1_000_000u64;
    while at < end {
        count += 1;
        if count > 20_000 {
            return Ok(None);
        }
        r.seek(SeekFrom::Start(at))?;
        let (id, _) = vint(r, true)?;
        let (length, unknown) = vint(r, false)?;
        let payload = r.stream_position()?;
        let stop = if unknown && id == 0x18538067 {
            end
        } else if unknown {
            return Ok(None);
        } else {
            match payload.checked_add(length) {
                Some(stop) if stop <= end => stop,
                _ => return Ok(None),
            }
        };
        if (level == 0 && id == 0x18538067) || (level == 1 && id == 0x1549a966) {
            return ebml(r, payload, stop, level + 1);
        }
        if level == 2 && id == 0x4489 {
            let value = if length == 8 {
                let mut data = [0; 8];
                r.read_exact(&mut data)?;
                f64::from_be_bytes(data)
            } else if length == 4 {
                let mut data = [0; 4];
                r.read_exact(&mut data)?;
                f32::from_be_bytes(data) as f64
            } else {
                return Ok(None);
            };
            if !value.is_finite() || value <= 0.0 {
                return Ok(None);
            }
            duration = Some(value);
        }
        if level == 2 && id == 0x2ad7b1 {
            if !(1..=8).contains(&length) {
                return Ok(None);
            }
            let mut data = [0; 8];
            r.read_exact(&mut data[8 - length as usize..])?;
            scale = u64::from_be_bytes(data);
            if scale == 0 {
                return Ok(None);
            }
        }
        at = stop;
    }
    Ok(duration.and_then(|ticks| {
        let millis = (ticks * scale as f64 / 1_000_000.0).round();
        (millis.is_finite() && millis <= MAX_DURATION_MS as f64).then_some(millis as u64)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut bytes = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        bytes.extend(kind);
        bytes.extend(payload);
        bytes
    }
    pub fn completed_mp4() -> Vec<u8> {
        let mut movie_header = [0; 20];
        movie_header[12..16].copy_from_slice(&1000u32.to_be_bytes());
        movie_header[16..20].copy_from_slice(&100u32.to_be_bytes());
        [
            atom(b"ftyp", b"isom0000"),
            atom(b"mdat", b"sample"),
            atom(b"moov", &atom(b"mvhd", &movie_header)),
        ]
        .concat()
    }
    #[test]
    fn movie_duration_respects_both_versions_and_timescale() {
        for version in [0u8, 1] {
            let (scale_at, duration_at, size) = if version == 0 {
                (12, 16, 20)
            } else {
                (20, 24, 32)
            };
            let mut header = vec![0; size];
            header[0] = version;
            header[scale_at..scale_at + 4].copy_from_slice(&90_000u32.to_be_bytes());
            if version == 0 {
                header[duration_at..].copy_from_slice(&11_116_260u32.to_be_bytes());
            } else {
                header[duration_at..].copy_from_slice(&11_116_260u64.to_be_bytes());
            }
            let bytes = [
                atom(b"ftyp", b"isom0000"),
                atom(b"moov", &atom(b"mvhd", &header)),
            ]
            .concat();
            assert_eq!(
                duration_ms(&mut Cursor::new(&bytes), bytes.len() as u64),
                Some(123_514)
            );
            header[scale_at..scale_at + 4].fill(0);
            let bytes = [
                atom(b"ftyp", b"isom0000"),
                atom(b"moov", &atom(b"mvhd", &header)),
            ]
            .concat();
            assert_eq!(
                duration_ms(&mut Cursor::new(&bytes), bytes.len() as u64),
                None
            );
        }
    }
    #[test]
    fn webm_duration_respects_scale_even_when_scale_follows_duration() {
        for scale_first in [true, false] {
            let duration = [vec![0x44, 0x89, 0x88], 61.757f64.to_be_bytes().to_vec()].concat();
            let scale = vec![0x2a, 0xd7, 0xb1, 0x83, 0x1e, 0x84, 0x80]; // 2,000,000 ns
            let info = if scale_first {
                [scale, duration].concat()
            } else {
                [duration, scale].concat()
            };
            let bytes = [
                vec![
                    0x1a,
                    0x45,
                    0xdf,
                    0xa3,
                    0x80,
                    0x18,
                    0x53,
                    0x80,
                    0x67,
                    0xff,
                    0x15,
                    0x49,
                    0xa9,
                    0x66,
                    0x80 | info.len() as u8,
                ],
                info,
            ]
            .concat();
            assert_eq!(
                duration_ms(&mut Cursor::new(&bytes), bytes.len() as u64),
                Some(124)
            );
        }
    }
    #[test]
    fn completed_movie_requires_bounded_atoms_and_nonzero_duration() {
        let bytes = completed_mp4();
        assert!(finalized(&mut Cursor::new(&bytes), bytes.len() as u64));
        for count in [8, 19, bytes.len() - 1] {
            assert!(!finalized(&mut Cursor::new(&bytes[..count]), count as u64));
        }
        let mut unfinished = atom(b"ftyp", b"isom0000");
        unfinished.extend([0, 0, 0, 0, b'm', b'd', b'a', b't']);
        unfinished.extend(b"recording");
        assert!(!finalized(
            &mut Cursor::new(&unfinished),
            unfinished.len() as u64
        ));
    }
    #[test]
    fn webm_requires_finite_positive_final_duration() {
        let mut bytes = vec![
            0x1a, 0x45, 0xdf, 0xa3, 0x80, 0x18, 0x53, 0x80, 0x67, 0xff, 0x15, 0x49, 0xa9, 0x66,
            0x8b, 0x44, 0x89, 0x88,
        ];
        bytes.extend(1000f64.to_be_bytes());
        assert!(finalized(&mut Cursor::new(&bytes), bytes.len() as u64));
        bytes.truncate(bytes.len() - 8);
        bytes.extend(f64::NAN.to_be_bytes());
        assert!(!finalized(&mut Cursor::new(&bytes), bytes.len() as u64));
    }
}
