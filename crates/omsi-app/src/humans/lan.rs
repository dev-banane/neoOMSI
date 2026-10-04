use super::*;

#[derive(Default)]
pub(super) struct Lan {
    pub(super) mirror: bool,
    pub(super) remote_now: Vec<BusNow>,
    claims_out: Vec<u32>,
    claimed: HashMap<u32, f64>,
    mirror_wait: HashMap<u32, (i64, usize)>,
    pub(super) handed: Vec<(i64, u64)>,
}

#[derive(Debug, Clone, Copy)]
pub struct MirrorPose {
    pub pos: DVec3,
    pub heading: f64,
    pub vel: DVec2,
    pub activity: Activity,
    pub aboard: Option<(u64, Vec3, f64, Option<usize>)>,
    pub waiting: Option<(i64, usize)>,
}

/// Far above the traffic's car ids.
pub fn remote_bus_id(player: u32) -> u64 {
    (1 << 40) | player as u64
}

pub fn placed_bus_id(uid: u64) -> u64 {
    (2 << 40) | uid
}

pub fn remote_bus_player(bus: u64) -> Option<u32> {
    (bus >> 40 == 1).then_some((bus & 0xFFFF_FFFF) as u32)
}

pub struct LanPerson {
    pub id: u32,
    pub ty: Arc<HumanType>,
    pub pos: DVec3,
    pub heading: f64,
    pub speed: f64,
    pub activity: Activity,
    pub aboard: Option<(u64, Vec3, f64, Option<usize>)>,
    pub waiting: Option<(i64, usize)>,
}

impl Person {
    fn lan(&self, aboard: Option<(u64, Vec3)>, waiting: Option<(i64, usize)>) -> LanPerson {
        let seat = match &self.state {
            State::Pax(x) if x.task == Task::SittingInBus => x.seat,
            _ => None,
        };
        LanPerson {
            id: self.id,
            ty: self.ty.clone(),
            pos: self.position,
            heading: self.heading,
            speed: if aboard.is_some() {
                0.0
            } else {
                self.vel.length()
            },
            activity: self.activity,
            aboard: aboard.map(|(bus, l)| (bus, l, self.lheading, seat)),
            waiting,
        }
    }
}

impl Humans {
    /// The room id as the shared randomness: host and clients draw the same people at first.
    pub fn set_lan_seed(&mut self, seed: u64) {
        self.rng = (seed ^ 0xA5A5_5A5A_1F2E_3D4C) | 1;
    }

    pub fn has_mirror(&self, id: u32) -> bool {
        self.people.iter().any(|p| p.id == id && p.remote)
    }

    pub fn lan_riders(&self) -> Vec<LanPerson> {
        self.people
            .iter()
            .filter(|p| !p.avatar && !p.remote)
            .filter_map(|p| match p.place {
                Place::Bus(BusId::Player, l) => Some(p.lan(Some((0, l)), None)),
                _ => None,
            })
            .collect()
    }

    pub fn lan_people(&self, near: DVec3, radius: f64) -> Vec<LanPerson> {
        self.people
            .iter()
            .filter(|p| {
                !p.avatar && !p.remote && (p.position - near).length_squared() < radius * radius
            })
            .filter_map(|p| {
                let aboard = match p.place {
                    Place::Bus(BusId::Player, _) => return None,
                    Place::Bus(BusId::Ai(bus), l) => Some((bus, l)),
                    Place::Ground => None,
                };
                let waiting = match &p.state {
                    State::Pax(x) if aboard.is_none() && x.task == Task::WaitingForBus => {
                        x.stop.zip(x.spot)
                    }
                    _ => None,
                };
                Some(p.lan(aboard, waiting))
            })
            .collect()
    }

    pub(super) fn far_from_players(&self, p: DVec3, r: f64) -> bool {
        (p - self.center).length() > r && self.lan_centers.iter().all(|c| (p - *c).length() > r)
    }

    pub(super) fn populate_lan_centers(
        &mut self,
        world: &World,
        net: &Network,
        renderer: &Renderer,
        scene: &mut Scene,
    ) {
        let mine = self.center;
        for c in self.lan_centers.clone() {
            if (c - mine).length() >= 150.0 {
                self.populate_with(world, Some(net), c);
                self.populate_on_foot(world, net, renderer, scene);
            }
        }
        self.center = mine;
    }

    pub fn hand_over(&mut self, player: u32, ids: &[u32]) -> Vec<u32> {
        let mut out = Vec::new();
        for &id in ids {
            let Some(i) = self
                .people
                .iter()
                .position(|p| p.id == id)
                .filter(|&i| !self.people[i].remote)
            else {
                continue;
            };
            let Some(x) = self.pax(i).filter(|x| x.task == Task::WaitingForBus) else {
                continue;
            };
            if let Some(stop) = x.stop {
                self.lan.handed.push((stop, remote_bus_id(player)));
            }
            self.remove_person(i);
            out.push(id);
        }
        out
    }

    pub fn set_mirror(&mut self, on: bool) {
        if self.lan.mirror == on {
            return;
        }
        self.lan.mirror = on;
        let mut i = 0;
        while i < self.people.len() {
            let p = &self.people[i];
            if p.avatar || (!p.remote && p.state.bus() == Some(BusId::Player)) {
                i += 1;
            } else {
                self.remove_person(i);
            }
        }
        // our own people are numbered far above the host's
        if on {
            self.next_id = self.next_id.max(1 << 30);
            for p in self.people.iter_mut().filter(|p| !p.avatar) {
                p.id = self.next_id;
                self.next_id += 1;
            }
        }
        for s in self.stops.values_mut() {
            s.taken.fill(false);
        }
        self.lan.claims_out.clear();
        self.lan.claimed.clear();
        self.lan.mirror_wait.clear();
    }

    pub fn mirror_add(
        &mut self,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        id: u32,
        ty: usize,
        pose: &MirrorPose,
    ) -> bool {
        if self.people.iter().any(|p| p.id == id) {
            return false;
        }
        let Some(i) = self.spawn_as(
            world,
            renderer,
            scene,
            pose.pos,
            pose.heading,
            State::Idle,
            Some(ty),
        ) else {
            return false;
        };
        self.next_id -= 1;
        let p = &mut self.people[i];
        p.id = id;
        p.remote = true;
        self.mirror_set(id, pose);
        true
    }

    pub fn mirror_set(&mut self, id: u32, pose: &MirrorPose) {
        let Some(p) = self.people.iter_mut().find(|p| p.id == id && p.remote) else {
            return;
        };
        p.position = pose.pos;
        p.heading = pose.heading;
        p.vel = pose.vel;
        p.activity = pose.activity;
        match pose.waiting {
            Some(w) => self.lan.mirror_wait.insert(id, w),
            None => self.lan.mirror_wait.remove(&id),
        };
        match pose.aboard {
            Some((bus, local, lheading, _)) => {
                p.place = Place::Bus(BusId::Ai(bus), local);
                p.lheading = lheading;
                p.vel = DVec2::ZERO;
            }
            None => p.place = Place::Ground,
        }
    }

    pub fn mirror_remove(&mut self, id: u32) {
        if let Some(i) = self.people.iter().position(|p| p.id == id && p.remote) {
            self.remove_person(i);
        }
        self.lan.claimed.remove(&id);
        self.lan.mirror_wait.remove(&id);
    }

    pub fn lan_people_by_id(&self, id: u32) -> Option<Arc<HumanType>> {
        self.people
            .iter()
            .find(|p| p.id == id && !p.remote)
            .map(|p| p.ty.clone())
    }

    pub fn mirror_positions(&self) -> Vec<(u32, DVec3)> {
        self.people
            .iter()
            .filter(|p| p.remote && p.place == Place::Ground)
            .map(|p| (p.id, p.position))
            .collect()
    }

    pub fn take_claims(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.lan.claims_out)
    }

    pub fn grant(&mut self, id: u32) -> bool {
        self.lan.claimed.remove(&id);
        let Some((stop, spot)) = self.lan.mirror_wait.remove(&id) else {
            return false;
        };
        let Some(i) = self.people.iter().position(|p| p.id == id && p.remote) else {
            return false;
        };
        let Some(sp) = self.stops.get(&stop).map(|s| s.spots.get(spot).cloned()) else {
            return false;
        };
        let seat_height = self.people[i].ty.def.seat_height;
        let mut pax = Pax::new(self.walk_pace() as f32);
        pax.task = Task::WaitingForBus;
        pax.stop = Some(stop);
        (pax.dest, pax.line) = self.draw_dest(stop);
        pax.ride_km = self.ride_km();
        pax.pos = self.people[i].position;
        pax.yaw = self.people[i].heading.to_radians();
        if let Some(sp) = sp {
            pax.spot = Some(spot);
            if let Some(t) = self.stops.get_mut(&stop).unwrap().taken.get_mut(spot) {
                *t = true;
            }
            if sp.height != 0.0 {
                pax.seat_h = sp.height;
                pax.pos = sp.pos - DVec3::Z * seat_height as f64;
                pax.pax_state = 2;
            }
            pax.yaw = sp.face.to_radians();
        }
        let p = &mut self.people[i];
        p.remote = false;
        p.state = State::Pax(Box::new(pax));
        true
    }

    pub(super) fn claim_waiting(&mut self) {
        if !self.lan.mirror {
            return;
        }
        let now = self.time;
        self.lan.claimed.retain(|_, t| now - *t < 10.0);
        let at: Vec<i64> = self
            .stops
            .iter()
            .filter(|(_, s)| s.buses.iter().any(|b| b.0 == BusId::Player))
            .map(|(id, _)| *id)
            .collect();
        let lan = &mut self.lan;
        for (id, (stop, _)) in &lan.mirror_wait {
            if at.contains(stop) && !lan.claimed.contains_key(id) {
                lan.claimed.insert(*id, now);
                lan.claims_out.push(*id);
            }
        }
    }
}
