use super::*;

const JOINT_BLEND: f32 = 0.5;

/// What the people deal with this frame.
pub(super) struct Frame<'a> {
    pub(super) world: &'a World,
    pub(super) player_bus: Option<&'a VehicleInstance>,
    pub(super) at_stops: HashMap<BusId, BusAtStops>,
    buses: Vec<BusNow>,
    ix: HashMap<BusId, usize>,
}

impl<'a> Frame<'a> {
    pub(super) fn new(
        world: &'a World,
        player_bus: Option<&'a VehicleInstance>,
        buses: Vec<BusNow>,
        at_stops: HashMap<BusId, BusAtStops>,
    ) -> Frame<'a> {
        let ix = buses.iter().enumerate().map(|(i, b)| (b.id, i)).collect();
        Frame {
            world,
            player_bus,
            at_stops,
            buses,
            ix,
        }
    }

    pub(super) fn buses(&self) -> &[BusNow] {
        &self.buses
    }

    pub(super) fn bus(&self, id: Option<BusId>) -> Option<&BusNow> {
        id.and_then(|id| self.ix.get(&id)).map(|k| &self.buses[*k])
    }
}

#[derive(Clone)]
pub(super) struct BusNow {
    pub(super) id: BusId,
    pub(super) cabin: Arc<Cabin>,
    pub(super) pos: DVec3,
    pub(super) rot: Mat4,
    pub(super) heading: f64,
    pub(super) speed: f64,
    pub(super) entry_open: Vec<bool>,
    pub(super) exit_open: Vec<bool>,
    pub(super) walk_open: Option<(Vec<bool>, Vec<bool>)>,
    pub(super) interior: f32,
    pub(super) air: CabinAir,
    pub(super) half: DVec2,
    pub(super) centre: DVec2,
    pub(super) trailers: Vec<PartFrame>,
    pub(super) terminus: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CabinAir {
    pub(super) temp: Option<f32>,
    pub(super) rel_hum: f32,
    pub(super) outside: f32,
    pub(super) brightness: f32,
}

impl CabinAir {
    fn of(v: &VehicleInstance) -> CabinAir {
        CabinAir {
            temp: v.var("Cabinair_Temp").filter(|t| t.is_finite()),
            rel_hum: v
                .var("Cabinair_relHum")
                .filter(|h| h.is_finite())
                .unwrap_or(0.0),
            outside: v.host.temperature,
            brightness: v.var("Envir_Brightness").unwrap_or(1.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PartFrame {
    pub(super) pos: DVec3,
    pub(super) rot: Mat4,
    pub(super) heading: f64,
    pub(super) offset: Vec3,
    pub(super) joint_y: f32,
    pub(super) half: DVec2,
    pub(super) centre: DVec2,
}

pub(super) fn part_frames(v: &VehicleInstance, cabin: &Cabin) -> Vec<PartFrame> {
    let frame = |(cp, t): (&CabinPart, &omsi_sim::vehicle::TrailerPart)| {
        let bb =
            t.ty.def
                .bounding_box
                .unwrap_or([2.5, 7.0, 3.0, 0.0, 0.0, 1.5]);
        PartFrame {
            pos: t.position,
            rot: t.body_rotation(),
            heading: t.heading,
            offset: cp.offset,
            joint_y: cp.joint_y,
            half: DVec2::new(bb[0] as f64 * 0.5, bb[1] as f64 * 0.5),
            centre: DVec2::new(bb[3] as f64, bb[4] as f64),
        }
    };
    cabin
        .parts
        .iter()
        .skip(1)
        .zip(&v.trailers)
        .map(frame)
        .collect()
}

fn behind(t: &PartFrame, y: f32) -> f32 {
    ((JOINT_BLEND - (y - t.joint_y)) / (2.0 * JOINT_BLEND)).clamp(0.0, 1.0)
}

/// Blended near a joint, so that a walk through the bellows has no jump.
pub(super) fn train_point(pos: DVec3, rot: &Mat4, trailers: &[PartFrame], local: Vec3) -> DVec3 {
    let mut here = pos + rot.transform_point3(local).as_dvec3();
    for t in trailers {
        let w = behind(t, local.y);
        if w <= 0.0 {
            break;
        }
        let there = t.pos + t.rot.transform_point3(local - t.offset).as_dvec3();
        here = here.lerp(there, w as f64);
        if w < 1.0 {
            break;
        }
    }
    here
}

pub(super) fn train_heading(heading: f64, trailers: &[PartFrame], local: Vec3) -> f64 {
    let mut here = heading;
    for t in trailers {
        let w = behind(t, local.y);
        if w <= 0.0 {
            break;
        }
        here += crowd::angle_diff(here, t.heading) * w as f64;
        if w < 1.0 {
            break;
        }
    }
    here
}

/// `PAX_*` are engine variables: stock scripts write them without declaring them.
fn script_reports(v: &VehicleInstance, name: &str) -> bool {
    v.has_script_var(name)
        || v.ty
            .program
            .var(name)
            .is_some_and(|id| v.ty.program.stores(id))
}

pub(super) fn doors_open(
    v: &VehicleInstance,
    n_entry: usize,
    n_exit: usize,
) -> (Vec<bool>, Vec<bool>) {
    let door = |k: usize| {
        v.var(&format!("door_{k}"))
            .or_else(|| v.var(&format!("door{k}")))
            .unwrap_or(0.0)
            > 0.5
    };
    let open = |name: String, k: usize| {
        if script_reports(v, &name) {
            v.var(&name).unwrap_or(0.0) > 0.5
        } else {
            door(k.min(7))
        }
    };
    let exit_base = if n_entry <= 1 { 1 } else { 2 };
    (
        (0..n_entry)
            .map(|i| open(format!("PAX_Entry{i}_Open"), i))
            .collect(),
        (0..n_exit)
            .map(|i| open(format!("PAX_Exit{i}_Open"), exit_base + i))
            .collect(),
    )
}

impl BusNow {
    fn of(id: BusId, v: &VehicleInstance, cabin: Arc<Cabin>, speed: f64) -> BusNow {
        let bb =
            v.ty.def
                .bounding_box
                .unwrap_or([2.5, 11.0, 3.0, 0.0, 0.0, 1.5]);
        BusNow {
            id,
            trailers: part_frames(v, &cabin),
            entry_open: vec![false; cabin.entries.len()],
            exit_open: vec![false; cabin.exits.len()],
            walk_open: None,
            cabin,
            pos: v.position,
            rot: v.body_rotation(),
            heading: v.heading,
            speed,
            interior: v.interior_light(),
            air: CabinAir::of(v),
            half: DVec2::new(bb[0] as f64 * 0.5, bb[1] as f64 * 0.5),
            centre: DVec2::new(bb[3] as f64, bb[4] as f64),
            terminus: None,
        }
    }

    fn parked(id: BusId, v: &VehicleInstance, cabin: Arc<Cabin>) -> BusNow {
        let walk_open = doors_open(v, cabin.entries.len(), cabin.exits.len());
        BusNow {
            walk_open: Some(walk_open),
            ..BusNow::of(id, v, cabin, v.physics.velocity_kmh() as f64 / 3.6)
        }
    }

    pub(super) fn world(&self, local: Vec3) -> DVec3 {
        train_point(self.pos, &self.rot, &self.trailers, local)
    }

    pub(super) fn to_local(&self, w: DVec3) -> Vec3 {
        let mut l = self
            .rot
            .inverse()
            .transform_point3((w - self.pos).as_vec3());
        for t in &self.trailers {
            if l.y > t.joint_y {
                break;
            }
            l = t.rot.inverse().transform_point3((w - t.pos).as_vec3()) + t.offset;
        }
        l
    }

    pub(super) fn tilt_at(&self, local: Vec3) -> Mat4 {
        let (mut rot, mut heading) = (self.rot, self.heading);
        for t in self
            .trailers
            .iter()
            .take_while(|t| behind(t, local.y) >= 0.5)
        {
            (rot, heading) = (t.rot, t.heading);
        }
        rot * Mat4::from_rotation_z(heading.to_radians() as f32)
    }

    pub(super) fn heading_at(&self, local: Vec3) -> f64 {
        train_heading(self.heading, &self.trailers, local)
    }

    pub(super) fn fwd(&self) -> DVec2 {
        let h = self.heading.to_radians();
        DVec2::new(h.sin(), h.cos())
    }

    pub(super) fn blocks(&self) -> Vec<Block> {
        let block = |pos: DVec3, heading: f64, half: DVec2, centre: DVec2| {
            let h = heading.to_radians();
            let (fwd, right) = (DVec2::new(h.sin(), h.cos()), DVec2::new(h.cos(), -h.sin()));
            Block {
                center: pos.truncate() + right * centre.x + fwd * centre.y,
                half,
                heading: h,
                vel: fwd * self.speed,
            }
        };
        let mut out = vec![block(self.pos, self.heading, self.half, self.centre)];
        out.extend(
            self.trailers
                .iter()
                .map(|t| block(t.pos, t.heading, t.half, t.centre)),
        );
        out
    }
}

impl Humans {
    pub(super) fn cabin_for(&mut self, v: &VehicleInstance) -> Option<Arc<Cabin>> {
        let parts = train_parts(v);
        let key: Vec<PathBuf> = parts.iter().map(|p| p.0.path.clone()).collect();
        if let Some(c) = self.cabins.get(&key) {
            return c.clone();
        }
        let cabin = Cabin::load_train(&parts).map(Arc::new);
        if let Some(c) = cabin.as_ref().filter(|c| c.parts.len() > 1) {
            log::info!(
                "passenger cabin of {}: {} sections joined ({} places, {} entries, {} exits, {} path points)",
                v.ty.def
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                c.parts.len(),
                c.seats.len(),
                c.entries.len(),
                c.exits.len(),
                c.points.len()
            );
        }
        self.cabins.insert(key, cabin.clone());
        cabin
    }

    /// `GivenTicket` is -1 until the driver hands a ticket over (at 0 the first passenger took
    /// ticket 0).
    pub fn set_cabin(&mut self, vehicle: &mut VehicleInstance) {
        vehicle.set_engine_var("GivenTicket", -1.0);
        let Some(c) = self.cabin_for(vehicle) else {
            log::info!("{}: no passenger cabin", vehicle.ty.def.path.display());
            return;
        };
        log::info!(
            "passenger cabin: {} places ({} seats), {} entries, {} exits, {} path points",
            c.seats.len(),
            c.seats.iter().filter(|s| s.seated).count(),
            c.entries.len(),
            c.exits.len(),
            c.points.len(),
        );
        self.seats.insert(BusId::Player, vec![false; c.seats.len()]);
        self.entry_req = vec![false; c.entries.len().max(1)];
        self.exit_req = vec![false; c.exits.len().max(1)];
        self.player_cabin = Some(c);
    }

    pub(super) fn gather_buses(
        &mut self,
        world: &World,
        bus: Option<&VehicleInstance>,
        traffic: Option<&Traffic>,
    ) -> Vec<BusNow> {
        let mut out = Vec::new();
        if let (Some(b), Some(cabin)) = (bus, self.player_cabin.clone()) {
            let (entry_open, exit_open) = doors_open(b, cabin.entries.len(), cabin.exits.len());
            let terminus = match (b.var("target_index_int"), b.host.hof.as_ref()) {
                (Some(i), Some(hof)) if i.is_finite() && i >= 0.0 => hof
                    .termini
                    .get(i.round() as usize)
                    .filter(|t| !t.all_exit)
                    .map(|t| t.texture_id.trim().to_string()),
                _ => None,
            };
            let speed = b.physics.velocity_kmh() as f64 / 3.6;
            out.push(BusNow {
                entry_open,
                exit_open,
                terminus,
                ..BusNow::of(BusId::Player, b, cabin, speed)
            });
        }
        if let Some(t) = traffic {
            self.gather_timetable_buses(world, t, &mut out);
        }
        for b in self.lan.remote_now.iter().chain(&self.placed_now) {
            self.seats
                .entry(b.id)
                .or_insert_with(|| vec![false; b.cabin.seats.len()]);
            out.push(b.clone());
        }
        out
    }

    fn gather_timetable_buses(&mut self, world: &World, t: &Traffic, out: &mut Vec<BusNow>) {
        let stops: Vec<(i64, DVec3, f64)> = world
            .bus_stops
            .lock()
            .iter()
            .map(|s| (s.0, s.1, s.2))
            .collect();
        let serving = |pos: DVec3, heading: f64| -> Option<i64> {
            let back = |s: &(i64, DVec3, f64)| crowd::angle_diff(heading, s.2).abs() > 100.0;
            stops
                .iter()
                .filter(|s| (s.1 - pos).length() < 18.0)
                .min_by(|a, c| {
                    back(a)
                        .cmp(&back(c))
                        .then((a.1 - pos).length().total_cmp(&(c.1 - pos).length()))
                })
                .map(|s| s.0)
        };
        let riding: HashSet<u64> = self
            .people
            .iter()
            .filter_map(|p| match p.state.bus() {
                Some(BusId::Ai(id)) => Some(id),
                _ => None,
            })
            .collect();
        let mut visits = HashMap::new();
        for c in t.cars.iter().filter(|c| c.is_bus()) {
            let from_eye = self
                .eye
                .map(|e| (c.vehicle.position - e.pos).length())
                .unwrap_or(f64::MAX);
            if (c.vehicle.position - self.center).length().min(from_eye) > 400.0
                && !riding.contains(&c.id)
            {
                continue;
            }
            let Some(cabin) = self.cabin_for(&c.vehicle) else {
                continue;
            };
            let speed = c.state.speed as f64;
            let stop = (c.at_station() && speed.abs() < 0.3)
                .then(|| serving(c.vehicle.position, c.vehicle.heading))
                .flatten();
            let mut bn = BusNow::of(BusId::Ai(c.id), &c.vehicle, cabin, speed);
            bn.terminus = c
                .bus
                .as_ref()
                .map(|b| b.terminus.trim().to_string())
                .filter(|t| !t.is_empty());
            if let Some(s) = stop {
                let visit = match self.ai_visits.get(&c.id) {
                    Some(&(vs, t0)) if vs == s => (vs, t0),
                    _ => (s, self.time),
                };
                visits.insert(c.id, visit);
                let v = &c.vehicle;
                if script_reports(v, "PAX_Entry0_Open")
                    || v.var("door_0").is_some()
                    || v.var("door0").is_some()
                {
                    (bn.entry_open, bn.exit_open) =
                        doors_open(v, bn.cabin.entries.len(), bn.cabin.exits.len());
                } else if self.time - visit.1 > 2.5 {
                    // the script does not say: the doors are open while the bus boards
                    bn.entry_open
                        .iter_mut()
                        .chain(bn.exit_open.iter_mut())
                        .for_each(|o| *o = true);
                }
            }
            self.seats
                .entry(bn.id)
                .or_insert_with(|| vec![false; bn.cabin.seats.len()]);
            out.push(bn);
        }
        self.ai_visits = visits;
        let alive: HashSet<u64> = t
            .cars
            .iter()
            .map(|c| c.id)
            .chain(
                self.lan
                    .remote_now
                    .iter()
                    .chain(&self.placed_now)
                    .filter_map(|b| match b.id {
                        BusId::Ai(id) => Some(id),
                        BusId::Player => None,
                    }),
            )
            .collect();
        self.seats.retain(|k, _| match k {
            BusId::Ai(id) => alive.contains(id),
            BusId::Player => true,
        });
    }

    pub fn set_placed_buses<'a>(
        &mut self,
        buses: impl Iterator<Item = (u64, &'a VehicleInstance)>,
    ) {
        self.placed_now = buses
            .filter_map(|(uid, v)| {
                Some(BusNow::parked(
                    BusId::Ai(placed_bus_id(uid)),
                    v,
                    self.cabin_for(v)?,
                ))
            })
            .collect();
    }

    pub fn set_remote_buses<'a>(
        &mut self,
        buses: impl Iterator<Item = (u32, &'a VehicleInstance)>,
    ) {
        self.lan.remote_now = buses
            .filter_map(|(player, v)| {
                Some(BusNow::parked(
                    BusId::Ai(remote_bus_id(player)),
                    v,
                    self.cabin_for(v)?,
                ))
            })
            .collect();
    }

    pub fn player_bus_swapped(
        &mut self,
        old_uid: u64,
        new_uid: u64,
        new_vehicle: &mut VehicleInstance,
    ) {
        let old = BusId::Ai(placed_bus_id(old_uid));
        let new = BusId::Ai(placed_bus_id(new_uid));
        let tmp = BusId::Ai(u64::MAX);
        self.remap_bus(BusId::Player, tmp);
        self.remap_bus(new, BusId::Player);
        self.remap_bus(tmp, old);
        let kept = self.seats.remove(&BusId::Player);
        self.player_cabin = None;
        self.set_cabin(new_vehicle);
        if let (Some(k), Some(now)) = (kept, self.seats.get_mut(&BusId::Player)) {
            if k.len() == now.len() {
                *now = k;
            }
        }
    }

    pub fn evict(&mut self, bus: BusId) {
        for i in (0..self.people.len()).rev() {
            let p = &self.people[i];
            let theirs = matches!(p.place, Place::Bus(b, _) if b == bus)
                || matches!(&p.state, State::Pax(x) if x.bus == Some(bus) || x.inside == Some(bus));
            if theirs {
                self.remove_person(i);
            }
        }
        self.seats.remove(&bus);
        if bus == BusId::Player {
            self.player_cabin = None;
        }
    }

    fn remap_bus(&mut self, from: BusId, to: BusId) {
        let fix = |b: &mut BusId| {
            if *b == from {
                *b = to;
            }
        };
        for p in &mut self.people {
            if let Place::Bus(b, _) = &mut p.place {
                fix(b);
            }
            if let State::Pax(x) = &mut p.state {
                x.bus.as_mut().map(fix);
                x.inside.as_mut().map(fix);
            }
        }
        if let Some(v) = self.seats.remove(&from) {
            self.seats.insert(to, v);
        }
    }

    pub fn knows_bus(&self, bus: u64) -> bool {
        self.lan.remote_now.iter().any(|b| b.id == BusId::Ai(bus))
            || self.seats.contains_key(&BusId::Ai(bus))
    }

    pub fn write_pax_vars(&self, b: &mut VehicleInstance) {
        for (i, r) in self.entry_req.iter().enumerate() {
            b.set_var(&format!("PAX_Entry{i}_Req"), *r as u8 as f32);
        }
        for (i, r) in self.exit_req.iter().enumerate() {
            b.set_var(&format!("PAX_Exit{i}_Req"), *r as u8 as f32);
        }
    }

    pub fn stop_wishes(&self) -> (HashSet<u64>, HashSet<i64>) {
        let (mut alighting, mut waiting) = (HashSet::new(), HashSet::new());
        for p in &self.people {
            let State::Pax(x) = &p.state else { continue };
            match x.task {
                Task::InBusToExit => {
                    if let Some(BusId::Ai(id)) = x.inside {
                        alighting.insert(id);
                    }
                }
                Task::WaitingForBus | Task::ToBus | Task::WalkingToBus => {
                    waiting.extend(x.stop);
                }
                _ => {}
            }
        }
        (alighting, waiting)
    }

    pub fn take_holds(&mut self) -> Vec<(u64, f32)> {
        std::mem::take(&mut self.holds)
    }

    pub fn take_ai_requests(&mut self) -> Vec<(u64, Vec<bool>, Vec<bool>)> {
        std::mem::take(&mut self.ai_requests)
    }

    pub fn seat_counts(&self) -> Vec<u32> {
        let Some(cabin) = self.player_cabin.as_ref() else {
            return Vec::new();
        };
        let sitting = self.people.iter().filter_map(|p| match &p.state {
            State::Pax(x) if x.inside == Some(BusId::Player) && x.task == Task::SittingInBus => {
                x.seat
            }
            _ => None,
        });
        seat_numbers(&cabin.seats, sitting)
    }

    pub fn path_link_counts(&self) -> Vec<u32> {
        let Some(cabin) = self.player_cabin.as_ref() else {
            return Vec::new();
        };
        let mut out = vec![0u32; cabin.links.len()];
        for p in &self.people {
            if let State::Pax(x) = &p.state {
                if x.inside == Some(BusId::Player)
                    && matches!(x.st, Move::ToTarget | Move::Path | Move::Turn)
                {
                    if let Some(c) = x.link.and_then(|l| out.get_mut(l)) {
                        *c += 1;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bus(name: &str, vars: &str, script: &str) -> (PathBuf, VehicleInstance) {
        let dir = std::env::temp_dir().join(format!("omsi-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let write = |f: &str, text: &str| std::fs::write(dir.join(f), text).unwrap();
        write(
            "test.bus",
            "[model]\nmodel.cfg\n[varnamelist]\n1\nvars.txt\n[script]\n1\nmain.osc\n",
        );
        write("model.cfg", "");
        write("vars.txt", vars);
        write("main.osc", script);
        let ty = Arc::new(omsi_sim::VehicleType::load(&dir, &dir.join("test.bus")).unwrap());
        (
            dir,
            VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default())),
        )
    }

    #[test]
    fn doors_open_falls_back_when_exit_vars_are_undeclared() {
        let (dir, mut v) = test_bus(
            "doors-open",
            "door_0\ndoor_1\ndoor_2\nPAX_Entry0_Open\n",
            "{init}\n{end}\n",
        );
        assert_eq!(doors_open(&v, 2, 1), (vec![false, false], vec![false]));
        v.set_var("PAX_Entry0_Open", 1.0);
        assert_eq!(doors_open(&v, 2, 1), (vec![true, false], vec![false]));
        // the rear door: no PAX_Exit0_Open, so door_2
        v.set_var("door_2", 1.0);
        assert_eq!(doors_open(&v, 2, 1), (vec![true, false], vec![true]));
        v.set_var("door_1", 1.0);
        assert_eq!(doors_open(&v, 2, 1), (vec![true, true], vec![true]));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn doors_open_reads_pax_vars_the_script_writes_without_declaring() {
        let (dir, mut v) = test_bus(
            "doors-open-undeclared",
            "door_0\n",
            "{frame}\n1 (S.L.PAX_Entry0_Open)\n{end}\n",
        );
        v.set_var("door_0", 0.0);
        v.set_var("PAX_Entry0_Open", 1.0);
        assert_eq!(doors_open(&v, 1, 0).0, vec![true]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn doors_open_3door_bus_handles_middle_and_rear_exits() {
        // 6 entries (every leaf of the 3 doors), 4 exits (the middle and the rear door)
        let (dir, mut v) = test_bus(
            "doors-3door",
            "door_0\ndoor_1\ndoor_2\ndoor_3\ndoor_4\ndoor_5\n",
            "{init}\n{end}\n",
        );
        assert_eq!(doors_open(&v, 6, 4), (vec![false; 6], vec![false; 4]));
        v.set_var("door_2", 1.0);
        v.set_var("door_3", 1.0);
        assert_eq!(
            doors_open(&v, 6, 4),
            (
                vec![false, false, true, true, false, false],
                vec![true, true, false, false]
            )
        );
        v.set_var("door_4", 1.0);
        v.set_var("door_5", 1.0);
        assert_eq!(
            doors_open(&v, 6, 4),
            (vec![false, false, true, true, true, true], vec![true; 4])
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
