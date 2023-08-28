#![feature(proc_macro_hygiene)]

use smashline::*;
use skyline;
use skyline::hooks::InlineCtx;
use skyline::nn::ro::LookupSymbol;
use smash::app;
use smash::lib::lua_const::*;
use smash::app::lua_bind::*;
use smash::lua2cpp::L2CFighterCommon;
use std::{thread, time};
use serde::Serialize;
use std::{collections::HashMap, fs::*, io::{self, Write}, env, path::PathBuf, str::FromStr};
use once_cell::sync::Lazy;
use minreq;
mod stage;

pub static mut FIGHTER_MANAGER_ADDR: usize = 0;

pub static mut amount_printed: i32 = 0;
pub static mut run_0_once_p1: i32 = 0;
pub static mut run_0_once_p2: i32 = 0;
pub static mut fighter_1_match_wins: i32 = 0;
pub static mut fighter_2_match_wins: i32 = 0;
pub static mut ENTRY_ID: usize = 0;
pub static mut STAGE_TABLE: Lazy<stage::StageTable> = Lazy::new(|| stage::StageTable::new(0x4548938, 0x16B));
pub static mut stages_used: Lazy<Vec<String>> = Lazy::new(|| Vec::new());
static mut PARAM_HASHES: Lazy<HashMap<u64, String>> = Lazy::new(|| HashMap::new());
pub static mut stocks_taken: Lazy<Vec<Vec<u8>>> = Lazy::new(|| Vec::new());

static ENDPOINT: &'static str = "http://10.0.0.41:5000/match_end";
const TIMEOUT: u64 = 10;

#[derive(Serialize)]
struct FpInfo {
    pub score: i32
}
#[derive(Serialize)]
struct EndInfo {
    pub fp1_info: FpInfo,
    pub fp2_info: FpInfo
}

extern "C" {
    #[link_name="_ZN3app3nfp10is_enabledEP9lua_State"]
    pub fn nfp_is_enabled(lua_state: u64) -> u64;
}

fn offset_to_addr<T>(offset: usize) -> *mut T {
    unsafe { (skyline::hooks::getRegionAddress(skyline::hooks::Region::Text) as usize + offset) as *mut T }
}

fn get_current_menu() -> u32 {
    return unsafe { *(offset_to_addr(0x53030f0) as *const u32) }
}

fn get_current_stage() -> u32 {
    return unsafe { *(offset_to_addr(0x53087c0) as *const u32) }
}

fn parse_label_file(file_path: &PathBuf, hashmap: &mut HashMap<u64, String>){
    if !file_path.exists() {
        return;
    }

    let text_file_string = read_to_string(file_path).unwrap();
    for line in text_file_string.split("\n") {
        let (hash, value) = line.split_at(line.find(',').unwrap());
        let hash = u64::from_str_radix(hash.trim_start_matches("0x"), 16).unwrap();
        let value = &value[1..];
        if hashmap.contains_key(&hash){
            continue;
        }
        hashmap.insert(hash, value.to_string());
    }
}

#[skyline::hook(replace = nfp_is_enabled)]
pub fn enable_hook(lua_state: u64) -> u64 {
    let result = original!()(lua_state);
    if result == 0 {
        println!("[match_end] One of the fighters is not an amiibo, exiting.");
        let millis = time::Duration::from_millis(500);
        let now = time::Instant::now();
        thread::sleep(millis);
        unsafe{
            skyline::nn::oe::ExitApplication();
        }
    }
    return result
}

// Inline context hook instead to avoid the place HDR hooks
#[skyline::hook(offset = 0x178a090 + (4 * 5), inline)]
unsafe fn setup_stage_offseted(
    ctx: &mut InlineCtx
    // stage_morph_id: u64, -> $x0
    // mut stage_id: u32,   -> $w1
    // ui_bgm_id: u64,      -> $x2
    // mut hazards_on: bool,-> $w3
    // mc_biomes_type: u32  -> $w4
)
{
    let stage_id = *ctx.registers[1].w.as_ref();
    let str_stage_id = PARAM_HASHES[&STAGE_TABLE.table[stage_id as usize].param_name_hash.0].clone();
    if str_stage_id != "settingstage".to_string()
    {
        println!("{}", str_stage_id);
        stages_used.push(str_stage_id.clone())
    }
}

#[fighter_frame_callback]
pub fn once_per_fighter_frame(fighter: &mut L2CFighterCommon)
{
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
        // println!("{}", FighterInformation::stock_count(fighter_2_information) as u8);
        if FighterInformation::stock_count(fighter_1_information) == 3 && run_0_once_p1 == 1 
        {
            run_0_once_p1 = 0;
            amount_printed = 0;
        }
        if FighterInformation::stock_count(fighter_2_information) == 3 && run_0_once_p2 == 1 
        {
            run_0_once_p2 = 0;
            amount_printed = 0;
        }

        if FighterInformation::stock_count(fighter_1_information) == 0 && run_0_once_p1 == 0 
        {
            fighter_2_match_wins = fighter_2_match_wins + 1;
            run_0_once_p1 = 1;
            stocks_taken.push(vec![FighterInformation::stock_count(fighter_2_information) as u8 - 3 ,3])
        }

        if FighterInformation::stock_count(fighter_2_information) == 0 && run_0_once_p2 == 0 
        {
            fighter_1_match_wins = fighter_1_match_wins + 1;
            run_0_once_p2 = 1;
            stocks_taken.push(vec![3, FighterInformation::stock_count(fighter_2_information) as u8 - 3])
        }

        if FighterManager::is_result_mode(fighter_manager) && FighterManager::entry_count(fighter_manager) > 0 
        {
            if amount_printed == 0 {
                let fp1_info = FpInfo{
                    score: fighter_1_match_wins
                };
                let fp2_info = FpInfo{
                    score: fighter_2_match_wins
                };

                let info = EndInfo {
                    fp1_info: fp1_info,
                    fp2_info: fp2_info
                };

                match minreq::post(ENDPOINT).with_header("Content-Type",  "application/json").with_json(&info).unwrap().with_timeout(TIMEOUT).send() {
                    Ok(s) => {
                        println!("Request sent!");
                    }
                    Err(err) => {
                        println!("{:?}", err);
                    }
                };
                if fighter_1_match_wins > fighter_2_match_wins {
                    println!("[match_end] Player 1 won. {}-{}", fighter_1_match_wins, fighter_2_match_wins);
                }
                if fighter_2_match_wins > fighter_1_match_wins {
                    println!("[match_end] Player 2 won. {}-{}", fighter_1_match_wins, fighter_2_match_wins);
                }
                let mut stage_string = "".to_string();
                for stage in stages_used.clone().into_iter() {
                        stage_string += stage.as_str();
                        stage_string += ", ";
                }
                stage_string = stage_string.strip_suffix(", ").unwrap().to_string();
                println!("[match_end] Player .{}-{}", fighter_1_match_wins, fighter_2_match_wins);

                amount_printed = amount_printed + 1;
                fighter_1_match_wins = 0;
                fighter_2_match_wins = 0;
                // dbg!(&stage_string);
                // dbg!(&stocks_taken);
                *stages_used = Vec::new();
                *stocks_taken = Vec::new()
            }
        }
    }
}

#[skyline::main(name = "match_end")]
pub fn main() {
    skyline::install_hooks!(enable_hook, setup_stage_offseted);
    install_agent_frame_callbacks!(once_per_fighter_frame);

    let path = PathBuf::from_str("sd:/ultimate/match_end/ParamLabels.csv").unwrap();
    unsafe { parse_label_file(&path, &mut *PARAM_HASHES) };
    // std::thread::spawn( || {
    //     loop {
    //         std::thread::sleep(std::time::Duration::from_secs(4));
    //         println!("[match_end] current_menu_id: {:#09x}", get_current_menu());
    //     }
    // });
}
