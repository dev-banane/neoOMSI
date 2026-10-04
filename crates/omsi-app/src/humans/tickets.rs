use super::*;

/// Nothing at midnight, all of it at 9:00, then falling on OMSI's line.
fn day_ticket_factor(t: f64) -> f32 {
    let t = t.rem_euclid(86_400.0);
    let rise = t / 32_400.0;
    let fall = 1.0 - (t - 32_400.0) / (88_776.0 - 32_400.0);
    rise.min(fall).clamp(0.0, 1.0) as f32
}

impl Humans {
    /// `None`: the player's bus.
    pub fn take_stamped(&mut self) -> Vec<Option<u64>> {
        std::mem::take(&mut self.stamped)
            .into_iter()
            .map(|b| match b {
                BusId::Ai(id) => Some(id),
                BusId::Player => None,
            })
            .collect()
    }

    fn pick_ticket(&mut self, age: f32) -> Option<usize> {
        let r = self.rand_f() as f32;
        let day = day_ticket_factor(self.time_of_day);
        let t = self.tickets.as_ref()?;
        let weight = |tk: &omsi_content::tickets::Ticket| {
            if (tk.age_min as f32) > age || (tk.age_max as f32) < age {
                0.0
            } else if tk.day_ticket {
                tk.probability.max(0.0) * day
            } else {
                tk.probability.max(0.0)
            }
        };
        let total: f32 = t.tickets.iter().map(weight).sum();
        if total <= 0.0 {
            return None;
        }
        let mut x = r * total;
        for (i, tk) in t.tickets.iter().enumerate() {
            let w = weight(tk);
            if w > 0.0 && x < w {
                return Some(i);
            }
            x -= w;
        }
        None
    }

    pub(super) fn decide_pax_ticket(&mut self, i: usize, bn: &BusNow) -> (Ticket, u8) {
        let Some(tp) = self.tickets.clone() else {
            return (Ticket::None, 0);
        };
        let mut r = self.rand_f() as f32;
        if bn.cabin.stamper.is_some() {
            if r < tp.stamper_prop {
                return (Ticket::Stamp, 0);
            }
            r -= tp.stamper_prop;
        }
        if bn.cabin.sale.is_some()
            && r < tp.ticketbuy_prop
            && !self.boarding.eq_ignore_ascii_case("walk")
        {
            let id = self
                .pick_ticket(self.people[i].age)
                .map_or(0, |t| (t + 1).min(255) as u8);
            return (Ticket::Buy, id);
        }
        (Ticket::None, 0)
    }

    pub fn hint(&self) -> Option<String> {
        if !self.boarding.eq_ignore_ascii_case("pay") {
            return None;
        }
        self.people.iter().find(|p| {
            matches!(&p.state, State::Pax(x) if x.inside == Some(BusId::Player) && x.ticket == Ticket::Buy && x.sub == 5)
        })?;
        let (name, value) = self.request.clone()?;
        Some(format!(
            "Passenger waiting for a ticket: {name} {value:.2} - press {}",
            self.ticket_key
        ))
    }

    pub fn take_change_tray(&mut self) {
        if let Some(m) = self.money.as_mut() {
            m.clear(true);
        }
    }

    pub fn give_change(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        coins: &[usize],
    ) {
        if coins.is_empty() {
            return;
        }
        let point = self.player_cabin.as_ref().and_then(|c| {
            c.data
                .change_points
                .first()
                .or(c.data.money_points.first())
                .cloned()
        });
        if let (Some(m), Some(pt)) = (self.money.as_mut(), point) {
            m.place(
                world,
                renderer,
                scene,
                coins,
                Vec3::from(pt.pos),
                pt.var,
                true,
            );
        }
    }

    pub fn sync_money(&mut self, renderer: &Renderer, scene: &mut Scene, bus: &VehicleInstance) {
        if let Some(m) = self.money.as_mut() {
            m.sync(renderer, scene, bus);
        }
    }

    /// The passenger asks again every 5 s (3 s from the third time) until the driver gets it
    /// right; `pardons` is shared by everybody at the desk, as in Omsi.exe.
    pub(super) fn desk_sale(
        &mut self,
        i: usize,
        bn: &BusNow,
        f: &Frame,
        renderer: &Renderer,
        scene: &mut Scene,
        taken_ticket: &mut bool,
    ) {
        let p = self.pax(i).unwrap().clone();
        // `auto` boarding plays the driver (a setting; OMSI has no such mode)
        let auto = self.boarding.eq_ignore_ascii_case("auto");
        let id = p.ticket_id as usize;
        let (name, value) = self
            .tickets
            .as_ref()
            .and_then(|t| t.tickets.get(id.saturating_sub(1)))
            .map(|t| (t.name.clone(), t.value))
            .unwrap_or_default();
        let tol = self.money.as_ref().map_or(0.01, |m| m.smallest_value()) / 2.0;
        let owed = p.paid - p.price;
        let (ch_ok, too_much, many) = if p.sub != 7 {
            (false, false, false)
        } else if auto {
            (true, false, false)
        } else {
            let given = self.money.as_ref().map_or(0.0, |m| m.change_value());
            let too_much = tol < given - owed;
            let enough = owed - given <= tol;
            let needed =
                self.money
                    .as_mut()
                    .map_or(0, |m| m.exact_coins_for(owed.max(0.0)).len()) as f32;
            let count = self.money.as_ref().map_or(0, |m| m.change_count()) as f32;
            let r = self.rand_f() as f32;
            (
                enough && !too_much,
                too_much,
                needed * (r + 1.5) <= count && count > 0.0,
            )
        };
        let (tk_ok, wrong) = match f
            .player_bus
            .and_then(|b| b.var("GivenTicket"))
            .unwrap_or(-1.0)
        {
            _ if p.sub != 5 => (false, false),
            _ if auto || self.give_ticket => (true, false),
            given if given < 0.0 => (false, false),
            given => {
                let ok = (given - (id as f32 - 1.0)).abs() < 0.5;
                (ok, !ok)
            }
        };
        match p.sub {
            3 => {
                if self.desk_busy.is_some_and(|d| d != self.people[i].id) {
                    return;
                }
                let k = 1 + self.rand() % 2;
                self.say(i, &format!("Ticket_{id}_{k}"), false);
                self.ticket_requests += 1;
                self.request = Some((name, value));
                self.desk_busy = Some(self.people[i].id);
                self.pardons = 0;
                let p = self.pax_mut(i).unwrap();
                p.timer = 0.5;
                p.reach = true;
                p.sub = 4;
                p.paid = 0.0;
            }
            4 if p.timer <= 0.0 => {
                let mut paid = value;
                if let Some(m) = self.money.as_mut() {
                    let coins = if self.exact_fare || auto {
                        m.exact_coins_for(value)
                    } else {
                        m.omsi_coins_for(value)
                    };
                    paid = m.value_of(&coins);
                    if let Some((pos, var)) = bn.cabin.money {
                        m.place(f.world, renderer, scene, &coins, pos, var, false);
                    }
                }
                self.paid = Some((paid, value));
                let p = self.pax_mut(i).unwrap();
                p.paid = paid;
                p.sub = 5;
                p.reach = false;
                p.look_driver = false;
                p.timer = 10.0;
            }
            5 if tk_ok => {
                let p = self.pax_mut(i).unwrap();
                p.sub = 6;
                p.timer = 0.5;
                p.reach = true;
                if let Some((_, t)) = bn.cabin.sale {
                    p.reach_at = t;
                }
                self.pardons = 0;
            }
            6 if p.timer <= 0.0 => {
                self.tickets_sold += 1;
                self.ticket_cash += value;
                *taken_ticket = true;
                if let Some(m) = self.money.as_mut() {
                    m.clear(false);
                }
                self.paid = None;
                let change = (p.price - p.paid).abs() > tol && !auto;
                self.change_due = Some(if change { owed } else { 0.0 });
                let p = self.pax_mut(i).unwrap();
                p.reach = false;
                p.look_driver = false;
                p.sub = if change { 7 } else { 8 };
                if change {
                    p.timer = 5.0;
                }
            }
            7 if ch_ok || (self.pardons > 1 && too_much) => {
                let p = self.pax_mut(i).unwrap();
                p.sub = 8;
                p.timer = 0.5;
                p.reach = true;
                if let Some(c) = bn.cabin.change_point {
                    p.reach_at = c;
                }
                if many {
                    self.say(i, "BadChange_1", true);
                    self.ticket_points += 1;
                } else {
                    if !too_much {
                        self.ticket_points += 2;
                    }
                    let r = self.rand_f() as f32;
                    if !too_much && self.tickets.as_ref().is_some_and(|t| r < t.chattiness) {
                        self.say(i, "Thanks_1", false);
                    }
                }
            }
            8 if p.timer <= 0.0 => {
                if let Some(m) = self.money.as_mut() {
                    m.clear(true);
                }
                self.change_due = None;
                self.request = None;
                self.desk_busy = None;
                self.boarded += 1;
                self.served += 1;
                self.route_to_place(i, bn);
                let p = self.pax_mut(i).unwrap();
                p.reach = false;
                p.look_driver = false;
                p.sub = 0;
                p.ticket = Ticket::None;
                p.pt = bn.cabin.sale.and_then(|s| s.0);
            }
            5 | 7 if p.timer <= 0.0 || (too_much && self.pardons == 0) => {
                let n = self.pardons as u64;
                let r = self.rand() % (n + 2);
                self.pax_mut(i).unwrap().timer = if n < 2 { 5.0 } else { 3.0 };
                let line = match (p.sub, n) {
                    (5, _) if n == 0 || r == 0 => if wrong {
                        "BadTicket_A"
                    } else {
                        "PardonTicket_1"
                    }
                    .to_string(),
                    (5, 1) if wrong => "BadTicket_B".to_string(),
                    (7, 0) => if too_much { "TooMuch_A" } else { "TooFew_A" }.to_string(),
                    (7, 1) if !too_much => "TooFew_B".to_string(),
                    (7, 1) => {
                        self.pardons = 2;
                        "TooMuch_B".to_string()
                    }
                    _ => format!("Pardon_{}", r.min(3)),
                };
                self.say(i, &line, true);
                if self.pardons == 0 || self.rand_f() < 0.7 {
                    self.pardons = self.pardons.saturating_add(1);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berlin_91() -> omsi_content::tickets::TicketPack {
        let t = |name: &str, age: (i32, i32), day: bool, p: f32| omsi_content::tickets::Ticket {
            name: name.into(),
            age_min: age.0,
            age_max: age.1,
            day_ticket: day,
            probability: p,
            ..Default::default()
        };
        omsi_content::tickets::TicketPack {
            stamper_prop: 0.3,
            ticketbuy_prop: 0.2,
            tickets: vec![
                t("Fahrschein", (14, 200), false, 1.0),
                t("Kurzstrecke", (14, 200), false, 0.4),
                t("Tageskarte", (14, 200), true, 0.2),
                t("Ermaessigt", (6, 13), false, 1.0),
                t("Kurzstrecke Erm", (6, 13), false, 0.4),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn tickets_by_age_and_time() {
        let mut h = Humans::new(Path::new("/nonexistent"));
        h.tickets = Some(Arc::new(berlin_91()));
        let count = |h: &mut Humans, age: f32| {
            let mut n = [0usize; 5];
            for _ in 0..4000 {
                n[h.pick_ticket(age).unwrap()] += 1;
            }
            n
        };
        h.time_of_day = 9.0 * 3600.0;
        let adult = count(&mut h, 40.0);
        assert_eq!(adult[3] + adult[4], 0);
        assert!(adult[2] > 300, "{adult:?}");
        let child = count(&mut h, 10.0);
        assert_eq!(child[0] + child[1] + child[2], 0);
        h.time_of_day = 1.0 * 3600.0;
        let early = count(&mut h, 40.0);
        assert!(early[2] * 4 < adult[2], "{early:?} vs {adult:?}");
        assert!(day_ticket_factor(9.0 * 3600.0) > 0.99);
        assert!(day_ticket_factor(0.0) < 0.01);
        assert!(
            (day_ticket_factor(20.0 * 3600.0) - (1.0 - 39_600.0 / 56_376.0) as f32).abs() < 1e-3
        );
        assert_eq!(h.pick_ticket(3.0), None);
    }
}
