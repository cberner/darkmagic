use std::io;

#[derive(Debug)]
pub(in crate) enum Error {
    InvalidData(String),
    Unsupported(String),
    Io(io::Error),
    Exif(exif::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<exif::Error> for Error {
    fn from(err: exif::Error) -> Error {
        Error::Exif(err)
    }
}
