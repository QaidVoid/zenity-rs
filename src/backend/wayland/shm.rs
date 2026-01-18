//! Shared memory buffer management for Wayland.

use std::{
    io::{Seek, SeekFrom, Write},
    os::fd::{AsFd, OwnedFd},
};

use wayland_client::{
    QueueHandle,
    protocol::{wl_buffer::WlBuffer, wl_shm::WlShm, wl_shm_pool::WlShmPool},
};

use crate::error::Error;

use super::WaylandState;

/// A shared memory pool for creating Wayland buffers.
pub(super) struct ShmPool {
    pool: WlShmPool,
    #[allow(dead_code)]
    fd: OwnedFd,
    data: memmap2::MmapMut,
    size: usize,
}

impl ShmPool {
    /// Creates a new shared memory pool with the given size.
    pub(super) fn new(
        shm: &WlShm,
        size: usize,
        qh: &QueueHandle<WaylandState>,
    ) -> Result<Self, Error> {
        // Create a temporary file for the shared memory
        let mut file = tempfile::tempfile()?;

        // Set the file size
        file.seek(SeekFrom::Start(size as u64 - 1))?;
        file.write_all(&[0])?;
        file.seek(SeekFrom::Start(0))?;

        // Memory map the file
        let data = unsafe { memmap2::MmapMut::map_mut(&file)? };

        let fd: OwnedFd = file.into();

        // Create the Wayland shm pool
        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());

        Ok(Self {
            pool,
            fd,
            data,
            size,
        })
    }

    /// Creates a buffer from this pool.
    pub(super) fn create_buffer(
        &self,
        width: i32,
        height: i32,
        stride: i32,
        qh: &QueueHandle<WaylandState>,
    ) -> WlBuffer {
        self.pool.create_buffer(
            0,
            width,
            height,
            stride,
            wayland_client::protocol::wl_shm::Format::Argb8888,
            qh,
            (),
        )
    }

    /// Returns a mutable slice of the pool's data.
    pub(super) fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data[..self.size]
    }
}
