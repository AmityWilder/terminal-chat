#![warn(clippy::undocumented_unsafe_blocks)]

use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr},
};

pub const ADDRESS: SocketAddr =
    SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);

type ImageByteCount = u16;
pub const MAX_IMAGE_BYTES: usize = 2048;
const _: () = {
    assert!(MAX_IMAGE_BYTES.ilog2() <= ImageByteCount::BITS);
};

/// TODO
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Image {
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

impl Message {
    pub fn write_to<W: Write>(&self, stream: &mut W) -> io::Result<()> {
        // header
        stream.write_all(
            &TextByteCount::try_from(self.text.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        stream.write_all(
            &ImageCount::try_from(self.images.len())
                .unwrap()
                .to_le_bytes(),
        )?;
        for image in &self.images {
            stream.write_all(
                &ImageByteCount::try_from(image.data.len())
                    .unwrap()
                    .to_le_bytes(),
            )?;
        }
        // content
        stream.write_all(self.text.as_bytes())?;
        for image in &self.images {
            stream.write_all(&image.data)?;
        }
        Ok(())
    }

    pub fn read_from<R: Read>(stream: &mut R) -> io::Result<Option<Self>> {
        // header
        let mut text_len = [0; _];
        let n = stream.read(&mut text_len)?;
        if n == 0 {
            return Ok(None);
        } else if n != text_len.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "failed to fill whole buffer",
            ));
        }
        let text_len = TextByteCount::from_le_bytes(text_len);
        let mut image_count = [0; _];
        stream.read_exact(&mut image_count)?;
        let image_count = ImageCount::from_le_bytes(image_count);
        let mut image_sizes = [[0; _]; 8];
        for image_size in &mut image_sizes[..image_count as usize] {
            stream.read_exact(image_size)?;
        }
        let image_sizes: [_; 8] =
            std::array::from_fn(|i| ImageByteCount::from_le_bytes(image_sizes[i]));
        // content
        Ok(Some(Self {
            text: {
                let mut text = vec![0; text_len as usize];
                stream.read_exact(&mut text)?;
                String::from_utf8(text).map_err(io::Error::other)?
            },
            images: image_sizes
                .into_iter()
                .take(image_count as usize)
                .map(|image_size| {
                    let mut data = vec![0; image_size as usize];
                    stream.read_exact(&mut data)?;
                    Ok(Image { data })
                })
                .collect::<io::Result<Vec<_>>>()?,
        }))
    }
}
