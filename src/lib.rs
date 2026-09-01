#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr},
};

pub const ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

type ImageAltLen = u16;
pub const MAX_IMAGE_ALT_TEXT: usize = 1024;
const _: () = {
    assert!(MAX_IMAGE_ALT_TEXT.ilog2() <= ImageAltLen::BITS);
};

type ImageByteCount = u16;
pub const MAX_IMAGE_BYTES: usize = 2048;
const _: () = {
    assert!(MAX_IMAGE_BYTES.ilog2() <= ImageByteCount::BITS);
};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Image {
    /// At most [`MAX_IMAGE_ALT_TEXT`] bytes
    pub alt_text: String,
    /// At most [`MAX_IMAGE_BYTES`] bytes
    pub data: Vec<u8>,
}

type TextByteCount = u16;
pub const MAX_TEXT_BYTES: usize = 2048;
const _: () = {
    assert!(MAX_TEXT_BYTES.ilog2() <= TextByteCount::BITS);
};

type ImageCount = u8;
pub const MAX_IMAGES: usize = 8;
const _: () = {
    assert!(MAX_IMAGES.ilog2() <= ImageCount::BITS);
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Message {
    /// At most [`MAX_TEXT_BYTES`] bytes
    pub text: String,
    /// At most [`MAX_IMAGES`] images
    pub images: Vec<Image>,
}

const START_OF_TEXT: u8 = 1;

impl Message {
    pub fn write_to<W: Write>(&self, stream: &mut W) -> io::Result<()> {
        stream.write_all(const { &[START_OF_TEXT] })?;
        assert!(self.text.len() <= MAX_TEXT_BYTES, "invalid text length");
        stream.write_all(
            &TextByteCount::try_from(self.text.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        assert!(self.images.len() <= MAX_IMAGES, "invalid image count");
        stream.write_all(
            &ImageCount::try_from(self.images.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        stream.write_all(self.text.as_bytes())?;
        for image in &self.images {
            assert!(
                image.alt_text.len() <= MAX_IMAGE_ALT_TEXT,
                "invalid image alt text length"
            );
            stream.write_all(
                &ImageAltLen::try_from(image.alt_text.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;
            assert!(
                image.data.len() <= MAX_IMAGE_BYTES,
                "invalid image data size"
            );
            stream.write_all(
                &ImageByteCount::try_from(image.data.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;

            stream.write_all(image.alt_text.as_bytes())?;
            stream.write_all(&image.data)?;
        }
        Ok(())
    }

    pub fn read_from<R: Read>(stream: &mut R) -> io::Result<Option<Self>> {
        let mut buf = [0];
        match stream.read(&mut buf)? {
            0 => return Ok(None),
            1 => assert_eq!(buf, [START_OF_TEXT], "expected Start Of Text"),
            _ => unreachable!(),
        }

        let mut text_len = [0; _];
        stream.read_exact(&mut text_len)?;
        let text_len = TextByteCount::from_le_bytes(text_len);

        let mut image_count = [0; _];
        stream.read_exact(&mut image_count)?;
        let image_count = ImageCount::from_le_bytes(image_count);

        Ok(Some(Self {
            text: {
                let mut text = vec![0; text_len as usize];
                stream.read_exact(&mut text)?;
                String::from_utf8(text).map_err(io::Error::other)?
            },
            images: std::iter::repeat_with(|| {
                let mut alt_len = [0; _];
                stream.read_exact(&mut alt_len)?;
                let alt_len = ImageAltLen::from_le_bytes(alt_len);

                let mut image_size = [0; _];
                stream.read_exact(&mut image_size)?;
                let image_size = ImageByteCount::from_le_bytes(image_size);

                let mut alt_text = vec![0; alt_len as usize];
                stream.read_exact(&mut alt_text)?;
                let alt_text = String::from_utf8(alt_text).map_err(io::Error::other)?;

                let mut data = vec![0; image_size as usize];
                stream.read_exact(&mut data)?;

                Ok(Image { alt_text, data })
            })
            .take(image_count as usize)
            .collect::<io::Result<Vec<_>>>()?,
        }))
    }
}
