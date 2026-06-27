use super::{model, sstr};
use crate::state::State;
use crate::{AppWindow, SimField, SimHub};
use slint::ComponentHandle;

pub fn push_sim_fields(ui: &AppWindow, s: &State) {
    let q = s.sim_query.to_lowercase();
    let fs: Vec<SimField> = s
        .sim
        .iter()
        .filter(|r| {
            q.is_empty()
                || format!("{} {}", r.label, r.expr)
                    .to_lowercase()
                    .contains(&q)
        })
        .map(|r| SimField {
            id: sstr(&r.id),
            label: sstr(&r.label),
            expr: sstr(&r.expr),
            enabled: r.enabled,
            builtin: r.builtin,
        })
        .collect();
    let sh = ui.global::<SimHub>();
    sh.set_fields(model(fs));
    sh.set_total_fields(s.sim.len() as i32);
}

pub fn regen_simhub(ui: &AppWindow, s: &State) {
    let mut expr = "'$' + ".to_string();
    let mut first = true;
    for r in &s.sim {
        if !first {
            expr.push_str(" + ';' + ");
        }
        first = false;
        if r.enabled {
            expr.push_str(&format!("({})", r.expr));
        } else {
            expr.push_str("'0'");
        }
    }
    expr.push_str(" + '\\n'");
    let sh = ui.global::<SimHub>();
    sh.set_generated(sstr(&expr));
    sh.set_car_message(sstr("'@CM' + isnull([CarModel],'') + '\\n'"));
}
