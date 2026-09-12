use core::slice;
use std::mem::size_of;

use super::{
    CapturerPara, FrameInfo, SharedMemory, ADDR_CAPTURER_PARA, ADDR_CAPTURE_FRAME_INFO,
};

#[inline]
pub fn i32_to_vec(i: i32) -> Vec<u8> {
    i.to_ne_bytes().to_vec()
}

#[inline]
pub fn ptr_to_i32(ptr: *const u8) -> i32 {
    unsafe {
        let v = slice::from_raw_parts(ptr, size_of::<i32>());
        i32::from_ne_bytes([v[0], v[1], v[2], v[3]])
    }
}

#[inline]
pub fn counter_ready(counter: *const u8) -> bool {
    unsafe {
        let wptr = counter;
        let rptr = counter.add(size_of::<i32>());
        let iw = ptr_to_i32(wptr);
        let ir = ptr_to_i32(rptr);
        if ir != iw {
            std::ptr::copy_nonoverlapping(wptr, rptr as *mut _, size_of::<i32>());
            true
        } else {
            false
        }
    }
}

#[inline]
pub fn counter_equal(counter: *const u8) -> bool {
    unsafe {
        let wptr = counter;
        let rptr = counter.add(size_of::<i32>());
        let iw = ptr_to_i32(wptr);
        let ir = ptr_to_i32(rptr);
        iw == ir
    }
}

#[inline]
pub fn increase_counter(counter: *mut u8) {
    unsafe {
        let wptr = counter;
        let rptr = counter.add(size_of::<i32>());
        let iw = ptr_to_i32(counter);
        let ir = ptr_to_i32(counter);
        let iw_plus1 = if iw == i32::MAX { 0 } else { iw + 1 };
        let v = i32_to_vec(iw_plus1);
        std::ptr::copy_nonoverlapping(v.as_ptr(), wptr, size_of::<i32>());
        if ir == iw_plus1 {
            let v = i32_to_vec(iw);
            std::ptr::copy_nonoverlapping(v.as_ptr(), rptr, size_of::<i32>());
        }
    }
}

#[inline]
pub fn align(v: usize, align: usize) -> usize {
    (v + align - 1) / align * align
}

#[inline]
pub fn set_para(shmem: &SharedMemory, para: CapturerPara) {
    let para_ptr = &para as *const CapturerPara as *const u8;
    let para_data;
    unsafe {
        para_data = slice::from_raw_parts(para_ptr, size_of::<CapturerPara>());
    }
    shmem.write(ADDR_CAPTURER_PARA, para_data);
}

#[inline]
pub fn set_frame_info(shmem: &SharedMemory, info: FrameInfo) {
    let ptr = &info as *const FrameInfo as *const u8;
    let data;
    unsafe {
        data = slice::from_raw_parts(ptr, size_of::<FrameInfo>());
    }
    shmem.write(ADDR_CAPTURE_FRAME_INFO, data);
}
