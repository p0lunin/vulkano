// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

//! Image storage (1D, 2D, 3D, arrays, etc.) and image views.
//!
//! An *image* is a region of memory whose purpose is to store multi-dimensional data. Its
//! most common use is to store a 2D array of color pixels (in other words an *image* in
//! everyday language), but it can also be used to store arbitrary data.
//!
//! The advantage of using an image compared to a buffer is that the memory layout is optimized
//! for locality. When reading a specific pixel of an image, reading the nearby pixels is really
//! fast. Most implementations have hardware dedicated to reading from images if you access them
//! through a sampler.
//!
//! # Properties of an image
//!
//! # Images and image views
//!
//! There is a distinction between *images* and *image views*. As its name suggests, an image
//! view describes how the GPU must interpret the image.
//!
//! Transfer and memory operations operate on images themselves, while reading/writing an image
//! operates on image views. You can create multiple image views from the same image.
//!
//! # High-level wrappers
//!
//! In the vulkano library, an image is any object that implements the [`ImageAccess`] trait. You
//! can create a view by wrapping them in an [`ImageView`].
//!
//! Since the `ImageAccess` trait is low-level, you are encouraged to not implement it yourself but
//! instead use one of the provided implementations that are specialized depending on the way you
//! are going to use the image:
//!
//! - An `AttachmentImage` can be used when you want to draw to an image.
//! - An `ImmutableImage` stores data which never need be changed after the initial upload,
//!   like a texture.
//!
//! # Low-level information
//!
//! To be written.
//!

use std::cmp;
use std::convert::TryFrom;

pub use self::aspect::ImageAspect;
pub use self::attachment::AttachmentImage;
pub use self::immutable::ImmutableImage;
pub use self::layout::ImageDescriptorLayouts;
pub use self::layout::ImageLayout;
pub use self::storage::StorageImage;
pub use self::swapchain::SwapchainImage;
pub use self::sys::ImageCreationError;
pub use self::traits::ImageAccess;
pub use self::traits::ImageInner;
pub use self::usage::ImageUsage;
pub use self::view::ImageViewAbstract;

mod aspect;
pub mod attachment; // TODO: make private
pub mod immutable; // TODO: make private
mod layout;
mod storage;
pub mod swapchain; // TODO: make private
pub mod sys;
pub mod traits;
mod usage;
pub mod view;

/// Specifies how many mipmaps must be allocated.
///
/// Note that at least one mipmap must be allocated, to store the main level of the image.
#[derive(Debug, Copy, Clone)]
pub enum MipmapsCount {
    /// Allocates the number of mipmaps required to store all the mipmaps of the image where each
    /// mipmap is half the dimensions of the previous level. Guaranteed to be always supported.
    ///
    /// Note that this is not necessarily the maximum number of mipmaps, as the Vulkan
    /// implementation may report that it supports a greater value.
    Log2,

    /// Allocate one mipmap (ie. just the main level). Always supported.
    One,

    /// Allocate the given number of mipmaps. May result in an error if the value is out of range
    /// of what the implementation supports.
    Specific(u32),
}

impl From<u32> for MipmapsCount {
    #[inline]
    fn from(num: u32) -> MipmapsCount {
        MipmapsCount::Specific(num)
    }
}

/// Helper type for creating extents
pub enum Extent {
    E1D([u32; 1]),
    E2D([u32; 2]),
    E3D([u32; 3]),
}

impl From<vk::Extent2D> for Extent {
    fn from(extent: vk::Extent2D) -> Self {
        Extent::E2D([extent.width, extent.height])
    }
}

impl From<vk::Extent3D> for Extent {
    fn from(extent: vk::Extent3D) -> Self {
        Extent::E3D([extent.width, extent.height, extent.depth])
    }
}
impl TryFrom<Extent> for vk::Extent2D {
    type Error = ();

    fn try_from(extent: Extent) -> Result<Self, Self::Error> {
        match extent {
            Extent::E2D(a) => Ok(vk::Extent2D {
                width: a[0],
                height: a[1],
            }),
            _ => Err(()),
        }
    }
}

impl TryFrom<Extent> for vk::Extent3D {
    type Error = ();

    fn try_from(extent: Extent) -> Result<Self, Self::Error> {
        match extent {
            Extent::E3D(a) => Ok(vk::Extent3D {
                width: a[0],
                height: a[1],
                depth: a[2],
            }),
            _ => Err(()),
        }
    }
}

/// Helper type returned from Device's `fn image_format_properties()`
pub struct ImageFormatProperties {
    pub max_extent: Extent,
    pub max_mip_levels: MipmapsCount,
    pub max_array_layers: u32,
    pub sample_counts: u32,
    pub max_resource_size: usize,
}

impl From<vk::ImageFormatProperties> for ImageFormatProperties {
    fn from(props: vk::ImageFormatProperties) -> Self {
        Self {
            max_extent: props.maxExtent.into(),
            max_mip_levels: props.maxMipLevels.into(),
            max_array_layers: props.maxArrayLayers,
            sample_counts: props.sampleCounts,
            max_resource_size: props.maxResourceSize as usize,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct ImageCreateFlags {
    pub sparse_binding: bool,
    pub sparse_residency: bool,
    pub sparse_aliased: bool,
    pub mutable_format: bool,
    pub cube_compatible: bool,
    pub array_2d_compatible: bool,
}

impl ImageCreateFlags {
    pub fn all() -> Self {
        Self {
            sparse_binding: true,
            sparse_residency: true,
            sparse_aliased: true,
            mutable_format: true,
            cube_compatible: true,
            array_2d_compatible: true,
        }
    }

    pub fn none() -> Self {
        Self::default()
    }
}

impl From<ImageCreateFlags> for vk::ImageCreateFlags {
    fn from(flags: ImageCreateFlags) -> Self {
        let mut vk_flags = Self::default();
        if flags.sparse_binding {
            vk_flags |= vk::IMAGE_CREATE_SPARSE_BINDING_BIT
        };
        if flags.sparse_residency {
            vk_flags |= vk::IMAGE_CREATE_SPARSE_RESIDENCY_BIT
        };
        if flags.sparse_aliased {
            vk_flags |= vk::IMAGE_CREATE_SPARSE_ALIASED_BIT
        };
        if flags.mutable_format {
            vk_flags |= vk::IMAGE_CREATE_MUTABLE_FORMAT_BIT
        };
        if flags.cube_compatible {
            vk_flags |= vk::IMAGE_CREATE_CUBE_COMPATIBLE_BIT
        };
        if flags.array_2d_compatible {
            vk_flags |= vk::IMAGE_CREATE_2D_ARRAY_COMPATIBLE_BIT_KHR
        };
        vk_flags
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageType {
    Dim1d,
    Dim2d,
    Dim3d,
}
impl From<ImageType> for vk::ImageType {
    fn from(image_type: ImageType) -> Self {
        match image_type {
            ImageType::Dim1d => vk::IMAGE_TYPE_1D,
            ImageType::Dim2d => vk::IMAGE_TYPE_2D,
            ImageType::Dim3d => vk::IMAGE_TYPE_3D,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageTiling {
    Optimal,
    Linear,
}

impl From<ImageTiling> for vk::ImageTiling {
    fn from(image_tiling: ImageTiling) -> Self {
        match image_tiling {
            ImageTiling::Optimal => vk::IMAGE_TILING_OPTIMAL,
            ImageTiling::Linear => vk::IMAGE_TILING_LINEAR,
        }
    }
}

/// The dimensions of an image.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ImageDimensions {
    Dim1d {
        width: u32,
        array_layers: u32,
    },
    Dim2d {
        width: u32,
        height: u32,
        array_layers: u32,
    },
    Dim3d {
        width: u32,
        height: u32,
        depth: u32,
    },
}

impl ImageDimensions {
    #[inline]
    pub fn width(&self) -> u32 {
        match *self {
            ImageDimensions::Dim1d { width, .. } => width,
            ImageDimensions::Dim2d { width, .. } => width,
            ImageDimensions::Dim3d { width, .. } => width,
        }
    }

    #[inline]
    pub fn height(&self) -> u32 {
        match *self {
            ImageDimensions::Dim1d { .. } => 1,
            ImageDimensions::Dim2d { height, .. } => height,
            ImageDimensions::Dim3d { height, .. } => height,
        }
    }

    #[inline]
    pub fn width_height(&self) -> [u32; 2] {
        [self.width(), self.height()]
    }

    #[inline]
    pub fn depth(&self) -> u32 {
        match *self {
            ImageDimensions::Dim1d { .. } => 1,
            ImageDimensions::Dim2d { .. } => 1,
            ImageDimensions::Dim3d { depth, .. } => depth,
        }
    }

    #[inline]
    pub fn width_height_depth(&self) -> [u32; 3] {
        [self.width(), self.height(), self.depth()]
    }

    #[inline]
    pub fn array_layers(&self) -> u32 {
        match *self {
            ImageDimensions::Dim1d { array_layers, .. } => array_layers,
            ImageDimensions::Dim2d { array_layers, .. } => array_layers,
            ImageDimensions::Dim3d { .. } => 1,
        }
    }

    /// Returns the total number of texels for an image of these dimensions.
    #[inline]
    pub fn num_texels(&self) -> u32 {
        self.width() * self.height() * self.depth() * self.array_layers()
    }

    /// Returns the maximum number of mipmaps for these image dimensions.
    ///
    /// The returned value is always at least superior or equal to 1.
    ///
    /// # Example
    ///
    /// ```
    /// use vulkano::image::ImageDimensions;
    ///
    /// let dims = ImageDimensions::Dim2d {
    ///     width: 32,
    ///     height: 50,
    ///     array_layers: 1,
    /// };
    ///
    /// assert_eq!(dims.max_mipmaps(), 6);
    /// ```
    ///
    pub fn max_mipmaps(&self) -> u32 {
        32 - (self.width() | self.height() | self.depth()).leading_zeros()
    }

    /// Returns the dimensions of the `level`th mipmap level. If `level` is 0, then the dimensions
    /// are left unchanged.
    ///
    /// Returns `None` if `level` is superior or equal to `max_mipmaps()`.
    ///
    /// # Example
    ///
    /// ```
    /// use vulkano::image::ImageDimensions;
    ///
    /// let dims = ImageDimensions::Dim2d {
    ///     width: 963,
    ///     height: 256,
    ///     array_layers: 1,
    /// };
    ///
    /// assert_eq!(dims.mipmap_dimensions(0), Some(dims));
    /// assert_eq!(dims.mipmap_dimensions(1), Some(ImageDimensions::Dim2d {
    ///     width: 481,
    ///     height: 128,
    ///     array_layers: 1,
    /// }));
    /// assert_eq!(dims.mipmap_dimensions(6), Some(ImageDimensions::Dim2d {
    ///     width: 15,
    ///     height: 4,
    ///     array_layers: 1,
    /// }));
    /// assert_eq!(dims.mipmap_dimensions(9), Some(ImageDimensions::Dim2d {
    ///     width: 1,
    ///     height: 1,
    ///     array_layers: 1,
    /// }));
    /// assert_eq!(dims.mipmap_dimensions(11), None);
    /// ```
    ///
    /// # Panic
    ///
    /// In debug mode, Panics if `width`, `height` or `depth` is equal to 0. In release, returns
    /// an unspecified value.
    ///
    pub fn mipmap_dimensions(&self, level: u32) -> Option<ImageDimensions> {
        if level == 0 {
            return Some(*self);
        }

        if level >= self.max_mipmaps() {
            return None;
        }

        Some(match *self {
            ImageDimensions::Dim1d {
                width,
                array_layers,
            } => {
                debug_assert_ne!(width, 0);
                ImageDimensions::Dim1d {
                    array_layers,
                    width: cmp::max(1, width >> level),
                }
            }

            ImageDimensions::Dim2d {
                width,
                height,
                array_layers,
            } => {
                debug_assert_ne!(width, 0);
                debug_assert_ne!(height, 0);
                ImageDimensions::Dim2d {
                    width: cmp::max(1, width >> level),
                    height: cmp::max(1, height >> level),
                    array_layers,
                }
            }

            ImageDimensions::Dim3d {
                width,
                height,
                depth,
            } => {
                debug_assert_ne!(width, 0);
                debug_assert_ne!(height, 0);
                ImageDimensions::Dim3d {
                    width: cmp::max(1, width >> level),
                    height: cmp::max(1, height >> level),
                    depth: cmp::max(1, depth >> level),
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::format;
    use crate::image::ImageDimensions;
    use crate::image::ImmutableImage;
    use crate::image::MipmapsCount;

    #[test]
    fn max_mipmaps() {
        let dims = ImageDimensions::Dim2d {
            width: 2,
            height: 1,
            array_layers: 1,
        };
        assert_eq!(dims.max_mipmaps(), 2);

        let dims = ImageDimensions::Dim2d {
            width: 2,
            height: 3,
            array_layers: 1,
        };
        assert_eq!(dims.max_mipmaps(), 2);

        let dims = ImageDimensions::Dim2d {
            width: 512,
            height: 512,
            array_layers: 1,
        };
        assert_eq!(dims.max_mipmaps(), 10);
    }

    #[test]
    fn mipmap_dimensions() {
        let dims = ImageDimensions::Dim2d {
            width: 283,
            height: 175,
            array_layers: 1,
        };
        assert_eq!(dims.mipmap_dimensions(0), Some(dims));
        assert_eq!(
            dims.mipmap_dimensions(1),
            Some(ImageDimensions::Dim2d {
                width: 141,
                height: 87,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(2),
            Some(ImageDimensions::Dim2d {
                width: 70,
                height: 43,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(3),
            Some(ImageDimensions::Dim2d {
                width: 35,
                height: 21,
                array_layers: 1,
            })
        );

        assert_eq!(
            dims.mipmap_dimensions(4),
            Some(ImageDimensions::Dim2d {
                width: 17,
                height: 10,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(5),
            Some(ImageDimensions::Dim2d {
                width: 8,
                height: 5,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(6),
            Some(ImageDimensions::Dim2d {
                width: 4,
                height: 2,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(7),
            Some(ImageDimensions::Dim2d {
                width: 2,
                height: 1,
                array_layers: 1,
            })
        );
        assert_eq!(
            dims.mipmap_dimensions(8),
            Some(ImageDimensions::Dim2d {
                width: 1,
                height: 1,
                array_layers: 1,
            })
        );
        assert_eq!(dims.mipmap_dimensions(9), None);
    }

    #[test]
    fn mipmap_working_immutable_image() {
        let (device, queue) = gfx_dev_and_queue!();

        let dimensions = ImageDimensions::Dim2d {
            width: 512,
            height: 512,
            array_layers: 1,
        };
        {
            let mut vec = Vec::new();

            vec.resize(512 * 512, 0u8);

            let (image, _) = ImmutableImage::from_iter(
                vec.into_iter(),
                dimensions,
                MipmapsCount::One,
                format::R8Unorm,
                queue.clone(),
            )
            .unwrap();
            assert_eq!(image.mipmap_levels(), 1);
        }
        {
            let mut vec = Vec::new();

            vec.resize(512 * 512, 0u8);

            let (image, _) = ImmutableImage::from_iter(
                vec.into_iter(),
                dimensions,
                MipmapsCount::Log2,
                format::R8Unorm,
                queue.clone(),
            )
            .unwrap();
            assert_eq!(image.mipmap_levels(), 10);
        }
    }
}
