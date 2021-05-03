use byteorder::{BigEndian, ByteOrder, LittleEndian, ReadBytesExt};
use exif::{Rational, SRational, Value};
use std::io;
use std::io::{Cursor, Error, ErrorKind, Read};

// See: https://www.media.mit.edu/pia/Research/deepview/exif.html#DataForm
const TYPE_UBYTE: u16 = 1;
const TYPE_ASCII: u16 = 2;
const TYPE_USHORT: u16 = 3;
const TYPE_ULONG: u16 = 4;
const TYPE_URATIONAL: u16 = 5;
const TYPE_BYTE: u16 = 6;
const TYPE_UNDEFINED: u16 = 7;
const TYPE_SHORT: u16 = 8;
const TYPE_LONG: u16 = 9;
const TYPE_RATIONAL: u16 = 10;
const TYPE_FLOAT: u16 = 11;
const TYPE_DOUBLE: u16 = 12;

const IFD_BIG_ENDIAN: u16 = 0x4d4d;
const IFD_LITTLE_ENDIAN: u16 = 0x4949;

pub(in crate) struct IfdEntry {
    pub tag: u16,
    pub value: Value,
}

pub(in crate) fn parse_canon_makernote(data: &[u8]) -> io::Result<Vec<IfdEntry>> {
    // Read the footer
    let mut cursor = Cursor::new(data[data.len() - 8..].to_vec());
    let footer_endian = cursor.read_u16::<BigEndian>()?;
    if footer_endian == IFD_LITTLE_ENDIAN {
        parse_canon_helper::<LittleEndian>(data)
    } else if footer_endian == IFD_BIG_ENDIAN {
        parse_canon_helper::<BigEndian>(data)
    } else {
        Err(Error::from(ErrorKind::InvalidInput))
    }
}

fn parse_canon_helper<E: ByteOrder>(data: &[u8]) -> io::Result<Vec<IfdEntry>> {
    // Read the footer
    let mut cursor = Cursor::new(data[data.len() - 8..].to_vec());
    // ignored
    let _footer_endian = cursor.read_u16::<E>()?;
    let fourty_two = cursor.read_u16::<E>()?;
    assert_eq!(fourty_two, 42);
    // The original offset of the maker note. All pointers are relative to this address, so we must
    // pad the buffer with this many bytes
    let original_offset = cursor.read_u32::<E>()? as isize;

    parse_ifd::<E>(data, -original_offset)
}

fn parse_ifd<E: ByteOrder>(data: &[u8], pointer_fixup: isize) -> io::Result<Vec<IfdEntry>> {
    let mut cursor = Cursor::new(data.to_vec());
    let entry_count = cursor.read_u16::<E>()?;

    let mut entries = vec![];
    for _ in 0..entry_count {
        let tag = cursor.read_u16::<E>()?;
        let value_type = cursor.read_u16::<E>()?;
        let element_width = type_width(value_type)?;
        let element_count = cursor.read_u32::<E>()?;
        let data_bytes = element_width
            .checked_mul(element_count as usize)
            .ok_or_else(|| Error::from(ErrorKind::InvalidInput))?;
        let value = if data_bytes <= 4 {
            // value(s) is inline
            let mut temp = [0u8; 4];
            cursor.read_exact(&mut temp)?;
            parse_value::<E>(value_type, &temp[..data_bytes])?
        } else {
            let data_ptr = (cursor.read_u32::<E>()? as isize) + pointer_fixup;
            if data_ptr < 0 || data_ptr + data_bytes as isize >= data.len() as isize {
                return Err(Error::from(ErrorKind::InvalidInput));
            }
            let data_ptr = data_ptr as usize;
            parse_value::<E>(value_type, &data[data_ptr..(data_ptr + data_bytes)])?
        };
        entries.push(IfdEntry { tag, value });
    }

    Ok(entries)
}

fn parse_value<E: ByteOrder>(data_type: u16, data: &[u8]) -> io::Result<Value> {
    Ok(match data_type {
        TYPE_BYTE => Value::SByte(data.iter().map(|x| *x as i8).collect()),
        TYPE_UBYTE => Value::Byte(data.to_vec()),
        TYPE_ASCII => Value::Ascii(data.split(|x| *x == 0).map(|x| x.to_vec()).collect()),
        // TODO: is it safe to pass zero here?
        TYPE_UNDEFINED => Value::Undefined(data.to_vec(), 0),
        TYPE_SHORT => {
            let mut value = vec![0i16; data.len() / type_width(data_type)?];
            E::read_i16_into(data, &mut value);
            Value::SShort(value)
        }
        TYPE_USHORT => {
            let mut value = vec![0u16; data.len() / type_width(data_type)?];
            E::read_u16_into(data, &mut value);
            Value::Short(value)
        }
        TYPE_LONG => {
            let mut value = vec![0i32; data.len() / type_width(data_type)?];
            E::read_i32_into(data, &mut value);
            Value::SLong(value)
        }
        TYPE_ULONG => {
            let mut value = vec![0u32; data.len() / type_width(data_type)?];
            E::read_u32_into(data, &mut value);
            Value::Long(value)
        }
        TYPE_FLOAT => {
            let mut value = vec![0f32; data.len() / type_width(data_type)?];
            E::read_f32_into(data, &mut value);
            Value::Float(value)
        }
        TYPE_RATIONAL => {
            let mut value = vec![0i32; 2 * data.len() / type_width(data_type)?];
            E::read_i32_into(data, &mut value);
            let (numerators, denominators): (Vec<i32>, Vec<i32>) =
                value.iter().partition(|x| **x % 2 == 0);
            Value::SRational(
                numerators
                    .iter()
                    .zip(denominators.iter())
                    .map(|(x, y)| SRational::from((*x, *y)))
                    .collect(),
            )
        }
        TYPE_URATIONAL => {
            let mut value = vec![0u32; 2 * data.len() / type_width(data_type)?];
            E::read_u32_into(data, &mut value);
            let (numerators, denominators): (Vec<u32>, Vec<u32>) =
                value.iter().partition(|x| **x % 2 == 0);
            Value::Rational(
                numerators
                    .iter()
                    .zip(denominators.iter())
                    .map(|(x, y)| Rational::from((*x, *y)))
                    .collect(),
            )
        }
        TYPE_DOUBLE => {
            let mut value = vec![0f64; data.len() / type_width(data_type)?];
            E::read_f64_into(data, &mut value);
            Value::Double(value)
        }
        _ => return Err(Error::from(ErrorKind::InvalidInput)),
    })
}

fn type_width(data_type: u16) -> io::Result<usize> {
    Ok(match data_type {
        TYPE_BYTE | TYPE_UBYTE | TYPE_ASCII | TYPE_UNDEFINED => 1,
        TYPE_SHORT | TYPE_USHORT => 2,
        TYPE_LONG | TYPE_ULONG | TYPE_FLOAT => 4,
        TYPE_RATIONAL | TYPE_URATIONAL | TYPE_DOUBLE => 8,
        _ => return Err(Error::from(ErrorKind::InvalidData)),
    })
}
