//use crate::api;
use crate::simulation::scenario;
use crate::ui::telemetry;
use crate::ui::userid;
use crate::ui::UI;
//use crate::worker_api::WorkerRequest;
use log::{error, info};
use rand::Rng;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::prelude::*;

static PANICKED: AtomicBool = AtomicBool::new(false);

fn has_panicked() -> bool {
    PANICKED.load(Ordering::SeqCst)
}

#[wasm_bindgen]
pub struct Game {
    ui: Option<Box<UI>>,
}

#[wasm_bindgen]
impl Game {
    pub fn start(&mut self, scenario_name: &str, code: &str) {
        if has_panicked() {
            return;
        }
        let seed = rand::thread_rng().gen();
        self.ui = Some(Box::new(UI::new(scenario_name, seed, code)));
        /*
        api::send_worker_request(&WorkerRequest::StartScenario {
            scenario_name: scenario_name.to_owned(),
            seed: rand::thread_rng().gen(),
            code: code.to_owned(),
        });
        */
    }

    pub fn render(&mut self) {
        if has_panicked() {
            return;
        }
        if self.ui.is_some() {
            self.ui.as_mut().unwrap().render();
        }
    }

    pub fn on_snapshot(&mut self, value: &[u8]) {
        if has_panicked() {
            return;
        }
        if self.ui.is_some() {
            self.ui
                .as_mut()
                .unwrap()
                .on_snapshot(bincode::deserialize(value).unwrap());
        }
    }

    pub fn on_key_event(&mut self, e: web_sys::KeyboardEvent) {
        if has_panicked() {
            return;
        }
        if self.ui.is_some() {
            self.ui.as_mut().unwrap().on_key_event(e);
        }
    }

    pub fn on_wheel_event(&mut self, e: web_sys::WheelEvent) {
        if has_panicked() {
            return;
        }
        if self.ui.is_some() {
            self.ui.as_mut().unwrap().on_wheel_event(e);
        }
    }

    pub fn get_initial_code(&self, scenario_name: &str) -> String {
        if has_panicked() {
            return "".to_string();
        }
        scenario::load(scenario_name).initial_code()
    }

    pub fn get_solution_code(&mut self, scenario_name: &str) -> String {
        if has_panicked() {
            return "".to_string();
        }
        scenario::load(scenario_name).solution()
    }

    pub fn get_saved_code(&mut self, scenario_name: &str) -> String {
        if has_panicked() {
            return "".to_string();
        }
        let window = web_sys::window().expect("no global `window` exists");
        let storage = window
            .local_storage()
            .expect("failed to get local storage")
            .unwrap();
        let initial_code = scenario::load(scenario_name).initial_code();
        match storage.get_item(&format!("/code/{}", scenario_name)) {
            Ok(Some(code)) => code,
            Ok(None) => {
                info!("No saved code, using starter code");
                initial_code
            }
            Err(msg) => {
                error!("Failed to load code: {:?}", msg);
                initial_code
            }
        }
    }

    pub fn save_code(&mut self, scenario_name: &str, code: &str) {
        if has_panicked() {
            return;
        }
        let window = web_sys::window().expect("no global `window` exists");
        if !code.is_empty() {
            let storage = window
                .local_storage()
                .expect("failed to get local storage")
                .unwrap();
            if let Err(msg) = storage.set_item(&format!("/code/{}", scenario_name), code) {
                error!("Failed to save code: {:?}", msg);
            }
        }
    }

    pub fn get_userid(&self) -> String {
        userid::get_userid()
    }

    pub fn get_username(&self, userid: &str) -> String {
        userid::get_username(userid)
    }

    pub fn get_scenarios(&self) -> JsValue {
        JsValue::from_serde(&scenario::list()).unwrap()
    }

    pub fn finished_background_simulations(&mut self, results: js_sys::Array) {
        if has_panicked() {
            return;
        }
        let mut snapshots = vec![];
        for x in results.iter() {
            let x = js_sys::Uint8Array::from(x);
            snapshots.push(bincode::deserialize(&x.to_vec()).unwrap())
        }
        if self.ui.is_some() {
            self.ui
                .as_mut()
                .unwrap()
                .finished_background_simulations(&snapshots);
        }
    }
}

#[wasm_bindgen]
pub fn create_game() -> Game {
    std::panic::set_hook(Box::new(|panic_info| {
        if has_panicked() {
            return;
        }
        console_error_panic_hook::hook(panic_info);
        telemetry::send(telemetry::Telemetry::Crash {
            msg: panic_info.to_string(),
        });
        PANICKED.store(true, Ordering::SeqCst);
    }));
    console_log::init_with_level(log::Level::Info).expect("initializing logging");
    log::info!("Version {}", &crate::version());
    Game { ui: None }
}

pub fn create() -> Game {
    console_log::init_with_level(log::Level::Info).expect("initializing logging");
    log::info!("Version {}", &crate::version());
    Game { ui: None }
}
