// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use std::error;
use std::fmt;

use crate::buffer::TypedBufferAccess;
use crate::device::Device;
use crate::device::DeviceOwned;
use crate::format::AcceptsPixels;
use crate::format::Format;
use crate::format::IncompatiblePixelsType;
use crate::image::ImageAccess;
use crate::VulkanObject;

/// Type of operation to check.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum CheckCopyBufferImageTy {
    BufferToImage,
    ImageToBuffer,
}

/// Checks whether a copy buffer-image command is valid. Can check both buffer-to-image copies and
/// image-to-buffer copies.
///
/// # Panic
///
/// - Panics if the buffer and image were not created with `device`.
///
pub fn check_copy_buffer_image<B, I, P>(
    device: &Device,
    buffer: &B,
    image: &I,
    ty: CheckCopyBufferImageTy,
    image_offset: [u32; 3],
    image_size: [u32; 3],
    image_first_layer: u32,
    image_num_layers: u32,
    image_mipmap: u32,
) -> Result<(), CheckCopyBufferImageError>
where
    I: ?Sized + ImageAccess,
    B: ?Sized + TypedBufferAccess<Content = [P]>,
    Format: AcceptsPixels<P>, // TODO: use a trait on the image itself instead
{
    let buffer_inner = buffer.inner();
    let image_inner = image.inner();

    assert_eq!(
        buffer_inner.buffer.device().internal_object(),
        device.internal_object()
    );
    assert_eq!(
        image_inner.image.device().internal_object(),
        device.internal_object()
    );

    match ty {
        CheckCopyBufferImageTy::BufferToImage => {
            if !buffer_inner.buffer.usage_transfer_source() {
                return Err(CheckCopyBufferImageError::SourceMissingTransferUsage);
            }
            if !image_inner.image.usage().transfer_destination {
                return Err(CheckCopyBufferImageError::DestinationMissingTransferUsage);
            }
        }
        CheckCopyBufferImageTy::ImageToBuffer => {
            if !image_inner.image.usage().transfer_source {
                return Err(CheckCopyBufferImageError::SourceMissingTransferUsage);
            }
            if !buffer_inner.buffer.usage_transfer_destination() {
                return Err(CheckCopyBufferImageError::DestinationMissingTransferUsage);
            }
        }
    }

    if image.samples() != 1 {
        return Err(CheckCopyBufferImageError::UnexpectedMultisampled);
    }

    let image_dimensions = match image.dimensions().mipmap_dimensions(image_mipmap) {
        Some(d) => d,
        None => return Err(CheckCopyBufferImageError::ImageCoordinatesOutOfRange),
    };

    if image_first_layer + image_num_layers > image_dimensions.array_layers() {
        return Err(CheckCopyBufferImageError::ImageCoordinatesOutOfRange);
    }

    if image_offset[0] + image_size[0] > image_dimensions.width() {
        return Err(CheckCopyBufferImageError::ImageCoordinatesOutOfRange);
    }

    if image_offset[1] + image_size[1] > image_dimensions.height() {
        return Err(CheckCopyBufferImageError::ImageCoordinatesOutOfRange);
    }

    if image_offset[2] + image_size[2] > image_dimensions.depth() {
        return Err(CheckCopyBufferImageError::ImageCoordinatesOutOfRange);
    }

    image.format().ensure_accepts()?;

    {
        let required_len = required_len_for_format(image.format(), image_size, image_num_layers);
        if required_len > buffer.len() {
            return Err(CheckCopyBufferImageError::BufferTooSmall {
                required_len,
                actual_len: buffer.len(),
            });
        }
    }

    // TODO: check memory overlap?

    Ok(())
}

/// Computes the minimum required len in elements for buffer with image data in specified
/// format of specified size.
fn required_len_for_format<P>(format: Format, image_size: [u32; 3], image_num_layers: u32) -> usize
where
    Format: AcceptsPixels<P>,
{
    let (block_width, block_height) = format.block_dimensions();
    let num_blocks = (image_size[0] + block_width - 1) / block_width
        * ((image_size[1] + block_height - 1) / block_height)
        * image_size[2]
        * image_num_layers;
    let required_len = num_blocks as usize * format.rate() as usize;

    return required_len;
}

#[cfg(test)]
mod tests {
    use crate::command_buffer::validity::copy_image_buffer::required_len_for_format;
    use crate::format::Format;

    #[test]
    fn test_required_len_for_format() {
        // issue #1292
        assert_eq!(
            required_len_for_format::<u8>(Format::BC1_RGBUnormBlock, [2048, 2048, 1], 1),
            2097152
        );
        // other test cases
        assert_eq!(
            required_len_for_format::<u8>(Format::R8G8B8A8Unorm, [2048, 2048, 1], 1),
            16777216
        );
        assert_eq!(
            required_len_for_format::<u8>(Format::R4G4UnormPack8, [512, 512, 1], 1),
            262144
        );
        assert_eq!(
            required_len_for_format::<u8>(Format::R8G8B8Uscaled, [512, 512, 1], 1),
            786432
        );
        assert_eq!(
            required_len_for_format::<u8>(Format::R32G32Uint, [512, 512, 1], 1),
            2097152
        );
        assert_eq!(
            required_len_for_format::<u32>(Format::R32G32Uint, [512, 512, 1], 1),
            524288
        );
        assert_eq!(
            required_len_for_format::<[u32; 2]>(Format::R32G32Uint, [512, 512, 1], 1),
            262144
        );
        assert_eq!(
            required_len_for_format::<u8>(Format::ASTC_8x8UnormBlock, [512, 512, 1], 1),
            65536
        );
        assert_eq!(
            required_len_for_format::<u8>(Format::ASTC_12x12SrgbBlock, [512, 512, 1], 1),
            29584
        );
    }
}

/// Error that can happen from `check_copy_buffer_image`.
#[derive(Debug, Copy, Clone)]
pub enum CheckCopyBufferImageError {
    /// The source buffer or image is missing the transfer source usage.
    SourceMissingTransferUsage,
    /// The destination buffer or image is missing the transfer destination usage.
    DestinationMissingTransferUsage,
    /// The source and destination are overlapping.
    OverlappingRanges,
    /// The image must not be multisampled.
    UnexpectedMultisampled,
    /// The image coordinates are out of range.
    ImageCoordinatesOutOfRange,
    /// The type of pixels in the buffer isn't compatible with the image format.
    WrongPixelType(IncompatiblePixelsType),
    /// The buffer is too small for the copy operation.
    BufferTooSmall {
        /// Required number of elements in the buffer.
        required_len: usize,
        /// Actual number of elements in the buffer.
        actual_len: usize,
    },
}

impl error::Error for CheckCopyBufferImageError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            CheckCopyBufferImageError::WrongPixelType(ref err) => Some(err),
            _ => None,
        }
    }
}

impl fmt::Display for CheckCopyBufferImageError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            fmt,
            "{}",
            match *self {
                CheckCopyBufferImageError::SourceMissingTransferUsage => {
                    "the source buffer is missing the transfer source usage"
                }
                CheckCopyBufferImageError::DestinationMissingTransferUsage => {
                    "the destination buffer is missing the transfer destination usage"
                }
                CheckCopyBufferImageError::OverlappingRanges => {
                    "the source and destination are overlapping"
                }
                CheckCopyBufferImageError::UnexpectedMultisampled => {
                    "the image must not be multisampled"
                }
                CheckCopyBufferImageError::ImageCoordinatesOutOfRange => {
                    "the image coordinates are out of range"
                }
                CheckCopyBufferImageError::WrongPixelType(_) => {
                    "the type of pixels in the buffer isn't compatible with the image format"
                }
                CheckCopyBufferImageError::BufferTooSmall { .. } => {
                    "the buffer is too small for the copy operation"
                }
            }
        )
    }
}

impl From<IncompatiblePixelsType> for CheckCopyBufferImageError {
    #[inline]
    fn from(err: IncompatiblePixelsType) -> CheckCopyBufferImageError {
        CheckCopyBufferImageError::WrongPixelType(err)
    }
}
