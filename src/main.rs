#![feature(proc_macro_hygiene, decl_macro)]

use std::fs::File;
use std::io::prelude::*;
use std::thread;
use std::sync::{mpsc, Arc, RwLock};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use chrono::prelude::*;
use rocket::State;

// static PROC_FILENAME: &'static str = "/sys/class/gpio/gpio17/value";
static PROC_FILENAME: &'static str = "./test";

#[derive(Debug, Clone)]
struct CoffeeState {
    brewing: bool,
    last_brewed: Option<DateTime<Utc>>,
}

extern crate chrono;
#[macro_use]
extern crate rocket;

#[get("/")]
fn index(state_lock: State<Arc<RwLock<CoffeeState>>>) -> String {
    let current_state = state_lock.read().unwrap();
    match current_state.last_brewed {
        Some(time) => format!("Coffee last brewed at {}", time),
        None => String::from("Coffee not brewed recently!"),
    }
}

fn start_poller_thread(tx: Sender<bool>) {
    thread::spawn(move || loop {
        let file = File::open(PROC_FILENAME);
        match file {
            Ok(mut f) => {
                let mut contents = String::new();
                let read_result = f.read_to_string(&mut contents);
                match read_result {
                    Ok(_) => match contents.as_ref() {
                        // TODO(nate): check if we need these newlines
                        "1\n" => tx.send(true).unwrap(),
                        "0\n" => tx.send(false).unwrap(),
                        _ => ()
                    },
                    _ => (),
                }
            }
            _ => {}
        }
        thread::sleep(Duration::from_millis(100));
    });
}

fn start_reader_thread(rx: Receiver<bool>, lock: Arc<RwLock<CoffeeState>>) {
    thread::spawn(move || {
        loop {
            let is_brewing = rx.recv().unwrap();
            let mut state = lock.write().unwrap();
            if state.brewing && !is_brewing {
                // we were previously brewing, now we are done
                state.brewing = false;
                state.last_brewed = Some(Utc::now());
            // TODO(nate): this is where we would fire of a request to
            // slack with the goods
            } else if !state.brewing && is_brewing {
                // we were not previously brewing, now we are
                state.brewing = true;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
}

fn start_poller_and_reader(lock: Arc<RwLock<CoffeeState>>) {
    let (tx, rx) = mpsc::channel();
    start_poller_thread(tx);
    start_reader_thread(rx, lock);
}

fn main() -> std::io::Result<()> {
    let init_state = CoffeeState {
        brewing: false,
        last_brewed: None,
    };
    let lock = Arc::new(RwLock::new(init_state));
    start_poller_and_reader(lock.clone());
    rocket::ignite()
        .mount("/", routes![index])
        .manage(lock)
        .launch();
    Ok(())
}
