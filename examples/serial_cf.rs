//! Example of transmitting data over serial interface using DMA.
//! For this to work, the PA9 and PA10 pins must be connected.
//! Target board: STM32F3DISCOVERY

#![no_std]
#![no_main]

use panic_semihosting as _;

use cortex_m::{asm, singleton};
use cortex_m_rt::entry;
use stm32f3xx_hal::{pac, prelude::*, serial::Serial};

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    let mut flash = dp.FLASH.constrain();

    // This is a workaround, so that the debugger will not disconnect
    // imidiatly on asm::wfi();
    // https://github.com/probe-rs/probe-rs/issues/350#issuecomment-740550519

    dp.DBGMCU.cr.modify(|_, w| {
        w.dbg_sleep().set_bit();
        w.dbg_standby().set_bit();
        w.dbg_stop().set_bit()
    });
    dp.RCC.ahbenr.modify(|_, w| w.dma1en().enabled());

    let mut rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);

    let rts = gpioa.pa12.into_af7_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh);

    let pins = (
        gpioa
            .pa9
            .into_af7_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh),
        gpioa
            .pa10
            .into_af7_push_pull(&mut gpioa.moder, &mut gpioa.otyper, &mut gpioa.afrh),
    );
    let mut serial = Serial::new_with_flow_control(dp.USART1, pins, 9600.Bd(), clocks, &mut rcc.apb2);




    
    let (mut tx, mut rx) = serial.split();


    // the data we are going to send over serial
    // let tx_buf = singleton!(: [u8; 9] = *b"hello DMA").unwrap();
    // // the buffer we are going to receive the transmitted data in
    // let rx_buf = singleton!(: [u8; 9] = [0; 9]).unwrap();
    let msg: u8 = 100;
    loop {
       
    // start separate DMAs for sending and receiving the data
    let sending = tx.write(msg).unwrap();
    sending.wait();
    let receiving = rx.read().unwrap();

    }

    // block until all data was transmitted and received
    // let (tx_buf, tx, tx) = sending.wait();
    // let (rx_buf, rx, rx) = receiving.wait();

    // After a transfer is finished its parts can be re-used for another one.
    // tx_buf.copy_from_slice(b"hi again!");

    // let sending = tx.write(tx_buf);
    // let receiving = rx.read_exact(rx_buf);

    // let (tx_buf, ..) = sending.wait();
    // let (rx_buf, ..) = receiving.wait();



    loop {
        asm::wfi();
    }
}
