use std::io::{Read, Seek, SeekFrom};

pub fn finalized(reader: &mut (impl Read + Seek), size: u64) -> bool {
    fn check(r: &mut (impl Read + Seek), size: u64) -> std::io::Result<bool> {
        r.seek(SeekFrom::Start(0))?;
        let mut magic = [0; 4];
        r.read_exact(&mut magic)?;
        if magic == [0x1a, 0x45, 0xdf, 0xa3] {
            ebml(r, 0, size, 0)
        } else {
            mp4(r, 0, size, false)
        }
    }
    check(reader, size).unwrap_or(false)
}
fn mp4(r: &mut (impl Read + Seek), start: u64, end: u64, movie: bool) -> std::io::Result<bool> {
    let mut at = start;
    let mut ftyp = false;
    let mut duration = false;
    let mut count = 0;
    while at < end {
        count += 1;
        if count > 20_000 || end - at < 8 {
            return Ok(false);
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
            return Ok(false);
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
                    return Ok(false);
                };
                let width = if version[0] == 0 { 4 } else { 8 };
                if length - head < offset + width {
                    return Ok(false);
                }
                r.seek(SeekFrom::Start(at + head + offset))?;
                let mut data = [0; 8];
                r.read_exact(&mut data[8 - width as usize..])?;
                duration = u64::from_be_bytes(data) > 0;
            }
            _ => {}
        }
        at += length;
    }
    Ok(duration && (movie || ftyp))
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
fn ebml(r: &mut (impl Read + Seek), start: u64, end: u64, level: u8) -> std::io::Result<bool> {
    let mut at = start;
    let mut count = 0;
    while at < end {
        count += 1;
        if count > 20_000 {
            return Ok(false);
        }
        r.seek(SeekFrom::Start(at))?;
        let (id, _) = vint(r, true)?;
        let (length, unknown) = vint(r, false)?;
        let payload = r.stream_position()?;
        let stop = if unknown && id == 0x18538067 {
            end
        } else if unknown {
            return Ok(false);
        } else {
            match payload.checked_add(length) {
                Some(stop) if stop <= end => stop,
                _ => return Ok(false),
            }
        };
        if (level == 0 && id == 0x18538067) || (level == 1 && id == 0x1549a966) {
            return ebml(r, payload, stop, level + 1);
        }
        if level == 2 && id == 0x4489 {
            let duration = if length == 8 {
                let mut data = [0; 8];
                r.read_exact(&mut data)?;
                f64::from_be_bytes(data)
            } else if length == 4 {
                let mut data = [0; 4];
                r.read_exact(&mut data)?;
                f32::from_be_bytes(data) as f64
            } else {
                return Ok(false);
            };
            return Ok(duration.is_finite() && duration > 0.0);
        }
        at = stop;
    }
    Ok(false)
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
