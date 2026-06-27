use slint::ComponentHandle;
use std::collections::HashMap;

use slint::{Color, SharedString};

use super::{col, model, sstr};
use crate::state::State;
use crate::util::car_variant;
use crate::{AppWindow, CarLib, CarRow};

pub fn rebuild_filtered(s: &mut State) {
    s.filtered.clear();
    let sim = s.sim_of(s.game);
    let q = s.query.to_lowercase();
    let klass_sel = if s.klass > 0 && (s.klass as usize) < s.class_list.len() {
        s.class_list[s.klass as usize].clone()
    } else {
        String::new()
    };
    for (i, c) in s.all_cars.iter().enumerate() {
        if c.sim != sim {
            continue;
        }
        if !klass_sel.is_empty() && c.klass != klass_sel {
            continue;
        }
        if !q.is_empty() && !c.name.to_lowercase().contains(&q) {
            continue;
        }
        s.filtered.push(i);
    }
}

pub fn push_car_results(ui: &AppWindow, s: &State) {
    let mut name_count: HashMap<String, i32> = HashMap::new();
    for &idx in &s.filtered {
        *name_count
            .entry(s.all_cars[idx].name.to_lowercase())
            .or_insert(0) += 1;
    }
    let sim = s.sim_of(s.game);
    let rows: Vec<CarRow> = s
        .filtered
        .iter()
        .map(|&idx| {
            let c = &s.all_cars[idx];
            let lname = c.name.to_lowercase();
            let mut disp = c.name.clone();
            if name_count.get(&lname).copied().unwrap_or(0) > 1 {
                let v = car_variant(&c.id, &c.name);
                if !v.is_empty() {
                    disp = format!("{} · {}", c.name, v);
                }
            }
            let badge: String = c.name.chars().take(3).collect();
            let make = c.name.split(' ').next().unwrap_or("").to_string();
            let active = c.sim == sim
                && (if !s.car_id.is_empty() {
                    c.id == s.car_id
                } else {
                    c.name == s.car_name
                });
            let leds: Vec<Color> = c.led_cols.iter().map(|&col_| col(col_)).collect();
            CarRow {
                id: sstr(&c.id),
                badge: sstr(&badge),
                name: sstr(&disp),
                make: sstr(&make),
                klass: sstr(&c.klass),
                year: 0,
                power: sstr(&if c.led_n != 0 {
                    format!("{} LED", c.led_n)
                } else {
                    String::new()
                }),
                redline: c.redline,
                active,
                leds: model(leds),
            }
        })
        .collect();
    let cl = ui.global::<CarLib>();
    cl.set_results(model(rows));
    cl.set_total_cars(s.all_cars.len() as i32);
    cl.set_have_db(!s.all_cars.is_empty());
    cl.set_db_info(sstr(&if s.all_cars.is_empty() {
        "no local database".to_string()
    } else {
        format!("{} cars · cached", s.all_cars.len())
    }));
}

pub fn push_classes(ui: &AppWindow, s: &mut State) {
    let sim = s.sim_of(s.game);
    let mut cl = vec!["All classes".to_string()];
    for c in &s.all_cars {
        if c.sim == sim && !c.klass.is_empty() && !cl.contains(&c.klass) {
            cl.push(c.klass.clone());
        }
    }
    cl[1..].sort();
    s.class_list = cl.clone();
    let m: Vec<SharedString> = cl.iter().map(|x| sstr(x)).collect();
    let clib = ui.global::<CarLib>();
    clib.set_classes(model(m));
    if s.klass >= cl.len() as i32 {
        s.klass = 0;
    }
    clib.set_klass(s.klass);
}

pub fn push_games(ui: &AppWindow, s: &State) {
    let m: Vec<SharedString> = s.sims.iter().map(|x| sstr(&x.0)).collect();
    let cl = ui.global::<CarLib>();
    cl.set_games(model(m));
    cl.set_game(s.game);
}
