//! # Flash memory
//!
//! Abstractions of the internal flash module.

use crate::pac::{flash, FLASH};
use cortex_m::asm;

/*
SCHLACHTPLAN

- implement flash **page** erase
- ??? check that it worked? (how?)
    * maybe there's a flash read at offset x that we can use?
- implement programming
- ??? check that it worked? (how?)

EXTRA BONUS POINTS

- adjust linker script so that there's a dedicated DATA section
- add boundary checks when erasing/writing
- merge erase & write (plus maybe a read that buffers all data in page parts that aren't to be
  overwritten?) into one convenient function
*/

const FLASH_KEYR_KEY_1: u32 = 0x45670123;
const FLASH_KEYR_KEY_2: u32 = 0xCDEF89AB;

const CCM_RAM_START: u32 = 0x10000000;
const PAGE_SZE: u32 = 0x800; // 2 KiB (2048 byte)

// TODO impl std::Error for this?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
/// Flash operation errors
pub enum FlashError {
    /// Flash is already being accessed
    Busy,
    /// Could not erase the desired Page
    EraseFailed,
    /// Could not unlock Flash for Erasing/Writing
    UnlockFailed,
}

/// Extension trait to constrain the FLASH peripheral
pub trait FlashExt {
    /// Constrains the FLASH peripheral to play nicely with the other abstractions
    fn constrain(self) -> Parts;

    /// Erase Flash Page at `address`.
    /// Note that one page = 2KByte
    ///
    /// ⚠️⚠️⚠️ CAUTION: ⚠️⚠️⚠️
    /// This function does *not* perform any bounds checks.
    /// If you erase program code, that is on you.
    fn page_erase(self, address: u32) -> Result<(), FlashError>;

    /// Write to Flash Page.
    /// Note that one page = 2KByte
    ///
    /// ⚠️⚠️⚠️ CAUTION: ⚠️⚠️⚠️
    /// This function does *not* perform any bounds checks.
    /// If you overwrite program code, that is on you.
    fn page_write(self, address: u32, data: u32) -> Result<(), FlashError>;
}

impl FlashExt for FLASH {
    fn constrain(self) -> Parts {
        Parts {
            acr: ACR { _0: () },
        }
    }

    fn page_erase(self, address: u32) -> Result<(), FlashError> {
        // 1. Check that no main Flash memory operation is ongoing by checking the BSY bit in
        //    the FLASH_SR register.
        if self.sr.read().bsy().bit_is_set() {
            // TODO alternatively wait until we can erase
            // We are busy! Come back later
            return Err(FlashError::Busy);
        }

        // TODO is the order correct here?
        if self.cr.read().lock().bit_is_set() {
            defmt::info!("CR_LOCK was set, unlocking...");
            unlock_cr(&self);

            if self.cr.read().lock().bit_is_set() {
                return Err(FlashError::UnlockFailed);
            }
        }

        // 2. Set the PER bit in the FLASH_CR register
        self.cr.modify(|_r, w| w.per().set_bit());

        // 3. Program the FLASH_AR register to select a page to erase
        // (this register is write-only, hence the use of `write()`)
        self.ar.write(|w| unsafe { w.bits(address) });

        // 4. Set the STRT bit in the FLASH_CR register (see below note)
        // TODO: this is where we get

        // Error: Error communicating with probe: An error with the usage of the probe occured
        // Caused by:
        // 0: An error with the usage of the probe occured
        // 1: An error specific to a probe type occured
        // 2: Command failed with status SwdDpWait
        self.cr.modify(|_r, w| w.strt().set_bit());

        // 5. Wait for the BSY bit to be reset
        while self.sr.read().bsy().bit_is_set() {
            // do nothing while the BSY bit is not reset yet
            asm::nop();
        }
        defmt::info!("BSY bit status: {}", self.sr.read().bsy().bit());

        defmt::info!("sr.WRPRTERR status: {}", self.sr.read().wrprterr().bit());

        // stolen form libopencm flash impl: reset PER bit
        //self.cr.modify(|_r, w| w.per().clear_bit());

        // 6. Check the EOP flag in the FLASH_SR register (it is set when the erase operation has succeeded),
        //    and then clear it by software.
        if self.sr.read().eop().bit_is_set() {
            // erase was successful
            // 7. Clear the EOP flag.
            self.sr.modify(|_r, w| w.eop().clear_bit())
        } else {
            // this should be set by now!
            return Err(FlashError::EraseFailed);
        }
        for _ in 0..10 {
            cortex_m::asm::nop();
        }
        // The software should start checking if the BSY bit equals ‘0’ at least one CPU cycle after setting the STRT bit.
        defmt::info!(
            "BSY bit status after address write: {}",
            self.sr.read().bsy().bit()
        );
        while self.sr.read().bsy().bit_is_set() {
            
        }

        Ok(())
        // // WE ARE ASSUMING that the above takes > cycle so we're not waiting explicitly (danger danger)
        // if self.sr.read().bsy().bit_is_set() {
        //     Ok(())
        // } else {
        //     Err(FlashError::Busy)
        // }
    }

    // TODO finish implementation
    fn page_write(self, address: u32, data: u32) -> Result<(), FlashError> {
        // TODO: do we have to unlock write protection (see "Unlocking the Flash memory")?

        // 1. Check that no main Flash memory operation is ongoing by checking the BSY bit in
        //    the FLASH_SR register.
        if self.sr.read().bsy().bit_is_set() {
            // We are busy! Come back later
            // TODO proper error tyoe
            return Err(FlashError::Busy);
        }

        // TODO is the order correct here?
        unlock_cr(&self);

        // 2. Set the PG bit in the FLASH_CR register.
        self.cr.write(|w| w.pg().bit(true));

        // 3. Perform the data write (half-word) at the desired address.
        self.ar.write(|w| unsafe { w.bits(address) });

        // dummy code
        unsafe {
            // for hword in data {
                core::ptr::write_volatile(address as *mut u32, data as u32);
            // }
        }

        // 4. Wait until the BSY bit is reset in the FLASH_SR register.
        // 5. Check the EOP flag in the FLASH_SR register (it is set when the programming operation
        //    has succeeded), and then clear it by software.

        // Copied from page erase, might need fixing
        // 5. Wait for the BSY bit to be reset
        while self.sr.read().bsy().bit_is_set() {
            // do nothing while the BSY bit is not reset yet
            asm::nop();
        }
        defmt::info!("BSY bit status: {}", self.sr.read().bsy().bit());

        defmt::info!("sr.WRPRTERR status: {}", self.sr.read().wrprterr().bit());

        // stolen form libopencm flash impl: reset PER bit
        //self.cr.modify(|_r, w| w.per().clear_bit());

        // 6. Check the EOP flag in the FLASH_SR register (it is set when the erase operation has succeeded),
        //    and then clear it by software.
        if self.sr.read().eop().bit_is_set() {
            // erase was successful
            // 7. Clear the EOP flag.
            self.sr.modify(|_r, w| w.eop().clear_bit())
        } else {
            // this should be set by now!
            return Err(FlashError::EraseFailed);
        }
        for _ in 0..10 {
            cortex_m::asm::nop();
        }
        // The software should start checking if the BSY bit equals ‘0’ at least one CPU cycle after setting the STRT bit.
        defmt::info!(
            "BSY bit status after address write: {}",
            self.sr.read().bsy().bit()
        );
        while self.sr.read().bsy().bit_is_set() {
        }

        Ok(())
    }
}

/// An unlocking sequence should be written to the FLASH_KEYR register to open the access to
/// the FLASH_CR register. This sequence consists of two write operations into FLASH_KEYR register:
/// 1. Write KEY1 = 0x45670123
/// 2. Write KEY2 = 0xCDEF89AB
/// Any wrong sequence locks up the FPEC and the FLASH_CR register until the next reset.
fn unlock_cr(flash: &FLASH) {
    flash.keyr.write(|w| w.fkeyr().bits(FLASH_KEYR_KEY_1));
    flash.keyr.write(|w| w.fkeyr().bits(FLASH_KEYR_KEY_2));
}

fn page_to_address() -> u32 {
    // how to get to other pages? multiply page size?
    CCM_RAM_START - PAGE_SZE
    }

/// Constrained FLASH peripheral
pub struct Parts {
    /// Opaque Access Control Register (ACR)
    pub acr: ACR,
}

/// Opaque Access Control Register (ACR)
pub struct ACR {
    _0: (),
}

impl ACR {
    pub(crate) fn acr(&mut self) -> &flash::ACR {
        // NOTE(unsafe) this proxy grants exclusive access to this register
        unsafe { &(*FLASH::ptr()).acr }
    }
}
