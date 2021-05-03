use crate::error::Error;
use crate::ifd::parse_canon_makernote;
use exif::{Exif, In, Rational, Tag, Value};
use std::path::Path;
use std::str::FromStr;

const SENSITIVITY_TYPE_SOS: u16 = 1;
const SENSITIVITY_TYPE_REI: u16 = 2;
const SENSITIVITY_TYPE_ISO: u16 = 3;
const SENSITIVITY_TYPE_SOS_AND_REI: u16 = 4;
const SENSITIVITY_TYPE_SOS_AND_ISO: u16 = 5;
const SENSITIVITY_TYPE_REI_AND_ISO: u16 = 6;
const SENSITIVITY_TYPE_SOS_AND_REI_AND_ISO: u16 = 7;

const TAG_CANON_SHOTINFO: u16 = 4;

const SHOTINFO_CAMERA_TEMPERATURE: usize = 12;

#[derive(Debug)]
pub(in crate) struct ImageMetadata {
    camera_model: String,
    camera_serial_number: String,
    // Generally ISO, but may also be REI or SOS
    sensor_sensitivity: u32,
    // Type of sensitivity used, as defined for EXIF tag 0x8830
    sensitivity_type: u16,
    // Time in seconds
    exposure_time: f32,
    // Temperature in C
    temperature: f32,
}

// Convert the given ascii data to an integer
fn atoi(data: &[u8]) -> Result<u8, Error> {
    if data.len() > 2 {
        return Err(Error::InvalidData("Data too long".to_string()));
    }
    let utf8 = String::from_utf8(data.to_vec())
        .map_err(|_| Error::InvalidData("Received non-UTF8 characters".to_string()))?;
    u8::from_str(&utf8).map_err(|_| {
        Error::InvalidData("Expected numeric ascii digits in ExifVersion field".to_string())
    })
}

fn get_exif_version(exif: &Exif) -> Result<(u8, u8), Error> {
    let field = exif
        .get_field(Tag::ExifVersion, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData("Missing ExifVersion field".to_string()))?;
    if let Value::Undefined(data, _) = &field.value {
        if data.len() != 4 {
            return Err(Error::InvalidData(
                "Expected 4 bytes for ExifVersion field".to_string(),
            ));
        }
        Ok((atoi(&data[..2])?, atoi(&data[2..])?))
    } else {
        Err(Error::InvalidData(
            "Expected 'undefined' type data for ExifVersion".to_string(),
        ))
    }
}

fn get_makernote(exif: &Exif) -> Result<Vec<u8>, Error> {
    let field = exif
        .get_field(Tag::MakerNote, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData("Missing MakerNote field".to_string()))?;
    if let Value::Undefined(data, _) = &field.value {
        Ok(data.clone())
    } else {
        Err(Error::InvalidData(
            "Expected 'undefined' type data for MakerNote".to_string(),
        ))
    }
}

fn get_str_field(exif: &Exif, tag: Tag, field_name: &'static str) -> Result<String, Error> {
    let field = exif
        .get_field(tag, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData(format!("Missing {} field", field_name)))?;
    if let Value::Ascii(data) = &field.value {
        if data.len() != 1 {
            return Err(Error::InvalidData(format!(
                "Expected single {} value",
                field_name
            )));
        }
        String::from_utf8(data[0].clone())
            .map_err(|_| Error::InvalidData(format!("Bad UTF-8 in {} field", field_name)))
    } else {
        Err(Error::InvalidData(format!(
            "Expected u16 data for {} field",
            field_name
        )))
    }
}

fn get_u16_field(exif: &Exif, tag: Tag, field_name: &'static str) -> Result<u16, Error> {
    let field = exif
        .get_field(tag, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData(format!("Missing {} field", field_name)))?;
    if let Value::Short(data) = &field.value {
        if data.len() != 1 {
            return Err(Error::InvalidData(format!(
                "Expected single {} value",
                field_name
            )));
        }
        Ok(data[0])
    } else {
        Err(Error::InvalidData(format!(
            "Expected u16 data for {} field",
            field_name
        )))
    }
}

fn get_u32_field(exif: &Exif, tag: Tag, field_name: &'static str) -> Result<u32, Error> {
    let field = exif
        .get_field(tag, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData(format!("Missing {} field", field_name)))?;
    if let Value::Long(data) = &field.value {
        if data.len() != 1 {
            return Err(Error::InvalidData(format!(
                "Expected single {} value",
                field_name
            )));
        }
        Ok(data[0])
    } else {
        Err(Error::InvalidData(format!(
            "Expected u32 data for {} field",
            field_name
        )))
    }
}

fn get_rational_field(exif: &Exif, tag: Tag, field_name: &'static str) -> Result<Rational, Error> {
    let field = exif
        .get_field(tag, In::PRIMARY)
        .ok_or_else(|| Error::InvalidData(format!("Missing {} field", field_name)))?;
    if let Value::Rational(data) = &field.value {
        if data.len() != 1 {
            return Err(Error::InvalidData(format!(
                "Expected single {} value",
                field_name
            )));
        }
        Ok(data[0])
    } else {
        Err(Error::InvalidData(format!(
            "Expected Rational data for {} field",
            field_name
        )))
    }
}

fn get_make(exif: &Exif) -> Result<String, Error> {
    get_str_field(exif, Tag::Make, "Make")
}

fn get_model(exif: &Exif) -> Result<String, Error> {
    let make = get_str_field(exif, Tag::Make, "Make")?;
    let model = get_str_field(exif, Tag::Model, "Model")?;
    if model.starts_with(&make) {
        Ok(model)
    } else {
        let mut make_and_model = make;
        if !make_and_model.ends_with(' ') {
            make_and_model.push(' ');
        }
        make_and_model.push_str(&model);
        Ok(make_and_model)
    }
}

fn get_serial_number(exif: &Exif) -> Result<String, Error> {
    get_str_field(exif, Tag::BodySerialNumber, "BodySerialNumber")
}

fn get_sensitivity(exif: &Exif) -> Result<(u32, u16), Error> {
    if get_exif_version(exif)? < (2, 30) {
        return Err(Error::Unsupported(
            "Exif version < 2.3 is not supported".to_string(),
        ));
    }
    let sensitivity_type = get_u16_field(exif, Tag::SensitivityType, "SensitivityType")?;
    let sensitivity = match sensitivity_type {
        SENSITIVITY_TYPE_ISO => get_u32_field(exif, Tag::ISOSpeed, "ISOSpeed")?,
        SENSITIVITY_TYPE_SOS => get_u32_field(
            exif,
            Tag::StandardOutputSensitivity,
            "StandardOutputSensitivity",
        )?,
        SENSITIVITY_TYPE_REI => get_u32_field(
            exif,
            Tag::RecommendedExposureIndex,
            "RecommendedExposureIndex",
        )?,
        SENSITIVITY_TYPE_SOS_AND_ISO => get_u32_field(exif, Tag::ISOSpeed, "ISOSpeed")?,
        SENSITIVITY_TYPE_SOS_AND_REI => get_u32_field(
            exif,
            Tag::StandardOutputSensitivity,
            "StandardOutputSensitivity",
        )?,
        SENSITIVITY_TYPE_REI_AND_ISO => get_u32_field(exif, Tag::ISOSpeed, "ISOSpeed")?,
        SENSITIVITY_TYPE_SOS_AND_REI_AND_ISO => get_u32_field(exif, Tag::ISOSpeed, "ISOSpeed")?,
        _ => return Err(Error::Unsupported("Unknown SensitivityType".to_string())),
    };
    Ok((sensitivity, sensitivity_type))
}

fn get_exposure_time(exif: &Exif) -> Result<f32, Error> {
    get_rational_field(exif, Tag::ExposureTime, "ExposureTime").map(|x| x.to_f64() as f32)
}

fn get_temperature(exif: &Exif) -> Result<f32, Error> {
    if !get_make(exif)?.eq("Canon") {
        return Err(Error::Unsupported(
            "Only Canon cameras are supported".to_string(),
        ));
    }

    let canon_makernote = parse_canon_makernote(&get_makernote(exif)?)?;
    for entry in canon_makernote {
        if entry.tag == TAG_CANON_SHOTINFO {
            if let Value::Short(data) = entry.value {
                return data
                    .get(SHOTINFO_CAMERA_TEMPERATURE)
                    .ok_or_else(|| {
                        Error::InvalidData("Missing Camera Temperature field".to_string())
                    })
                    .map(|x| (*x - 128) as f32);
            } else {
                return Err(Error::InvalidData(
                    "ShotInfo field is not a short array".to_string(),
                ));
            }
        }
    }

    Err(Error::InvalidData(
        "Canon ShotInfo maker note not found".to_string(),
    ))
}

pub(in crate) struct MetadataParser {}

impl MetadataParser {
    pub fn new() -> MetadataParser {
        MetadataParser {}
    }

    pub fn read_file<P: AsRef<Path>>(&self, path: P) -> Result<ImageMetadata, Error> {
        let file = std::fs::File::open(path)?;
        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader)?;

        let (sensor_sensitivity, sensitivity_type) = get_sensitivity(&exif)?;
        Ok(ImageMetadata {
            camera_model: get_model(&exif)?,
            camera_serial_number: get_serial_number(&exif)?,
            sensor_sensitivity,
            sensitivity_type,
            exposure_time: get_exposure_time(&exif)?,
            temperature: get_temperature(&exif)?,
        })
    }
}
