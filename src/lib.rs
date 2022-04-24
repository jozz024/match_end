#![feature(proc_macro_hygiene)]

use smashline::*;
use skyline;
use skyline::nn::ro::LookupSymbol;
use smash::app;
use smash::lib::lua_const::*;
use smash::app::lua_bind::*;
use smash::lua2cpp::L2CFighterCommon;
use std::{thread, time};

pub static mut FIGHTER_MANAGER_ADDR: usize = 0;

pub static mut amount_printed: i32 = 0;
pub static mut run_0_once_p1: i32 = 0;
pub static mut run_0_once_p2: i32 = 0;
pub static mut fighter_1_match_wins: i32 = 0;
pub static mut fighter_2_match_wins: i32 = 0;
pub static mut ENTRY_ID: usize = 0;

extern "C" {
    #[link_name="_ZN3app3nfp10is_enabledEP9lua_State"]
    pub fn nfp_is_enabled(lua_state: u64) -> u64;
}

#[skyline::hook(replace = nfp_is_enabled)]
pub fn enable_hook(lua_state: u64) -> u64 {
    let result = original!()(lua_state);
    if result == 0 {
        println!("[match_end] One of the fighters is not an amiibo, exiting.");
        let millis = time::Duration::from_millis(1000);
        let now = time::Instant::now();
        thread::sleep(millis);
        unsafe{
            skyline::nn::oe::ExitApplication();
        }
    }
    return result
}

#[fighter_frame_callback]
pub fn once_per_fighter_frame(fighter: &mut L2CFighterCommon) {
    unsafe {
        LookupSymbol(
            &mut FIGHTER_MANAGER_ADDR,
            "_ZN3lib9SingletonIN3app14FighterManagerEE9instance_E\u{0}"
                .as_bytes()
                .as_ptr(),
        );
        let fighter_manager = *(FIGHTER_MANAGER_ADDR as *mut *mut smash::app::FighterManager);
        let fighter_1_information = FighterManager::get_fighter_information(
            fighter_manager,
            app::FighterEntryID(0)
        ) as *mut app::FighterInformation;
        let fighter_2_information = FighterManager::get_fighter_information(
            fighter_manager,
            app::FighterEntryID(1)
        ) as *mut app::FighterInformation;

        if FighterInformation::stock_count(fighter_1_information) == 3 && run_0_once_p1 == 1 {
            run_0_once_p1 = 0;
            amount_printed = 0;
        }
        if FighterInformation::stock_count(fighter_2_information) == 3 && run_0_once_p2 == 1 {
            run_0_once_p2 = 0;
            amount_printed = 0;
        }

        if FighterInformation::stock_count(fighter_1_information) == 0 && run_0_once_p1 == 0 {
            fighter_2_match_wins = fighter_2_match_wins + 1;
            run_0_once_p1 = 1;
        }

        if FighterInformation::stock_count(fighter_2_information) == 0 && run_0_once_p2 == 0 {
            fighter_1_match_wins = fighter_1_match_wins + 1;
            run_0_once_p2 = 1;
        }

        if FighterManager::is_result_mode(fighter_manager) && FighterManager::entry_count(fighter_manager) > 0 {
            if amount_printed == 0 {
                if fighter_1_match_wins > fighter_2_match_wins {
                    println!("[match_end] Player 1 won. {}-{}", fighter_1_match_wins, fighter_2_match_wins);
                }
                if fighter_2_match_wins > fighter_1_match_wins {
                    println!("[match_end] Player 2 won. {}-{}", fighter_1_match_wins, fighter_2_match_wins);
                }
                amount_printed = amount_printed + 1;
                fighter_1_match_wins = 0;
                fighter_2_match_wins = 0;
            }
        }
    }
}

#[skyline::main(name = "match_end")]
pub fn main() {
    skyline::install_hooks!(enable_hook);
    install_agent_frame_callbacks!(once_per_fighter_frame);
}
