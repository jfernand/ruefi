//! The seam between the generic driver logic and a specific environment.

/// A block of memory suitable for controller DMA (CORB, RIRB, buffer
/// descriptor lists, and sample data): physically contiguous, and mapped so
/// the CPU can read/write it through `ptr` while the controller reads/writes
/// it through `phys_addr`.
pub struct DmaBuffer {
    pub ptr: *mut u8,
    pub phys_addr: u64,
    pub len: usize,
}

impl DmaBuffer {
    /// # Safety
    /// `ptr` must be valid for reads and writes of `len` bytes for as long as
    /// this buffer is used, and `phys_addr` must be the address the HDA
    /// controller's DMA engine would use to reach the same memory.
    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// Everything the [`crate::Controller`] needs from its environment: access to
/// the controller's memory-mapped registers (already resolved to some base
/// address by the caller -- this trait only ever sees offsets from that
/// base), a way to allocate DMA memory, and a way to wait.
pub trait Platform {
    fn mmio_read32(&self, offset: u32) -> u32;
    fn mmio_write32(&mut self, offset: u32, value: u32);

    fn mmio_read16(&self, offset: u32) -> u16;
    fn mmio_write16(&mut self, offset: u32, value: u16);

    fn mmio_read8(&self, offset: u32) -> u8;
    fn mmio_write8(&mut self, offset: u32, value: u8);

    /// Busy-waits for at least `micros` microseconds.
    fn delay_us(&self, micros: u32);

    /// Allocates `len` zeroed bytes of DMA-capable memory, aligned to
    /// `align` bytes. Used for the CORB, RIRB, buffer descriptor lists, and
    /// PCM sample buffers, all of which the controller reaches via a
    /// physical address programmed into a register.
    fn alloc_dma(&mut self, len: usize, align: usize) -> DmaBuffer;
}
